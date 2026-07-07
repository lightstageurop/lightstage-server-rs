use std::io::{self, Write};

use num_enum::TryFromPrimitive;

use crate::payload::{DmxOutHeader, HeartBeatPayload, KinetPayload, PollPayload, PollReplyPayload};

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive)]
#[repr(u16)]
enum KinetPacketType {
    Poll = 0x0001,      // DiscoverSupplies
    PollReply = 0x0002, // DiscoverSuppliesReply
    SetIp = 0x0003,
    SetUniverse = 0x0005,
    SetName = 0x0006,
    HeartBeat = 0x0008,
    DmxOut = 0x0101,
    // PortOut = 0x0108,
    // PortOutSync = 0x0109,
    DiscoverFixturesSerialRequest = 0x0201,
    DiscoverFixturesSerialReply = 0x0202,
    DiscoverFixturesChannelRequest = 0x0203, // get dmx address
}

/// Serialisable packets
///
/// For `DmxOut` does not include DMX512 data
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KinetPacketHeader {
    /// A poll packet to scan local subnet for active power supplies.
    ///
    /// Aka. `DiscoverSupplies`
    Poll(PollPayload),
    /// A response to [`Self::Poll`], emitted by a power supply.
    ///
    /// Aka. `DiscoverSuppliesReply`
    PollReply(PollReplyPayload),
    /// A heartbeat sent out by each power supply every 90s.
    HeartBeat(HeartBeatPayload),
    /// The header only for a DMX512 packet streamed to power supply(ies).
    ///
    /// DMX512 data should be appended directly to the serialised header.
    DmxOut(DmxOutHeader),
}

impl KinetPacketHeader {
    const KINET_MAGIC: u32 = 0x0401_DC4A;
    const KINET_VERSION_1: u16 = 0x0001;
    pub const HEADER_SIZE: usize = 8;

    fn kind(&self) -> KinetPacketType {
        match self {
            Self::Poll(_) => KinetPacketType::Poll,
            Self::PollReply(_) => KinetPacketType::PollReply,
            Self::HeartBeat(_) => KinetPacketType::HeartBeat,
            Self::DmxOut(_) => KinetPacketType::DmxOut,
        }
    }

    /// Overall buffer length neccessary for serialised packet
    ///
    /// For [`Self::DmxOut`] this **does** include the 512 bytes of DMX data.
    #[must_use]
    pub fn packet_size(&self) -> usize {
        Self::HEADER_SIZE
            + match self {
                Self::Poll(_) => PollPayload::SIZE,
                Self::PollReply(_) => PollReplyPayload::SIZE,
                Self::HeartBeat(_) => HeartBeatPayload::SIZE,
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
            Self::HeartBeat(payload) => payload.write_to(writer),
            Self::DmxOut(payload) => payload.write_to(writer),
        }?;

        Ok(())
    }

    pub fn read_from<R: io::Read>(reader: &mut R) -> io::Result<Self> {
        let mut magic_bytes = [0u8; 4];
        let mut version_bytes = [0u8; 2];
        let mut kind_bytes = [0u8; 2];

        reader.read_exact(&mut magic_bytes)?;
        reader.read_exact(&mut version_bytes)?;
        reader.read_exact(&mut kind_bytes)?;

        // validate magic
        let magic = u32::from_be_bytes(magic_bytes);
        if magic != Self::KINET_MAGIC {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Invalid KiNET magic: {magic:#0X}"),
            ));
        }

        // validate version
        let version = u16::from_le_bytes(version_bytes);
        if version != Self::KINET_VERSION_1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Unsupported KiNET version: {version}"),
            ));
        }

        // parse type
        let kind_value = u16::from_le_bytes(kind_bytes);
        let kind = KinetPacketType::try_from(kind_value).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Unknown KiNET packet type identifier: {kind_value:#06X}"),
            )
        })?;

        match kind {
            KinetPacketType::Poll => Ok(Self::Poll(PollPayload::read_from(reader)?)),
            KinetPacketType::PollReply => Ok(Self::PollReply(PollReplyPayload::read_from(reader)?)),
            KinetPacketType::HeartBeat => Ok(Self::HeartBeat(HeartBeatPayload::read_from(reader)?)),
            KinetPacketType::DmxOut => Ok(Self::DmxOut(DmxOutHeader::read_from(reader)?)),
            unhandled => Err(io::Error::new(
                io::ErrorKind::Unsupported,
                format!("KiNET packet type {unhandled:?} is recognised, but unimplemented."),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{io::Cursor, net::Ipv4Addr};

    use super::*;

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

        let mut node_label = [0u8; 33];
        let label_str = b"Generic Power Supply Label";
        node_label[..label_str.len()].copy_from_slice(label_str);

        let header = KinetPacketHeader::PollReply(PollReplyPayload {
            sequence: 42,
            src_ip: Ipv4Addr::new(10, 1, 2, 3),
            mac: [0x00, 0x0a, 0xc5, 0xdd, 0xee, 0xff],
            data: 0x0001,
            serial: 0x3D00_5678,
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
            &[0x00, 0x0a, 0xc5, 0xdd, 0xee, 0xff],
            "MAC mismatch"
        );
        assert_eq!(&buf[22..24], &[0x01, 0x00], "Data mismatch");
        assert_eq!(&buf[24..28], &[0x78, 0x56, 0x00, 0x3D], "Serial mismatch");
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

        let mut node_label = [0u8; 33];
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
    fn test_to_bytes_heartbeat() {
        let packet = KinetPacketHeader::HeartBeat(HeartBeatPayload {
            src_ip: Ipv4Addr::new(10, 37, 1, 2),
            mac: [0x00, 0x0A, 0xC5, 0x12, 0x34, 0x56],
            serial: 0x3D00_FEDC,
            ..Default::default()
        });

        let mut buf = Vec::new();
        packet.write_to(&mut buf).unwrap();

        assert_eq!(
            &buf[0..8],
            &[0x04, 0x01, 0xDC, 0x4A, 0x01, 0x00, 0x08, 0x00],
            "Header mismatch"
        );
        assert_eq!(&buf[8..12], &[0x00, 0x00, 0x00, 0x00], "Sequence mismatch");
        assert_eq!(&buf[12..16], &[10, 37, 1, 2], "IP mismatch");
        assert_eq!(
            &buf[16..22],
            &[0x00, 0x0a, 0xc5, 0x12, 0x34, 0x56],
            "MAC mismatch"
        );
        assert_eq!(&buf[22..24], &[0x01, 0x00], "Data16 mismatch");
        assert_eq!(&buf[24..28], &[0xdc, 0xfe, 0x00, 0x3d], "Serial mismatch");
        assert_eq!(&buf[28..32], &[0x01, 0x00, 0x03, 0x00], "Data32 mismatch");
        assert_eq!(buf.len(), 32);
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
