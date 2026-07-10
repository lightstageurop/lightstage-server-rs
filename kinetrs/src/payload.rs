use std::{
    ffi::CStr,
    io::{self, Read, Write},
    net::Ipv4Addr,
};

use crate::KinetPacketHeader;

/// Payload that can be serialised into a `KiNET` packet.
pub trait KinetPayload {
    /// Serialised byte length of this payload
    const SIZE: usize;

    /// Serialised byte length of the entire packet
    ///
    /// For [`KinetPacketHeader::DmxOut`], this **does not** include the DMX512 data.
    const PACKET_SIZE: usize = KinetPacketHeader::HEADER_SIZE + Self::SIZE;

    /// Serialise the payload into writer
    fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()>;

    /// Deserialise
    fn read_from<R: Read>(reader: &mut R) -> io::Result<Self>
    where
        Self: Sized;
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

    fn read_from<R: Read>(reader: &mut R) -> io::Result<Self>
    where
        Self: Sized,
    {
        let mut seq_bytes = [0u8; 4];
        let mut ip_bytes = [0u8; 4];
        let mut reserved = [0u8; 2];

        reader.read_exact(&mut seq_bytes)?;
        reader.read_exact(&mut ip_bytes)?;
        reader.read_exact(&mut reserved)?;
        if reserved != [0u8; 2] {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "reserved bytes must be zero",
            ));
        }

        Ok(Self {
            sequence: u32::from_le_bytes(seq_bytes),
            magic_ip: Ipv4Addr::from(ip_bytes),
        })
    }
}

/// Payload for [`KinetPacketHeader::PollReply`]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    /// Null-padded ASCII user-visible device label.
    ///
    /// Should be null terminated.
    pub node_label: [u8; 33],
}

impl Default for PollReplyPayload {
    fn default() -> Self {
        Self {
            src_ip: Ipv4Addr::UNSPECIFIED,
            node_name: [0u8; 60],
            node_label: [0u8; 33],
            sequence: Default::default(),
            mac: Default::default(),
            data: Default::default(),
            serial: Default::default(),
        }
    }
}

impl PollReplyPayload {
    #[must_use]
    pub fn with_name(mut self, name: &str) -> Option<Self> {
        if !name.is_ascii() || name.len() >= self.node_name.len() {
            return None;
        }
        self.node_name.fill(0u8);
        let bytes = name.as_bytes();
        self.node_name[..bytes.len()].copy_from_slice(bytes);
        Some(self)
    }

    #[must_use]
    pub fn with_label(mut self, label: &str) -> Option<Self> {
        if !label.is_ascii() || label.len() >= self.node_label.len() {
            return None;
        }
        self.node_label.fill(0u8);
        let bytes = label.as_bytes();
        self.node_label[..bytes.len()].copy_from_slice(bytes);
        Some(self)
    }

    #[must_use]
    pub fn node_name_as_str(&self) -> Option<&str> {
        CStr::from_bytes_until_nul(&self.node_name)
            .ok()
            .and_then(|cstr| cstr.to_str().ok())
    }

    #[must_use]
    pub fn node_label_as_str(&self) -> Option<&str> {
        CStr::from_bytes_until_nul(&self.node_label)
            .ok()
            .and_then(|cstr| cstr.to_str().ok())
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
        Ok(())
    }

    fn read_from<R: Read>(reader: &mut R) -> io::Result<Self>
    where
        Self: Sized,
    {
        let mut seq_bytes = [0u8; 4];
        let mut src_ip = [0u8; 4];
        let mut mac = [0u8; 6];
        let mut data = [0u8; 2];
        let mut serial = [0u8; 4];
        let mut reserved = [0u8; 4];
        let mut node_name = [0u8; 60];
        let mut node_label = [0u8; 33];

        reader.read_exact(&mut seq_bytes)?;
        reader.read_exact(&mut src_ip)?;
        reader.read_exact(&mut mac)?;
        reader.read_exact(&mut data)?;
        reader.read_exact(&mut serial)?;
        reader.read_exact(&mut reserved)?;
        reader.read_exact(&mut node_name)?;
        reader.read_exact(&mut node_label)?;

        Ok(Self {
            sequence: u32::from_le_bytes(seq_bytes),
            src_ip: Ipv4Addr::from(src_ip),
            mac,
            data: u16::from_le_bytes(data),
            serial: u32::from_le_bytes(serial),
            node_name,
            node_label,
        })
    }
}

/// Payload for [`KinetPacketHeader::HeartBeat`]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeartBeatPayload {
    /// Sequence. Appears to be unused and always zero.
    pub sequence: u32,
    /// Device IPv4 address
    pub src_ip: Ipv4Addr,
    /// Device MAC address
    pub mac: [u8; 6],
    /// Unknown field. Observed as `0x0001` usually.
    pub data16: u16,
    /// Device serial number
    pub serial: u32,
    /// Unknown field. Observed as `0x00030001` usually.
    pub data32: u32,
}

impl Default for HeartBeatPayload {
    fn default() -> Self {
        Self {
            sequence: 0,
            data16: 0x0001,
            src_ip: Ipv4Addr::UNSPECIFIED,
            mac: Default::default(),
            serial: Default::default(),
            data32: 0x0003_0001,
        }
    }
}

