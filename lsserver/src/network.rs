//! # `KiNET` communication with PDSs
//!
//! Discovery, DMX refreshing and heartbeat listening.

use std::{
    collections::HashMap,
    io::Cursor,
    net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket},
    sync::{Arc, RwLock},
    thread,
    time::{Duration, Instant},
};

use anyhow::anyhow;
use kinetrs::{DmxOutHeader, KinetPacketHeader, KinetPayload, PollPayload, PollReplyPayload};
use tracing::{debug, error, info, warn};

use crate::{
    LightStageFrame,
    config::ServerConfig,
    state::{SharedState, StageMode, TickResult},
};

/// One of our discovered PDS on the network.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KinetPowerSupply {
    pub remote_adr: SocketAddr,
    pub arc_index: usize,
    pub is_rgb: bool,
}

/// Discover PDS on the network with [`kinetrs::KinetPacketHeader::Poll`] and listen for replies.
pub fn discover_pds(port: u16) -> anyhow::Result<Vec<KinetPowerSupply>> {
    let ifaces = local_ip_address::list_afinet_netifas()?;

    // Find the correct local IP to bind to when there are multple interfaces
    // TODO this should be in main so we can reused for the refresh dmx thread too.
    let local_ip = ifaces
        .into_iter()
        .find_map(|(_, ip)| match ip {
            IpAddr::V4(v4_addr) if v4_addr.octets()[0] == 10 => Some(ip),
            _ => None,
        })
        .ok_or_else(|| {
            anyhow!(
                "No active network interfaces found in 10.0.0.0/8 range. Is ethernet connected?"
            )
        })?;

    // Bind to it, instead of 0.0.0.0 which may result in a different interface being used.
    let socket = UdpSocket::bind(SocketAddr::new(local_ip, 0))?;

    socket.set_broadcast(true)?;
    socket.set_read_timeout(Some(Duration::from_millis(100)))?;

    // Outbound discovery packet
    let poll_packet: KinetPacketHeader = PollPayload {
        // This cannot be 0.0.0.0 or 255.255.255.255 otherwise the replies will never reach us.
        // It doesn't technically have to be on the correct subnet however.
        magic_ip: Ipv4Addr::new(10, 37, 1, 1),
        ..Default::default()
    }
    .into();

    // Serialise and send it
    let mut buf = Vec::new();
    poll_packet.write_to(&mut buf)?;
    socket.send_to(&buf, SocketAddr::new(Ipv4Addr::BROADCAST.into(), port))?;

    let mut discovered_targets = Vec::new();
    let mut buf = [0u8; PollReplyPayload::PACKET_SIZE];
    let start_time = Instant::now();

    while start_time.elapsed() < Duration::from_secs(1) {
        // ignore recv timeouts or other socket errors
        let Ok((size, _src)) = socket.recv_from(&mut buf) else {
            continue;
        };

        // serialise the packet or warn and continue
        let packet = match KinetPacketHeader::read_from(&mut Cursor::new(&mut buf[..size])) {
            Ok(p) => p,
            Err(e) => {
                warn!("Received unparsable network packet: {e:?}");
                continue;
            }
        };

        // ignore anything that isnt a reply (eg. heartbeat)
        let KinetPacketHeader::PollReply(reply) = packet else {
            continue;
        };

        let label = reply.node_label_as_str().unwrap_or_default();
        debug!(
            "Found PDS {:X} at {}. Label: {}",
            reply.serial, reply.src_ip, label
        );

        // check and parse our custom label format. "Arc N(C/W)"
        let label_parts: Vec<&str> = label.split_whitespace().collect(); // eg. ["Arc","0C"]
        if let [_, identifier] = label_parts.as_slice() {
            let (is_rgb, num_str) = if let Some(n) = identifier.strip_suffix('C') {
                (true, n)
            } else if let Some(n) = identifier.strip_suffix('W') {
                (false, n)
            } else {
                continue; // identifier doesn't end in C or W.
            };

            // try and parse the arc number
            if let Ok(arc_index) = num_str.parse::<usize>() {
                // success. push back PDS info
                discovered_targets.push(KinetPowerSupply {
                    remote_adr: SocketAddr::new(reply.src_ip.into(), port),
                    arc_index,
                    is_rgb,
                });
            }
        }
    }

    Ok(discovered_targets)
}

/// Map a vec of discovered PDSs for faster lookup.
///
/// key: `(arc_index, is_rgb)`, value: `SocketAddr`
pub fn map_targets(
    raw_targets: Vec<KinetPowerSupply>,
) -> HashMap<(usize, bool), std::net::SocketAddr> {
    raw_targets
        .into_iter()
        .map(|pds| ((pds.arc_index, pds.is_rgb), pds.remote_adr))
        .collect()
}

/// Manages `KiNET` communication
#[derive(Debug)]
pub struct NetworkManager {
    state: SharedState,
    config: ServerConfig,
    last_heartbeat: Arc<RwLock<Instant>>,
}

impl NetworkManager {
    pub fn new(state: SharedState, config: ServerConfig) -> Self {
        Self {
            state,
            config,
            last_heartbeat: Arc::new(RwLock::new(Instant::now())),
        }
    }

    /// Discover PDS, spawn kinet threads
    pub fn start(self) -> anyhow::Result<()> {
        let raw_targets = discover_pds(self.config.kinet_port)?;
        let targets = map_targets(raw_targets);
        info!("Discovered {} power supplies", targets.len());

        self.spawn_heartbeat_monitor();

        let mut socket = UdpSocket::bind("0.0.0.0:0")?;
        thread::spawn(move || self.run(&mut socket, &targets));

        Ok(())
    }

