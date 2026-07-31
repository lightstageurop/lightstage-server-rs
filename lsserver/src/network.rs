//! # `KiNET` communication with PDSs
//!
//! Discovery, DMX refreshing and heartbeat listening.

use std::{
    collections::{HashMap, HashSet},
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
    config::ServerConfig,
    renderer::LightStageFrame,
    state::{SharedState, StageMode, TickResult},
};

/// Hashmap key for a PDS: `(arc_index, is_rgb)`
type PdsKey = (usize, bool);

/// One of our discovered PDS on the network.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KinetPowerSupply {
    pub remote_adr: SocketAddr,
    pub serial: u32,
    pub arc_index: usize,
    pub is_rgb: bool,
}

/// Find the correct local IP to bind to when there are multple interfaces
fn get_local_kinet_ip() -> anyhow::Result<IpAddr> {
    let ifaces = local_ip_address::list_afinet_netifas()?;

    ifaces
        .into_iter()
        .find_map(|(_, ip)| match ip {
            IpAddr::V4(v4_addr) if v4_addr.octets()[0] == 10 => Some(ip),
            _ => None,
        })
        .ok_or_else(|| {
            anyhow!(
                "No active network interfaces found in 10.0.0.0/8 range. Is ethernet connected?"
            )
        })
}

/// Discover PDS on the network with [`kinetrs::KinetPacketHeader::Poll`] and listen for replies.
pub fn discover_pds(
    port: u16,
    num_arcs: usize,
    max_retries: usize,
) -> anyhow::Result<Vec<KinetPowerSupply>> {
    // Bind to specific IP, instead of 0.0.0.0 which may result in a different interface being used.
    let local_ip = get_local_kinet_ip()?;
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

    let expected_total = num_arcs * 2;

    let mut discovered_targets = Vec::new();
    let mut seen_serials = HashSet::new();
    let mut recv_buf = [0u8; PollReplyPayload::PACKET_SIZE];

    for attempt in 1..=max_retries {
        socket.send_to(&buf, SocketAddr::new(Ipv4Addr::BROADCAST.into(), port))?;
        let start_time = Instant::now();

        while start_time.elapsed() < Duration::from_millis(500) {
            // ignore recv timeouts or other socket errors
            let Ok((size, _src)) = socket.recv_from(&mut recv_buf) else {
                continue;
            };

            // serialise the packet or warn and continue
            let packet = match KinetPacketHeader::read_from(&mut Cursor::new(&mut recv_buf[..size]))
            {
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

            if !seen_serials.insert(reply.serial) {
                continue; // already processed this serial in a previous attempt
            }

            let label = reply.node_label_as_str().unwrap_or_default();
            debug!(
                "Found PDS {:X} at {}. Label: '{}'",
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
                        serial: reply.serial,
                        arc_index,
                        is_rgb,
                    });
                }
            }
        }

        info!(
            "Discovery attempt {attempt}/{max_retries}: found {}/{} expected PDSs.",
            discovered_targets.len(),
            expected_total
        );

        if discovered_targets.len() >= expected_total {
            thread::sleep(Duration::from_millis(200));
        }
    }

    Ok(discovered_targets)
}

/// Map a vec of discovered PDSs for faster lookup.
///
/// key: `(arc_index, is_rgb)`, value: `SocketAddr`
pub fn map_and_validate_targets(
    raw_targets: Vec<KinetPowerSupply>,
    num_arcs: usize,
) -> anyhow::Result<HashMap<PdsKey, KinetPowerSupply>> {
    let mut targets: HashMap<PdsKey, KinetPowerSupply> = HashMap::new();
    let mut duplicates = Vec::new();
    let mut out_of_bounds = Vec::new();

    for pds in raw_targets {
        if pds.arc_index >= num_arcs {
            out_of_bounds.push(format!(
                "Arc {}{} at {} (Serial: {:X}",
                pds.arc_index,
                if pds.is_rgb { 'C' } else { 'W' },
                pds.remote_adr,
                pds.serial
            ));
        }

        let key = (pds.arc_index, pds.is_rgb);
        if let Some(existing) = targets.insert(key, pds.clone()) {
            if existing.remote_adr != pds.remote_adr {
                duplicates.push(format!(
                    "Arc {}{}: both {} (Serial: {:X}) and {} (Serial: {:X})",
                    pds.arc_index,
                    if pds.is_rgb { 'C' } else { 'W' },
                    existing.remote_adr,
                    existing.serial,
                    pds.remote_adr,
                    pds.serial
                ));
            }
        }
    }

    if !out_of_bounds.is_empty() {
        warn!(
            "Found PDSs with invalid arc indices for configured {num_arcs} arcs: [{}]",
            out_of_bounds.join(", ")
        );
    }

    if !duplicates.is_empty() {
        anyhow::bail!("Duplicate PDSs detected:\n - {}", duplicates.join("\n - "))
    }

    let mut missing = Vec::new();
    for arc in 0..num_arcs {
        if !targets.contains_key(&(arc, true)) {
            missing.push(format!("Arc {arc}C"));
        }
        if !targets.contains_key(&(arc, false)) {
            missing.push(format!("Arc {arc}W"));
        }
    }

    if !missing.is_empty() {
        anyhow::bail!(
            "Could not discover all PDSs. Missing {} required for {num_arcs} arcs: [{}]",
            missing.len(),
            missing.join(", ")
        );
    }

    Ok(targets)
}