impl KinetPayload for HeartBeatPayload {
    const SIZE: usize = 24;

    fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        writer.write_all(&self.sequence.to_le_bytes())?;
        writer.write_all(&self.src_ip.octets())?;
        writer.write_all(&self.mac)?;
        writer.write_all(&self.data16.to_le_bytes())?;
        writer.write_all(&self.serial.to_le_bytes())?;
        writer.write_all(&self.data32.to_le_bytes())?;
        Ok(())
    }

    fn read_from<R: Read>(reader: &mut R) -> io::Result<Self>
    where
        Self: Sized,
    {
        let mut seq_bytes = [0u8; 4];
        let mut ip_bytes = [0u8; 4];
        let mut mac = [0u8; 6];
        let mut data16_bytes = [0u8; 2];
        let mut serial_bytes = [0u8; 4];
        let mut data32_bytes = [0u8; 4];

        reader.read_exact(&mut seq_bytes)?;
        reader.read_exact(&mut ip_bytes)?;
        reader.read_exact(&mut mac)?;
        reader.read_exact(&mut data16_bytes)?;
        reader.read_exact(&mut serial_bytes)?;
        reader.read_exact(&mut data32_bytes)?;

        Ok(Self {
            sequence: u32::from_le_bytes(seq_bytes),
            src_ip: Ipv4Addr::from(ip_bytes),
            mac,
            data16: u16::from_le_bytes(data16_bytes),
            serial: u32::from_le_bytes(serial_bytes),
            data32: u32::from_le_bytes(data32_bytes),
        })
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

    fn read_from<R: Read>(reader: &mut R) -> io::Result<Self>
    where
        Self: Sized,
    {
        let mut seq_bytes = [0u8; 4];
        let mut port_byte = [0u8; 1];
        let mut padding_byte = [0u8; 1];
        let mut flags_bytes = [0u8; 2];
        let mut timer_bytes = [0u8; 4];
        let mut universe_byte = [0u8; 1];

        reader.read_exact(&mut seq_bytes)?;
        reader.read_exact(&mut port_byte)?;
        reader.read_exact(&mut padding_byte)?;
        reader.read_exact(&mut flags_bytes)?;
        reader.read_exact(&mut timer_bytes)?;
        reader.read_exact(&mut universe_byte)?;

        Ok(Self {
            sequence: u32::from_le_bytes(seq_bytes),
            port: port_byte[0],
            flags: u16::from_le_bytes(flags_bytes),
            timer_val: u32::from_le_bytes(timer_bytes),
            universe: universe_byte[0],
        })
    }
}

#[cfg(test)]
mod tests {
    use std::fmt::Debug;

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

    fn assert_roundtrip<T: KinetPayload + PartialEq + Debug>(original: T) {
        let mut write_buf = Vec::new();
        original.write_to(&mut write_buf).unwrap();

        let mut read_cursor = &write_buf[..];
        let deserialised =
            T::read_from(&mut read_cursor).expect("Failed to deserialise serialised packet!");

        assert_eq!(original, deserialised, "Round-trip packets mismatched!");
        assert!(
            read_cursor.is_empty(),
            "Deserialisation did not consume all bytes of serialised data!"
        );
    }

    #[test]
    fn test_payload_sizes_match_constants() {
        assert_payload_size::<PollPayload>("PollPayload");
        assert_payload_size::<PollReplyPayload>("PollReplyPayload");
        assert_payload_size::<HeartBeatPayload>("HeartBeatPayload");
        assert_payload_size::<DmxOutHeader>("DmxOutHeader");
    }

    #[test]
    fn test_poll_roundtrip() {
        assert_roundtrip(PollPayload {
            sequence: 4242,
            magic_ip: Ipv4Addr::new(10, 37, 1, 2),
        });
    }

    #[test]
    fn test_pollreply_roundtrip() {
        let mut node_name = [0u8; 60];
        let name_str = b"Generic Power Supply Name";
        node_name[..name_str.len()].copy_from_slice(name_str);

        let mut node_label = [0u8; 33];
        let label_str = b"Generic Power Supply Label";
        node_label[..label_str.len()].copy_from_slice(label_str);

        assert_roundtrip(PollReplyPayload {
            sequence: 0,
            src_ip: Ipv4Addr::new(10, 37, 3, 4),
            mac: [0x00, 0x0a, 0xc5, 0x65, 0x43, 0x21],
            data: 0x0001,
            serial: 0x3D00_1234,
            node_name,
            node_label,
        });
    }

    #[test]
    fn test_heartbeat_roundtrip() {
        assert_roundtrip(HeartBeatPayload {
            sequence: 0,
            src_ip: Ipv4Addr::new(10, 37, 120, 230),
            mac: [0x00, 0x0a, 0xc5, 0x12, 0x34, 0x56],
            data16: 0x0001,
            serial: 0x3D00_4242,
            data32: 0x0003_0001,
        });
    }

    #[test]
    fn test_dmxout_roundtrip() {
        assert_roundtrip(DmxOutHeader {
            sequence: 420_420,
            ..Default::default()
        });
    }
}
