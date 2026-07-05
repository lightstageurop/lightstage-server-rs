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

use std::{
    io::{self, Write},
    net::Ipv4Addr,
};

/// Default target UDP port
pub const KINET_UDP_PORT: u16 = 6038;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
enum KinetPacketType {
    Poll = 0x0001,      // DiscoverSupplies
    PollReply = 0x0002, // DiscoverSuppliesReply
    SetIp = 0x0003,
    SetUniverse = 0x0005,
    SetName = 0x0006,
    DmxOut = 0x0101,
    // PortOut = 0x0108,
    // PortOutSync = 0x0109,
    DiscoverFixturesSerialRequest = 0x0201,
    DiscoverFixturesChannelRequest = 0x0203, // get dmx address
}

/// Payload that can be serialised into a KiNET packet.
pub trait KinetPayload {
    /// Serialised byte length of this payload
    const SIZE: usize;

    /// Serialised byte length of the entire packet
    ///
    /// For [`KinetPacketHeader::DmxOut`], this **does not** include the DMX512 data.
    const PACKET_SIZE: usize = KinetPacketHeader::HEADER_SIZE + Self::SIZE;

    /// Serialise the payload into writer
    fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()>;
}

/// Payload for [`KinetPacketHeader::Poll`]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PollPayload {
    /// Sequence. Appears to be unused and always zero.
    pub sequence: u32,
    /// Unsure what the use of this is. Devices will spoof their source IP to this.
    pub magic_ip: Ipv4Addr,
}

impl Default for PollPayload {
    fn default() -> Self {
        Self {
            sequence: 0,
            magic_ip: Ipv4Addr::UNSPECIFIED,
        }
    }
}

impl KinetPayload for PollPayload {
    const SIZE: usize = 10;

    fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        writer.write_all(&self.sequence.to_le_bytes())?;
        writer.write_all(&self.magic_ip.octets())?;
        writer.write_all(&[0u8; 2])?; // reserved
        Ok(())
    }
}

/// Payload for [`KinetPacketHeader::PollReply`]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PollReplyPayload {
    /// Sequence. Appears to be unused and always zero.
    pub sequence: u32,
    /// Device IPv4 address
    pub src_ip: Ipv4Addr,
    /// Device MAC address
    pub mac: [u8; 6],
    /// Unknown field. Observed as `0x0001` usually.
    pub data: u16,
    /// Device serial number
    pub serial: u32,
    /// Null-padded ASCII device description string
    pub node_name: [u8; 60],
    /// Null-padded ASCII user-visible device label
    pub node_label: [u8; 31],
}

impl Default for PollReplyPayload {
    fn default() -> Self {
        Self {
            src_ip: Ipv4Addr::UNSPECIFIED,
            node_name: [0u8; 60],
            node_label: [0u8; 31],
            sequence: Default::default(),
            mac: Default::default(),
            data: Default::default(),
            serial: Default::default(),
        }
    }
}

impl KinetPayload for PollReplyPayload {
    const SIZE: usize = 117;

    fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        writer.write_all(&self.sequence.to_le_bytes())?;
        writer.write_all(&self.src_ip.octets())?;
        writer.write_all(&self.mac)?;
        writer.write_all(&self.data.to_le_bytes())?;
        writer.write_all(&self.serial.to_le_bytes())?;
        writer.write_all(&[0u8; 4])?;
        writer.write_all(&self.node_name)?;
        writer.write_all(&self.node_label)?;
        writer.write_all(&[0u8; 2])?;
        Ok(())
    }
}

/// Payload for [`KinetPacketHeader::DmxOut`]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DmxOutHeader {
    /// Packet sequence number. Usually ignored by hardware.
    pub sequence: u32,
    /// DMX output port index
    ///
    /// Seemingly only used for v2 in broadcast environment
    /// See <https://colorkinetics.helpdocs.io/article/umxjxmoc7a-ki-net-ethernet-protocol-whitepaper#ki_net_universes>
    pub port: u8,
    /// Unsure what this does, seems to always be zero
    pub flags: u16,
    /// Unsure what this does. Usually zero or `u32::MAX`
    pub timer_val: u32,
    /// DMX universe id to target. Rarely used aside from broadcast configurations.
    pub universe: u8,
}

impl Default for DmxOutHeader {
    fn default() -> Self {
        Self {
            sequence: 0,
            port: 0, // always zero for v1?
            flags: 0,
            timer_val: u32::MAX, // TODO test this. kinet.py uses u32::MAX, OLA uses 0u32
            universe: 0,
        }
    }
}

impl KinetPayload for DmxOutHeader {
    const SIZE: usize = 13;

    fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        writer.write_all(&self.sequence.to_le_bytes())?;
        writer.write_all(&[self.port])?;
        writer.write_all(&[0u8])?; // padding byte
        writer.write_all(&self.flags.to_le_bytes())?;
        writer.write_all(&self.timer_val.to_le_bytes())?;
        writer.write_all(&[self.universe])?;
        Ok(())
    }
}

