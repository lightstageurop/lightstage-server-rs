use std::{
    io::Cursor,
    net::UdpSocket,
    sync::{Arc, RwLock},
    thread,
    time::{Duration, Instant},
};

use kinetrs::{DmxOutHeader, KinetPacketHeader, KinetPayload};
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

use crate::{
    config::ServerConfig,
    demo::DemoAnimator,
    network::{discover_pds, map_targets},
    renderer::Renderer,
    state::{SharedState, StageMode, StageState},
};

mod api;
mod config;
mod demo;
mod fixtures;
mod network;
mod renderer;
mod state;
mod universe;

#[derive(Debug, Clone)]
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

    let raw_targets = discover_pds(config.kinet_port)?;
    let targets = map_targets(raw_targets);

    info!("Discovered {} power supplies", targets.len());

    let mut renderer = Renderer::new(&config);
    for universe in 0..config.num_arcs {
        for fixture in 0..config.lights_per_arc {
            let address = fixtures::DmxAddress::new((fixture * 6 + 1) as u16).unwrap();

            renderer.rgb_fixtures[universe].push(fixtures::RgbFixture::new(address).unwrap());

            renderer.white_fixtures[universe].push(fixtures::WhiteFixture::new(address).unwrap());
        }
    }

    let state: SharedState = Arc::new(RwLock::new(StageState::new(renderer)));

    let state_net = state.clone();
    let socket = UdpSocket::bind("0.0.0.0:0")?;

    thread::spawn(move || {
        // Neither ManagementTool nor kinet.py use this, always set to zero.
        // we do just cuz
        let mut sequence = 0u32;

        let mut packet = vec![0u8; DmxOutHeader::PACKET_SIZE + 512];

        let mut next_time = Instant::now();

        loop {
            {
                let frame = {
                    let lock = state_net.write().unwrap();
                    match lock.mode {
                        state::StageMode::Demo | state::StageMode::Manual => {
                            lock.current_frame.clone()
                        }
                        state::StageMode::Playback => todo!(),
                    }
                };

                let header: KinetPacketHeader = DmxOutHeader {
                    sequence,
                    ..Default::default()
                }
                .into();
                header
                    .write_to(&mut Cursor::new(&mut packet[0..DmxOutHeader::PACKET_SIZE]))
                    .expect("failed to serialise");

                for arc in 0..config.num_arcs {
                    if let Some(rgb_addr) = targets.get(&(arc, true)) {
                        let universe = frame.rgb_universes[arc];

                        packet[DmxOutHeader::PACKET_SIZE..].copy_from_slice(&universe);
                        socket
                            .send_to(&packet, rgb_addr)
                            .expect("failed to send rgb");
                    }

                    if let Some(white_addr) = targets.get(&(arc, false)) {
                        let universe = frame.white_universes[arc];
                        packet[DmxOutHeader::PACKET_SIZE..].copy_from_slice(&universe);
                        socket
                            .send_to(&packet, white_addr)
                            .expect("failed to send white");
                    }
                }

                sequence = sequence.wrapping_add(1);
            }

            let refresh_time = Duration::from_millis(config.refresh_rate_ms);
            next_time += refresh_time;

            let now = Instant::now();
            if next_time > now {
                thread::sleep(next_time - now);
            } else {
                let lateness = now.duration_since(next_time.checked_sub(refresh_time).unwrap());
                warn!(
                    "oops. frame took {lateness:?} (Target was {:?})",
                    refresh_time
                );

                next_time = now;
            }
        }
    });

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

    let state_render = state.clone();

    thread::spawn(move || {
        thread::sleep(Duration::from_millis(500));

        let animator = DemoAnimator::new(0.1);

        loop {
            {
                let mut lock = state_render.write().unwrap();
                if lock.mode == StageMode::Demo {
                    animator.tick(&mut lock, &config);
                }
            }
            thread::sleep(Duration::from_millis(10));
        }
    });

    api::start_server(config, state.clone()).await;

    Ok(())
}
