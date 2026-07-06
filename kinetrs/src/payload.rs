use std::{
    io::{self, Write},
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

#[cfg(test)]
mod tests {
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
    fn test_payload_sizes_match_constants() {
        assert_payload_size::<PollPayload>("PollPayload");
        assert_payload_size::<PollReplyPayload>("PollReplyPayload");
        assert_payload_size::<HeartBeatPayload>("HeartBeatPayload");
        assert_payload_size::<DmxOutHeader>("DmxOutHeader");
    }
}
