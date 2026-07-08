use std::{
    collections::HashMap,
    io::Cursor,
    net::{Ipv4Addr, SocketAddr, UdpSocket},
    time::{Duration, Instant},
};

use kinetrs::{KinetPacketHeader, KinetPayload, PollPayload, PollReplyPayload};

/// One of our discovered PDS on the network.
pub struct KinetPowerSupply {
    pub remote_adr: SocketAddr,
    pub arc_index: usize,
    pub is_rgb: bool,
}

/// Discover PDS on the network with [`kinetrs::KinetPacketHeader::Poll`] and listen for replies.
pub fn discover_pds(port: u16) -> anyhow::Result<Vec<KinetPowerSupply>> {
    let socket = UdpSocket::bind("10.37.23.200:0")?;
    let _ = socket.set_broadcast(true);
    let _ = socket.set_read_timeout(Some(Duration::from_millis(100)));

    let poll_packet = KinetPacketHeader::Poll(PollPayload {
        magic_ip: Ipv4Addr::new(10, 37, 1, 1), // this cannot be 0.0.0.0 or 255.255.255.255 because the OS ignores it otherwise
        ..Default::default()
    });
    let mut buf = Vec::new();
    let _ = poll_packet.write_to(&mut buf);

    socket.send_to(&buf, SocketAddr::new(Ipv4Addr::BROADCAST.into(), port))?;

    let mut discovered_targets = Vec::new();

    let mut buf = [0u8; PollReplyPayload::PACKET_SIZE];

    let start_time = Instant::now();

    while start_time.elapsed() < Duration::from_secs(2) {
        if let Ok((size, _src)) = socket.recv_from(&mut buf) {
            match KinetPacketHeader::read_from(&mut Cursor::new(&mut buf[..size])) {
                Ok(packet) => match packet {
                    KinetPacketHeader::PollReply(reply) => {
                        println!(
                            "Found PDS {:X} at {}. Label: {}",
                            reply.serial,
                            reply.src_ip,
                            reply.node_label_as_str().unwrap()
                        );

                        // TODO rewrite this shit
                        let label = reply.node_label_as_str().unwrap_or_default();
                        let label_parts: Vec<&str> = label.split_whitespace().collect();
                        if label_parts.len() == 2 {
                            let identifier = label_parts[1];
                            let is_rgb = identifier.ends_with('C');
                            let is_white = identifier.ends_with('W');
                            if is_rgb || is_white {
                                let num_str = &identifier[..identifier.len() - 1];
                                if let Ok(arc_index) = num_str.parse::<usize>() {
                                    let addr = SocketAddr::new(reply.src_ip.into(), port);
                                    discovered_targets.push(KinetPowerSupply {
                                        remote_adr: addr,
                                        arc_index,
                                        is_rgb,
                                    });
                                }
                            }
                        }
                    }
                    _ => todo!(), // and maybe handle errors correctly?
                },
                Err(_) => todo!(),
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
) -> std::collections::HashMap<(usize, bool), std::net::SocketAddr> {
    let mut mapped = HashMap::new();

    for pds in raw_targets {
        mapped.insert((pds.arc_index, pds.is_rgb), pds.remote_adr);
    }

    mapped
}
