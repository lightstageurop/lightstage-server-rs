use std::sync::{Arc, RwLock};

use tracing::info;
use tracing_subscriber::EnvFilter;

use crate::{
    config::ServerConfig,
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
mod universe;

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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .compact()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let config = ServerConfig::default();

    info!("Starting light stage server..");

    let mut renderer = Renderer::new(&config);
    for universe in 0..config.num_arcs {
        for fixture in 0..config.lights_per_arc {
            let address = fixtures::DmxAddress::new((fixture * 6 + 1) as u16).unwrap();

            renderer.rgb_fixtures[universe].push(fixtures::RgbFixture::new(address).unwrap());

            renderer.white_fixtures[universe].push(fixtures::WhiteFixture::new(address).unwrap());
        }
    }
    let state: SharedState = Arc::new(RwLock::new(StageState::new(renderer, config)));

    network::NetworkManager::new(state.clone(), config).start()?;

    // {
    //     let state = state.clone();
    //     thread::spawn(move || {
    //         for arc in 0..config.num_arcs {
    //             for light in 0..config.lights_per_arc {
    //                 renderer.rgb_fixtures[arc][light].set_color(0, 65535, 0);
    //                 renderer.white_fixtures[arc][light].set_white(0, 0, 65535);
    //                 {
    //                     let mut state_e = state.write().unwrap();
    //                     renderer.update(&mut state_e);
    //                 }

    //                 thread::sleep(Duration::from_millis(100));
    //             }
    //         }
    //     });
    // }

    api::start_server(config, state.clone()).await;

    Ok(())
}
