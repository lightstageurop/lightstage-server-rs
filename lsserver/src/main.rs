use std::{
    env,
    io::IsTerminal,
    os::fd::AsRawFd,
    sync::{Arc, RwLock},
};

use clap::Parser;
use std::io;
use tokio::sync::broadcast;
use tracing::info;
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LightStageFrame {
    pub rgb_universes: [[u8; 512]; 12], // TODO dont hard code
    pub white_universes: [[u8; 512]; 12],
}

impl Default for LightStageFrame {
    fn default() -> Self {
        Self::black()
    }
}

impl LightStageFrame {
    #[must_use]
    pub fn black() -> Self {
        Self {
            rgb_universes: [[0u8; 512]; 12],
            white_universes: [[0u8; 512]; 12],
        }
    }
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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
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

    // parse cli args
    let config = ServerConfig::from(CliConfig::parse());

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
