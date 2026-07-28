use std::{
    env,
    io::IsTerminal,
    os::{fd::AsRawFd, unix::process::CommandExt},
    process::{self, Command},
    sync::{Arc, RwLock},
};

use clap::Parser;
use self_update::cargo_crate_version;
use std::io;
use tokio::sync::broadcast;
use tracing::{error, info, warn};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

use crate::{
    config::{CliConfig, ServerConfig},
    renderer::Renderer,
    state::{SharedState, StageState},
};

mod animator;
mod api;
mod config;
mod fixtures;
mod network;
mod renderer;
mod state;

// self_update config
const GITHUB_REPO_OWNER: &str = "lightstageurop";
const GITHUB_REPO_NAME: &str = "lightstage-server-rs";
const BIN_NAME: &str = "lsserver";
const TAG_PREFIX: &str = "lsserver-v";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LightStageFrame {
    pub rgb_universes: Vec<[u8; 512]>,
    pub white_universes: Vec<[u8; 512]>,
}

impl LightStageFrame {
    #[must_use]
    pub fn black(num_arcs: usize) -> Self {
        Self {
            rgb_universes: vec![[0u8; 512]; num_arcs],
            white_universes: vec![[0u8; 512]; num_arcs],
        }
    }

    pub fn clear(&mut self) {
        for u in &mut self.rgb_universes {
            u.fill(0u8);
        }
        for u in &mut self.white_universes {
            u.fill(0u8);
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // tracing_subscriber init
    init_tracing();

    // parse cli args
    let config = ServerConfig::from(CliConfig::parse());

    // self_update
    if let Err(err) = check_apply_update().await {
        warn!("Failed to update: {err}");
    }

    info!("Starting light stage server..");

    let (tx, _rx) = broadcast::channel(100);

    let mut renderer = Renderer::new(&config);
    for universe in 0..config.num_arcs {
        for fixture in 0..config.lights_per_arc {
            let address = fixtures::DmxAddress::new((fixture * 6 + 1) as u16).unwrap();

            renderer.rgb_fixtures[universe].push(fixtures::RgbFixture::new(address).unwrap());

            renderer.white_fixtures[universe].push(fixtures::WhiteFixture::new(address).unwrap());
        }
    }
    let state: SharedState = Arc::new(RwLock::new(StageState::new(renderer, config, tx.clone())));

    network::NetworkManager::new(state.clone(), config).start()?;
    api::start_server(config, state.clone()).await;

    Ok(())
}

fn init_tracing() {
    // if we can log to journal, do so.
    let journal_layer = tracing_journald::layer().ok();

    // prevent duplicate logs when running as a systemd service
    let fmt_layer = if stdout_is_journal_stream() {
        // stdout would go to journal anyway
        None
    } else {
        let is_tty = io::stdout().is_terminal();
        Some(tracing_subscriber::fmt::layer().compact().with_ansi(is_tty))
    };

    // use RUST_LOG var for log level
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(journal_layer)
        .with(fmt_layer)
        .with(env_filter)
        .init();
}

/// Check if stdout is going to journal
fn stdout_is_journal_stream() -> bool {
    let Ok(journal_stream) = env::var("JOURNAL_STREAM") else {
        return false;
    };

    unsafe {
        let mut stat: libc::stat = std::mem::zeroed();
        if libc::fstat(io::stdout().as_raw_fd(), &raw mut stat) != 0 {
            return false;
        }
        journal_stream == format!("{}:{}", stat.st_dev, stat.st_ino)
    }
}

/// Apply automatic updates from github releases
async fn check_apply_update() -> anyhow::Result<()> {
    if cfg!(debug_assertions) || env::var_os("CARGO").is_some() {
        info!("Running in debug mode or using cargo; skipping self_update.");
        return Ok(());
    }

    // capture current binary path, before it is deleted.
    let exe = env::current_exe()?;

    info!("Checking for updates..");
    let status = self_update::backends::github::Update::configure()
        // repo
        .repo_owner(GITHUB_REPO_OWNER)
        .repo_name(GITHUB_REPO_NAME)
        .bin_name(BIN_NAME)
        .tag_prefix(TAG_PREFIX)
        // silence except for progress bar, and do not prompt for confirmation
        .show_download_progress(true)
        .unattended()
        // compare with crate version
        .current_version(cargo_crate_version!())
        // go
        .build_async()?
        .update_async()
        .await?;

    match status {
        self_update::VersionStatus::Updated(v) => {
            info!("Updated to version {v}. Restarting..");

            #[cfg(unix)]
            {
                let args: Vec<_> = env::args_os().skip(1).collect();
                let err = Command::new(exe).args(args).exec();
                error!("Failed to re-exec process: {err}. Exiting..");
            }

            process::exit(1);
        }
        self_update::VersionStatus::UpToDate(v) => {
            info!("Already up to date. Version: {v}");
        }
        _ => {}
    }

    Ok(())
}
