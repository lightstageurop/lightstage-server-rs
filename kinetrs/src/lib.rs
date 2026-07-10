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

use std::io;

pub use packet::{KinetPacketHeader, KinetPacketType};
pub use payload::{DmxOutHeader, HeartBeatPayload, KinetPayload, PollPayload, PollReplyPayload};
use thiserror::Error;

/// Default target UDP port
pub const KINET_UDP_PORT: u16 = 6038;

/// Errors that can occur from deserialisation or conversions.
#[derive(Debug, Error)]
pub enum KinetError {
    /// An IO error.
    #[error("IO Error: {0}")]
    Io(#[from] io::Error),

    /// Packet header does not start with mandatory magic sequence.
    #[error("Invalid KiNET magic: {0:#0X}")]
    InvalidMagic(u32),

    /// Specified a protocol version other than 1 (`0x0001`).
    #[error("Unsupported KiNET version: {0}")]
    UnsupportedVersion(u16),

    /// Received a packet type code that is unrecognised.
    #[error("Unknown KiNET packet type identifier: {0:#06X}")]
    UnknownPacketType(u16),

    /// Valid packet code detected, but parsing hasn't been implemented by this library yet.
    #[error("KiNET packet type {0:?} is recognised, but unimplemented.")]
    UnimplementedPacketType(KinetPacketType),

    /// A type conversion failed ([`TryFrom`]) because we held a different variant than requested.
    #[error("Mismatched packet types: expected {expected:?}, got {actual:?}")]
    MismatchedPacketType {
        expected: KinetPacketType,
        actual: KinetPacketType,
    },
}