/// Timing parameters for DMX refresh loop
///
/// In playback modes, multiple DMX packets may be sent for each logical animation frame,
/// however the DMX refresh rate must always stay confined within some limits and remain synchronised
/// with the requested `capture_hz`.
#[derive(Debug, Clone, Copy)]
pub struct FrameTiming {
    /// Number of DMX refresh packets to be sent before advancing to the next animator frame.
    pub sub_ticks_per_frame: usize,
    /// Time between DMX refresh packets
    pub tick_duration: Duration,
}

impl FrameTiming {
    /// Computes and returns a new [`FrameTiming`] struct for current [`StageMode`].
    ///
    /// [`StageMode::Playback`] and [`StageMode::OLAT`] synchronise frame advancement to requested `capture_hz`,
    /// but may have multiple `sub_ticks_per_frame`, if possible while staying under `base_refresh_ms`.
    pub fn calculate(base_refresh_ms: u64, mode: StageMode, capture_hz: Option<f64>) -> Self {
        let refresh_time = Duration::from_millis(base_refresh_ms);

        match mode {
            StageMode::Demo | StageMode::Manual => Self {
                sub_ticks_per_frame: 1,
                tick_duration: refresh_time,
            },
            StageMode::Playback | StageMode::OLAT => {
                // find max network ticks per frame update
                let max_network_hz = 1000.0 / base_refresh_ms as f64;
                let hz = capture_hz.unwrap_or(max_network_hz);
                let sub_ticks_per_frame = (max_network_hz / hz).floor().max(1.0) as usize;
                // real network refresh rate synced with capture_hz
                let real_network_hz = (hz * sub_ticks_per_frame as f64).min(max_network_hz);
                let refresh_time = Duration::from_secs_f64(1.0 / (real_network_hz));
                Self {
                    sub_ticks_per_frame,
                    tick_duration: refresh_time,
                }
            }
        }
    }
}

/// Sends DMX packets ([`kinetrs::KinetPacketHeader::DmxOut`]) to all target PDSs.
#[derive(Debug)]
pub struct DmxBroadcaster<'a> {
    socket: &'a UdpSocket,
    targets: &'a HashMap<PdsKey, KinetPowerSupply>,
    num_arcs: usize,
    sequence: u32,
    packet_buf: Vec<u8>,
}

impl<'a> DmxBroadcaster<'a> {
    /// Returns a new [`DmxBroadcaster`].
    ///
    /// Uses borrowed udp socket and targets.
    /// Allocates and reuses a packet buffer large enough to serialise a `DmxOut` packet.
    pub fn new(
        socket: &'a UdpSocket,
        targets: &'a HashMap<PdsKey, KinetPowerSupply>,
        num_arcs: usize,
    ) -> Self {
        Self {
            socket,
            targets,
            num_arcs,
            sequence: 0,
            packet_buf: vec![0u8; DmxOutHeader::PACKET_SIZE + 512],
        }
    }

    /// Broadcast a complete [`LightStageFrame`].
    ///
    /// Serialises and sends a `DmxOut` packet to each target PDS, with its respective DMX universe from the frame.
    pub fn broadcast_frame(&mut self, frame: &LightStageFrame) {
        // build header (same for each PDS)
        let header = KinetPacketHeader::from(DmxOutHeader {
            // Neither ManagementTool nor kinet.py use this, always set to zero. we do, because we can.
            sequence: self.sequence,
            ..Default::default()
        });

        if header
            .write_to(&mut Cursor::new(
                &mut self.packet_buf[0..DmxOutHeader::PACKET_SIZE],
            ))
            .is_err()
        {
            error!("Failed to serialise DmxOut header!");
            return;
        }

        // send universes for each arc
        for arc in 0..self.num_arcs {
            // colour PDS
            if let Some(KinetPowerSupply {
                remote_adr: rgb_addr,
                ..
            }) = self.targets.get(&(arc, true))
            {
                self.packet_buf[DmxOutHeader::PACKET_SIZE..]
                    .copy_from_slice(&frame.rgb_universes[arc]);
                let _ = self.socket.send_to(&self.packet_buf, rgb_addr);
            }

            // white PDS
            if let Some(KinetPowerSupply {
                remote_adr: white_addr,
                ..
            }) = self.targets.get(&(arc, false))
            {
                self.packet_buf[DmxOutHeader::PACKET_SIZE..]
                    .copy_from_slice(&frame.white_universes[arc]);
                let _ = self.socket.send_to(&self.packet_buf, white_addr);
            }
        }

        self.sequence = self.sequence.wrapping_add(1);
    }
}