/// Serialisable packets
///
/// For `DmxOut` does not include DMX512 data
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KinetPacketHeader {
    /// A poll packet to scan local subnet for active power supplies.
    ///
    /// Aka. `DiscoverSupplies`
    Poll(PollPayload),
    /// A response to [`Self::Poll`], emitted by a power supply.
    ///
    /// Aka. `DiscoverSuppliesReply`
    PollReply(PollReplyPayload),
    /// The header only for a DMX512 packet streamed to power supply(ies).
    ///
    /// DMX512 data should be appended directly to the serialised header.
    DmxOut(DmxOutHeader),
}

impl KinetPacketHeader {
    const KINET_MAGIC: u32 = 0x0401_DC4A;
    const KINET_VERSION_1: u16 = 0x0001;
    const HEADER_SIZE: usize = 8;

    fn kind(&self) -> KinetPacketType {
        match self {
            Self::Poll(_) => KinetPacketType::Poll,
            Self::PollReply(_) => KinetPacketType::PollReply,
            Self::DmxOut(_) => KinetPacketType::DmxOut,
        }
    }

    /// Overall buffer length neccessary for serialised packet
    ///
    /// For [`Self::DmxOut`] this **does** include the 512 bytes of DMX data.
    pub fn packet_size(&self) -> usize {
        Self::HEADER_SIZE
            + match self {
                Self::Poll(_) => PollPayload::SIZE,
                Self::PollReply(_) => PollReplyPayload::SIZE,
                Self::DmxOut(_) => DmxOutHeader::SIZE + 512,
            }
    }

    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        // Magic is left as BE,
        // the rest of the protocol appears to be LE
        writer.write_all(&Self::KINET_MAGIC.to_be_bytes())?;
        writer.write_all(&Self::KINET_VERSION_1.to_le_bytes())?;
        writer.write_all(&(self.kind() as u16).to_le_bytes())?;

        match self {
            Self::Poll(payload) => payload.write_to(writer),
            Self::PollReply(payload) => payload.write_to(writer),
            Self::DmxOut(payload) => payload.write_to(writer),
        }?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    fn assert_payload_size<T: KinetPayload + Default>(name: &str) {
        let mut buf = Vec::new();
        T::default().write_to(&mut buf).unwrap();
        assert_eq!(
            buf.len(),
            T::SIZE,
            "Payload type {name} wire size constant mismatched!"
        );
    }

    #[test]
    fn test_default_values() {
        let header = KinetPacketHeader::DmxOut(DmxOutHeader::default());
        if let KinetPacketHeader::DmxOut(DmxOutHeader { timer_val, .. }) = header {
            assert_eq!(timer_val, u32::MAX);
        } else {
            panic!("Default KinetHeader not DmxOut!")
        }
    }

    #[test]
    fn test_payload_sizes_match_constants() {
        assert_payload_size::<PollPayload>("PollPayload");
        assert_payload_size::<PollReplyPayload>("PollReplyPayload");
        assert_payload_size::<DmxOutHeader>("DmxOutHeader");
    }

    #[test]
    fn test_to_bytes_discover_supplies() {
        let header = KinetPacketHeader::Poll(PollPayload {
            sequence: 42,
            magic_ip: Ipv4Addr::new(10, 1, 1, 222),
        });

        // Header(8) + Seq(4) + IP(4) + Reserved(2) = 18 bytes
        let mut buf = [0u8; 18];
        header.write_to(&mut Cursor::new(&mut buf[..])).unwrap();

        // Expected byte array
        let expected: [u8; 18] = [
            0x04, 0x01, 0xDC, 0x4A, // Magic
            0x01, 0x00, // Version 1
            0x01, 0x00, // DiscoverSupplies
            0x2A, 0x00, 0x00, 0x00, // Sequence
            0x0a, 0x01, 0x01, 0xde, // IP: 10.1.1.222
            0x00, 0x00, // reserved
        ];

        assert_eq!(
            buf, expected,
            "DiscoverSupplies serialized bytes do not match expected layout"
        );
    }

