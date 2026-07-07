use std::{
    io::Cursor,
    net::UdpSocket,
    sync::{Arc, RwLock},
    thread,
    time::{Duration, Instant},
};

use kinetrs::{DmxOutHeader, KinetPacketHeader, KinetPayload};

use crate::{
    config::ServerConfig,
    network::{discover_pds, map_targets},
    renderer::Renderer,
};

mod config;
mod fixtures;
mod network;
mod renderer;
mod universe;

#[derive(Clone)]
pub struct LightStageFrame {
    pub rgb_universes: [[u8; 512]; 12], // TODO dont hard code
    pub white_universes: [[u8; 512]; 12],
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

pub type SharedState = Arc<RwLock<LightStageFrame>>;

fn main() -> anyhow::Result<()> {
    let config = ServerConfig::default();

    println!("Starting light stage server..");

    let raw_targets = discover_pds(config.kinet_port)?;
    let targets = map_targets(raw_targets);

    println!("Discovered {} power supplies", targets.len());

    let state: SharedState = Arc::new(RwLock::new(LightStageFrame::black()));

    let mut renderer = Renderer::new(&config);
    for universe in 0..config.num_arcs {
        for fixture in 0..config.lights_per_arc {
            let address = fixtures::DmxAddress::new((fixture * 6 + 1) as u16).unwrap();

            renderer.rgb_fixtures[universe].push(fixtures::RgbFixture::new(address).unwrap());

            renderer.white_fixtures[universe].push(fixtures::WhiteFixture::new(address).unwrap());
        }
    }

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
                let frame = { state_net.read().unwrap() }.clone();

                for arc in 0..config.num_arcs {
                    if let Some(rgb_addr) = targets.get(&(arc, true)) {
                        let universe = frame.rgb_universes[arc];
                        let header = KinetPacketHeader::DmxOut(DmxOutHeader {
                            sequence,
                            ..Default::default()
                        });
                        header
                            .write_to(&mut Cursor::new(&mut packet[0..21]))
                            .expect("failed to serialise");
                        packet[21..].copy_from_slice(&universe);
                        socket
                            .send_to(&packet, rgb_addr)
                            .expect("failed to send rgb");
                    }

                    if let Some(white_addr) = targets.get(&(arc, false)) {
                        let universe = frame.white_universes[arc];
                        let header = KinetPacketHeader::DmxOut(DmxOutHeader {
                            sequence,
                            ..Default::default()
                        });
                        header
                            .write_to(&mut Cursor::new(&mut packet[0..21]))
                            .expect("failed to serialise");
                        packet[21..].copy_from_slice(&universe);
                        socket
                            .send_to(&packet, white_addr)
                            .expect("failed to send white");
                    }
                }

                sequence = sequence.wrapping_add(1);
            }

            next_time += config.refresh_rate;

            let now = Instant::now();
            if next_time > now {
                thread::sleep(next_time - now);
            } else {
                let lateness =
                    now.duration_since(next_time.checked_sub(config.refresh_rate).unwrap());
                println!(
                    "oops. frame took {lateness:?} (Target was {:?})",
                    config.refresh_rate
                );

                next_time = now;
            }
        }
    });

    // {
    //     let state = state.clone();
    //     thread::spawn(move || {
    //         for arc in 0..12 {
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

    // this following is chatgpt's doing not mine
    // it's pretty.
    // i don't trust it.
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(500));

        let start_time = Instant::now();

        // Adjust this value between 0.0 (off) and 1.0 (full blast)
        let brightness: f32 = 0.1;

        // Pre-calculate our sine wave boundaries based on the brightness
        let amplitude = 32767.5 * brightness;
        let center = 32767.5 * brightness;

        loop {
            let elapsed = start_time.elapsed().as_secs_f32();

            for arc in 0..12 {
                for light in 0..config.lights_per_arc {
                    let phase_offset = (arc as f32 * 0.5) + (light as f32 * 0.2);
                    let t = elapsed * 2.0 + phase_offset;

                    // Calculate RGB using the new dimmed amplitude and center
                    let r = ((t).sin() * amplitude + center) as u16;
                    let g = ((t + 2.094).sin() * amplitude + center) as u16;
                    let b = ((t + 4.188).sin() * amplitude + center) as u16;

                    renderer.rgb_fixtures[arc][light].set_color(r, g, b);

                    // Apply the same dimming to the white fixtures
                    let w_phase = elapsed * 1.5 - phase_offset;
                    let w = ((w_phase.sin() * amplitude) + center) as u16;
                    renderer.white_fixtures[arc][light].set_white(w, w, w);
                }
            }

            {
                let mut state_e = state_render.write().unwrap();
                renderer.update(&mut state_e);
            }

            thread::sleep(Duration::from_millis(10));
        }
    });

    loop {
        thread::park();
    }
}
