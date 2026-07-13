use std::{
    collections::HashMap,
    io::Cursor,
    net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket},
    time::{Duration, Instant},
};

use anyhow::anyhow;
use kinetrs::{KinetPacketHeader, KinetPayload, PollPayload, PollReplyPayload};
use tracing::{debug, warn};

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
