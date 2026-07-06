//! # `KiNETrs` - Color Kinetics protocol
//!
//! This crate provides serialisation for a subset of the `KiNET` (Philips Color Kinetics) v1 UDP protocol, including:
//! - Power supply discovery ([`PollPayload`], [`PollReplyPayload`])
//! - Dmx Output ([`DmxOutHeader`])
//!
//! ## Endianness
//!
//! The protocol appears to be primarily little-endian for most fields,
//! however for the DMX data, most fixtures that use multiple bytes per channel are big-endian.
//!
//! ## Example
//!
//! ```rust
//! use kinetrs::{KinetPacketHeader, DmxOutHeader};
//!
//! let dmx_meta = DmxOutHeader {
//!     sequence: 42,
//!     ..Default::default()
//! };
//! let packet = KinetPacketHeader::DmxOut(dmx_meta);
//!
//! let mut buf = Vec::with_capacity(packet.packet_size());
//! packet.write_to(&mut buf).unwrap();
//!
//! let fixture_data = [255u8; 512]; // Full intensity
//! buf.extend_from_slice(&fixture_data);
//!
//! // Ready to send via std::net::UdpSocket!
//!
//! # assert_eq!(buf.len(), 533)
//! ```

mod fixtures;
mod packet;
mod payload;

pub use packet::KinetPacketHeader;
pub use payload::{DmxOutHeader, HeartBeatPayload, KinetPayload, PollPayload, PollReplyPayload};

/// Default target UDP port
pub const KINET_UDP_PORT: u16 = 6038;