/// Manages `KiNET` communication
#[derive(Debug)]
pub struct NetworkManager {
    state: SharedState,
    config: ServerConfig,
    pds_heartbeats: Arc<RwLock<HashMap<u32, Instant>>>,
}

impl NetworkManager {
    /// PDS Timeout: 90s interval * 2 + 20s grace period
    const PDS_TIMEOUT_LIMIT: Duration = Duration::from_secs(200);

    pub fn new(state: SharedState, config: ServerConfig) -> Self {
        Self {
            state,
            config,
            pds_heartbeats: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Discover PDS, spawn kinet threads
    pub fn start(self) -> anyhow::Result<()> {
        let raw_targets = discover_pds(self.config.kinet_port, self.config.num_arcs, 3)?;
        let targets = map_and_validate_targets(raw_targets, self.config.num_arcs)?;
        info!(
            "Successfully discovered and mapped {} power supplies",
            targets.len()
        );

        // initialise heartbeat tracker with only valid targets
        let now = Instant::now();
        {
            let mut hb_lock = self.pds_heartbeats.write().unwrap();
            for pds in targets.values() {
                hb_lock.insert(pds.serial, now);
            }
        }

        self.spawn_heartbeat_monitor();

        let mut socket = UdpSocket::bind("0.0.0.0:0")?;
        thread::spawn(move || self.run(&mut socket, &targets));

        Ok(())
    }

    pub fn is_healthy(&self) -> bool {
        let heartbeats = self.pds_heartbeats.read().unwrap();

        if heartbeats.is_empty() {
            return false;
        }

        heartbeats
            .values()
            .all(|&last_hb| last_hb.elapsed() <= Self::PDS_TIMEOUT_LIMIT)
    }

    /// DMX refresh loop
    fn run(self, socket: &mut UdpSocket, targets: &HashMap<PdsKey, KinetPowerSupply>) {
        let mut broadcaster = DmxBroadcaster::new(socket, targets, self.config.num_arcs);

        let mut next_tick_time = Instant::now();
        let mut timing = FrameTiming::calculate(self.config.refresh_rate_ms, StageMode::Demo, None);

        let mut sub_tick_counter = 0;
        let mut current_frame = LightStageFrame::black(self.config.num_arcs);
        let mut pending_camera_trigger = false;

        loop {
            // only advance animation tick every k network packets
            if sub_tick_counter == 0 {
                // update current_frame_data and get mode, result.
                let (tick_result, mode, capture_hz) = {
                    let mut lock = self.state.write().unwrap();
                    let result = lock.advance_tick(&mut current_frame);
                    let hz = lock.capture_hz();
                    (result, lock.mode(), hz)
                };

                // set synced refresh rate
                timing = FrameTiming::calculate(self.config.refresh_rate_ms, mode, capture_hz);

                // TODO fire cameras from the last frame before we send the new frame
                if pending_camera_trigger {
                    // hopefully this is enough time for the fixtures to turn on
                    thread::sleep(Duration::from_millis(4));
                    // TODO gpio
                }

                pending_camera_trigger = tick_result == TickResult::TriggerCapture;
            }

            sub_tick_counter += 1;
            if sub_tick_counter >= timing.sub_ticks_per_frame {
                sub_tick_counter = 0;
            }

            broadcaster.broadcast_frame(&current_frame);

            next_tick_time += timing.tick_duration;
            let now = Instant::now();
            if next_tick_time > now {
                thread::sleep(next_tick_time - now);
            } else {
                let lateness = now.duration_since(
                    next_tick_time
                        .checked_sub(timing.tick_duration)
                        .unwrap_or(now),
                );
                warn!(
                    "oops. frame took {lateness:?} (Target was {:?})",
                    timing.tick_duration
                );
                next_tick_time = now;
            }
        }
    }

    fn spawn_heartbeat_monitor(&self) {
        let rx_port = self.config.heartbeat_port;

        let hbs_rx = self.pds_heartbeats.clone();
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
                        {
                            let mut lock = hbs_rx.write().unwrap();
                            if let Some(last_hb) = lock.get_mut(&hb.serial) {
                                *last_hb = Instant::now();
                            }
                        }
                    }
                }
            }
        });

        // watchdog thread
        let hbs_wd = self.pds_heartbeats.clone();
        thread::spawn(move || {
            let mut offline = HashSet::new();

            loop {
                thread::sleep(Duration::from_secs(5));
                let heartbeats = hbs_wd.read().unwrap();

                for (&serial, &last_hb) in heartbeats.iter() {
                    let elapsed = last_hb.elapsed();
                    if elapsed > Self::PDS_TIMEOUT_LIMIT {
                        if offline.insert(serial) {
                            error!(
                                "Lost communication with PDS {serial:X}! No heartbeats received for over {:?}.",
                                Self::PDS_TIMEOUT_LIMIT
                            );
                        }
                    } else if offline.remove(&serial) {
                        info!("PDS {serial:X} has reconnected.");
                    }
                }
            }
        });
    }
}