    pub fn is_healthy(&self) -> bool {
        self.last_heartbeat.read().unwrap().elapsed() < Duration::from_secs(91)
    }

    /// DMX refresh loop
    fn run(self, socket: &mut UdpSocket, targets: &HashMap<(usize, bool), SocketAddr>) {
        // Neither ManagementTool nor kinet.py use this, always set to zero. we do, because we can.
        let mut sequence = 0u32;
        let mut packet = vec![0u8; DmxOutHeader::PACKET_SIZE + 512];
        let mut next_time = Instant::now();
        let mut refresh_time = Duration::from_millis(self.config.refresh_rate_ms);

        let mut pkt_counter = 0;
        let mut pkts_per_frame = 1;
        let mut current_frame_data = LightStageFrame::black();
        let mut should_trigger = false;

        loop {
            // only advance animation tick every k network packets
            if pkt_counter == 0 {
                // update current_frame_data and get mode, result.
                let (tick_result, mode, capture_hz) = {
                    let mut lock = self.state.write().unwrap();
                    let result = lock.advance_tick(&mut current_frame_data);
                    let hz = lock.active_session.as_ref().map(|s| s.config.capture_hz);
                    (result, lock.mode, hz)
                };

                // set synced refresh rate
                match mode {
                    StageMode::Demo | StageMode::Manual => {
                        pkts_per_frame = 1;
                        refresh_time = Duration::from_millis(self.config.refresh_rate_ms);
                    }
                    StageMode::Playback | StageMode::OLAT => {
                        // find max network ticks per frame update
                        let max_network_hz = 1000.0 / self.config.refresh_rate_ms as f64;
                        let hz = capture_hz.unwrap_or(max_network_hz);
                        pkts_per_frame = (max_network_hz / hz).floor().max(1.0) as usize;
                        // real network refresh rate synced with capture_hz
                        let real_network_hz = (hz * pkts_per_frame as f64).min(max_network_hz);
                        refresh_time = Duration::from_secs_f64(1.0 / (real_network_hz));
                    }
                }

                // TODO fire cameras from the last frame before we send the new frame
                if should_trigger {
                    // hopefully this is enough time for the fixtures to turn on
                    thread::sleep(Duration::from_millis(4));
                    // TODO gpio
                }

                should_trigger = tick_result == TickResult::TriggerCapture;
            }

            pkt_counter += 1;
            if pkt_counter >= pkts_per_frame {
                pkt_counter = 0;
            }

            // build header (same for each PDS)
            KinetPacketHeader::from(DmxOutHeader {
                sequence,
                ..Default::default()
            })
            .write_to(&mut Cursor::new(&mut packet[0..DmxOutHeader::PACKET_SIZE]))
            .expect("failed to serialise");

            for arc in 0..self.config.num_arcs {
                if let Some(rgb_addr) = targets.get(&(arc, true)) {
                    packet[DmxOutHeader::PACKET_SIZE..]
                        .copy_from_slice(&current_frame_data.rgb_universes[arc]);
                    let _ = socket.send_to(&packet, rgb_addr);
                }

                if let Some(white_addr) = targets.get(&(arc, false)) {
                    packet[DmxOutHeader::PACKET_SIZE..]
                        .copy_from_slice(&current_frame_data.white_universes[arc]);
                    let _ = socket.send_to(&packet, white_addr);
                }
            }

            sequence = sequence.wrapping_add(1);

            next_time += refresh_time;
            let now = Instant::now();
            if next_time > now {
                thread::sleep(next_time - now);
            } else {
                let lateness =
                    now.duration_since(next_time.checked_sub(refresh_time).unwrap_or(now));
                warn!(
                    "oops. frame took {lateness:?} (Target was {:?})",
                    refresh_time
                );
                next_time = now;
            }
        }
    }

    fn spawn_heartbeat_monitor(&self) {
        // TODO actually map each target and keep counter for each

        // received thread
        let last_hb_receiver = self.last_heartbeat.clone();
        let rx_port = self.config.heartbeat_port;
        thread::spawn(move || {
            // Bind to the port where the power supplies broadcast or echo replies
            let rx_socket = UdpSocket::bind(format!("0.0.0.0:{rx_port}"))
                // must be 0.0.0.0 not unicast otherwise os will not give us heartbeats sent to 255.255.255.255
                .expect("Failed to bind incoming KiNET heartbeat socket");
            let mut buf = [0u8; 1024];

            loop {
                if let Ok((amt, _src)) = rx_socket.recv_from(&mut buf) {
                    let mut cursor = Cursor::new(&buf[..amt]);
                    // Check if it's a valid KiNET packet format
                    if let Ok(KinetPacketHeader::HeartBeat(hb)) =
                        KinetPacketHeader::read_from(&mut cursor)
                    {
                        debug!("heartbeat: {hb:?}");
                        let mut lock = last_hb_receiver.write().unwrap();
                        *lock = Instant::now();
                    }
                }
            }
        });

        // watchdog thread
        let last_hb_watchdog = self.last_heartbeat.clone();
        thread::spawn(move || {
            // 90s interval * 2 + 20s grace period
            let timeout_limit = Duration::from_secs(200);
            loop {
                thread::sleep(Duration::from_secs(5));
                let time_since_last = last_hb_watchdog.read().unwrap().elapsed();
                if time_since_last > timeout_limit {
                    error!(
                        "Lost communication with KiNET power supplies! No heartbeats received for over 90 seconds."
                    );
                    // TODO update state here or just panic?
                }
            }
        });
    }
}