    #[test]
    fn test_to_bytes_discover_supplies_reply() {
        let mut node_name = [0u8; 60];
        let name_str = b"Generic Power Supply Name";
        node_name[..name_str.len()].copy_from_slice(name_str);

        let mut node_label = [0u8; 31];
        let label_str = b"Generic Power Supply Label";
        node_label[..label_str.len()].copy_from_slice(label_str);

        let header = KinetPacketHeader::PollReply(PollReplyPayload {
            sequence: 42,
            src_ip: Ipv4Addr::new(10, 1, 2, 3),
            mac: [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff],
            data: 0x0001,
            serial: 0x1234_5678,
            node_name,
            node_label,
        });

        // Header(8) + Seq(4) + IP(4) + MAC(6) + Data(2) + Serial(4) + Res32(4) + Name(60) + Label(31) + Pad(2) = 125 bytes
        let mut buf = [0u8; 125];
        header.write_to(&mut Cursor::new(&mut buf[..])).unwrap();

        assert_eq!(
            &buf[0..8],
            &[0x04, 0x01, 0xDC, 0x4A, 0x01, 0x00, 0x02, 0x00],
            "Header mismatch"
        );
        assert_eq!(&buf[8..12], &[0x2a, 0x00, 0x00, 0x00], "Sequence mismatch");
        assert_eq!(&buf[12..16], &[10, 1, 2, 3], "IP mismatch");
        assert_eq!(
            &buf[16..22],
            &[0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff],
            "MAC mismatch"
        );
        assert_eq!(&buf[22..24], &[0x01, 0x00], "Data mismatch");
        assert_eq!(&buf[24..28], &[0x78, 0x56, 0x34, 0x12], "Serial mismatch");
        assert_eq!(
            &buf[28..32],
            &[0x00, 0x00, 0x00, 0x00],
            "Reserved32 mismatch"
        );
        assert_eq!(
            &buf[32..32 + name_str.len()],
            name_str,
            "Node Name mismatch"
        );
        assert_eq!(
            &buf[92..92 + label_str.len()],
            label_str,
            "Node Label mismatch"
        );
        assert_eq!(&buf[123..125], &[0x00, 0x00], "Padding byte mismatch");
    }

    #[test]
    fn test_to_bytes_discover_supplies_reply_real_world_snapshot() {
        // Exact packet bytes from pcap
        let exact_payload: [u8; 125] = [
            0x04, 0x01, 0xDC, 0x4A, 0x01, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0a, 0x25,
            0xd3, 0x03, 0x00, 0x0a, 0xc5, 0x25, 0xd3, 0x43, 0x01, 0x00, 0x9b, 0x18, 0x00, 0x3d,
            0x00, 0x00, 0x00, 0x00, 0x4d, 0x3a, 0x43, 0x6f, 0x6c, 0x6f, 0x72, 0x20, 0x4b, 0x69,
            0x6e, 0x65, 0x74, 0x69, 0x63, 0x73, 0x20, 0x49, 0x6e, 0x63, 0x6f, 0x72, 0x70, 0x6f,
            0x72, 0x61, 0x74, 0x65, 0x64, 0x0a, 0x44, 0x3a, 0x50, 0x44, 0x53, 0x2d, 0x58, 0x0a,
            0x23, 0x3a, 0x53, 0x46, 0x54, 0x2d, 0x30, 0x30, 0x30, 0x30, 0x38, 0x30, 0x2d, 0x30,
            0x30, 0x0a, 0x52, 0x3a, 0x30, 0x32, 0x0a, 0x00, 0x41, 0x72, 0x63, 0x20, 0x31, 0x57,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];

        let mut node_name = [0u8; 60];
        let name_str = b"M:Color Kinetics Incorporated\nD:PDS-X\n#:SFT-000080-00\nR:02\n";
        node_name[..name_str.len()].copy_from_slice(name_str);

        let mut node_label = [0u8; 31];
        let label_str = b"Arc 1W";
        node_label[..label_str.len()].copy_from_slice(label_str);

        let header = KinetPacketHeader::PollReply(PollReplyPayload {
            sequence: 0,
            src_ip: Ipv4Addr::new(10, 37, 211, 3),
            mac: [0x00, 0x0a, 0xc5, 0x25, 0xd3, 0x43],
            data: 0x0001,
            serial: 0x3D00_189B,
            node_name,
            node_label,
        });

        let mut buf = [0u8; 125];
        header.write_to(&mut Cursor::new(&mut buf[..])).unwrap();

        assert_eq!(
            buf, exact_payload,
            "Real-world DiscoverSuppliesReply snapshot failed to match!"
        );
    }

    #[test]
    fn test_to_bytes_dmx_out() {
        let header = KinetPacketHeader::DmxOut(DmxOutHeader {
            sequence: 128,
            port: 0,
            flags: 0,
            timer_val: u32::MAX,
            universe: 0,
        });

        let mut buf = [0u8; 21];
        header.write_to(&mut Cursor::new(&mut buf[..])).unwrap();

        // Expected byte array for DmxOut
        let expected: [u8; 21] = [
            0x04, 0x01, 0xDC, 0x4A, // Magic
            0x01, 0x00, // Version 1
            0x01, 0x01, // DmxOut
            0x80, 0x00, 0x00, 0x00, // Sequence
            0x00, // Port
            0x00, // padding
            0x00, 0x00, // Flags
            0xFF, 0xFF, 0xFF, 0xFF, // Timer
            0x00, // Universe
        ];

        assert_eq!(
            buf, expected,
            "DmxOut serialized bytes do not match expected layout"
        );
    }
}
