use std::{
    net::UdpSocket,
    sync::{Arc, RwLock},
    thread,
    time::{Duration, Instant},
};

use kinetrs::{KINET_UDP_PORT, KinetHeader};

use crate::{
    config::{KINET_REFRESH_RATE_MS, LIGHTS_PER_ARC, NUM_ARCS, PDS_SUBNET_BASE},
    renderer::Renderer,
};

mod config;
mod fixtures;
mod renderer;
mod universe;

pub struct LightStageFrame {
    pub rgb_universes: [[u8; 512]; NUM_ARCS],
    pub white_universes: [[u8; 512]; NUM_ARCS],
}

impl LightStageFrame {
    #[must_use]
    pub fn black() -> Self {
        Self {
            rgb_universes: [[0u8; 512]; NUM_ARCS],
            white_universes: [[0u8; 512]; NUM_ARCS],
        }
    }
}

pub type SharedState = Arc<RwLock<LightStageFrame>>;

fn main() -> anyhow::Result<()> {
    let state: SharedState = Arc::new(RwLock::new(LightStageFrame::black()));

    let mut renderer = Renderer::new();
    for universe in 0..NUM_ARCS {
        for fixture in 0..LIGHTS_PER_ARC {
            let address = fixtures::DmxAddress::new((fixture * 6 + 1) as u16).unwrap();

            renderer.rgb_fixtures[universe].push(fixtures::RgbFixture::new(address).unwrap());

            renderer.white_fixtures[universe].push(fixtures::WhiteFixture::new(address).unwrap());
        }
    }

    let socket = UdpSocket::bind("0.0.0.0:0")?;
    let state_net = state.clone();

    thread::spawn(move || {
        let mut sequence = 0u32;

        let mut packet = vec![0u8; 21 + 512];

        loop {
            let start = Instant::now();
            {
                {
                    let frame = state_net.read().unwrap();

                    for pds in 0..24 {
                        let universe = if pds % 2 == 0 {
                            frame.rgb_universes[pds / 2]
                        } else {
                            frame.white_universes[pds / 2]
                        };

                        let header = KinetHeader {
                            sequence,
                            ..Default::default()
                        };
                        sequence = sequence.wrapping_add(1);
                        packet[0..21].copy_from_slice(&header.to_bytes());
                        packet[21..].copy_from_slice(&universe);

                        let target = format!("{PDS_SUBNET_BASE}{pds}:{KINET_UDP_PORT}");
                        socket.send_to(&packet, target).expect("failed to send");
                    }
                }

                let elapsed = start.elapsed();
                if elapsed < KINET_REFRESH_RATE_MS {
                    thread::sleep(KINET_REFRESH_RATE_MS - elapsed);
                }
            }
        }
    });

    // {
    //     let state = state.clone();
    //     thread::spawn(move || {
    //         for arc in 0..12 {
    //             for light in 0..LIGHTS_PER_ARC {
    //                 renderer.rgb_fixtures[arc][light].set_color(100, 100, 100);
    //                 renderer.white_fixtures[arc][light].set_white(100, 100, 100);
    //                 {
    //                     let mut state_e = state.write().unwrap();
    //                     renderer.update(&mut state_e);
    //                 }

    //                 thread::sleep(Duration::from_millis(500));
    //             }
    //         }
    //     });
    // }

    let state_render = state.clone();

    // this following is chatgpt's doing not mine
    // it's pretty.
    // i don't trust it.
    thread::spawn(move || {
        let start_time = Instant::now();

        // Adjust this value between 0.0 (off) and 1.0 (full blast)
        let brightness: f32 = 0.2;

        // Pre-calculate our sine wave boundaries based on the brightness
        let amplitude = 127.5 * brightness;
        let center = 127.5 * brightness;

        loop {
            let elapsed = start_time.elapsed().as_secs_f32();

            for arc in 0..12 {
                for light in 0..LIGHTS_PER_ARC {
                    let phase_offset = (arc as f32 * 0.5) + (light as f32 * 0.2);
                    let t = elapsed * 2.0 + phase_offset;

                    // Calculate RGB using the new dimmed amplitude and center
                    let r = ((t).sin() * amplitude + center) as u8;
                    let g = ((t + 2.094).sin() * amplitude + center) as u8;
                    let b = ((t + 4.188).sin() * amplitude + center) as u8;

                    renderer.rgb_fixtures[arc][light].set_color(r, g, b);

                    // Apply the same dimming to the white fixtures
                    let w_phase = elapsed * 1.5 - phase_offset;
                    let w = ((w_phase.sin() * amplitude) + center) as u8;
                    renderer.white_fixtures[arc][light].set_white(w, w, w);
                }
            }

            {
                let mut state_e = state_render.write().unwrap();
                renderer.update(&mut state_e);
            }

            thread::sleep(Duration::from_millis(16));
        }
    });

    loop {
        thread::park();
    }
}
