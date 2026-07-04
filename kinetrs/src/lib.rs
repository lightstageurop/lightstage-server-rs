const KINET_MAGIC: u32 = 0x0401_DC4A;
const KINET_VERSION: u16 = 0x0001;

pub const KINET_UDP_PORT: u16 = 6038;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum KinetHeaderType {
    DiscoverSupplies = 0x0001,      // poll
    DiscoverSuppliesReply = 0x0002, // poll reply
    SetIp = 0x0003,
    SetUniverse = 0x0005,
    SetName = 0x0006,
    DmxOut = 0x0101,
    // PortOut = 0x0108,
    // PortOutSync = 0x0109,
    DiscoverFixturesSerialRequest = 0x0201,
    DiscoverFixturesChannelRequest = 0x0203, // get dmx address
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KinetHeader {
    pub kind: KinetHeaderType,
    pub sequence: u32,
    // DMX output port
    // Seemingly only used for v2 in broadcast environment
    // See https://colorkinetics.helpdocs.io/article/umxjxmoc7a-ki-net-ethernet-protocol-whitepaper#ki_net_universes
    pub port: u8,
    // No idea what this does, seems to always be zero
    pub flags: u16,
    // no idea what this does
    pub timer_val: u32,
    // Only used for broadcast
    pub universe: u8,
}

impl Default for KinetHeader {
    fn default() -> Self {
        Self {
            kind: KinetHeaderType::DmxOut,
            sequence: 0,
            port: 0, // always zero for v1?
            flags: 0,
            timer_val: u32::MAX, // TODO test this. kinet.py uses u32::MAX, OLA uses 0u32
            universe: 0,
        }
    }
}

impl KinetHeader {
    pub fn new(kind: KinetHeaderType) -> Self {
        Self {
            kind,
            ..Default::default()
        }
    }

    pub fn to_bytes(&self) -> [u8; 21] {
        let mut bytes = [0u8; 21];

        // Magic is left as BE,
        // the rest of the protocol appears to be LE
        bytes[0..4].copy_from_slice(&KINET_MAGIC.to_be_bytes());
        bytes[4..6].copy_from_slice(&KINET_VERSION.to_le_bytes());
        bytes[6..8].copy_from_slice(&(self.kind as u16).to_le_bytes());
        bytes[8..12].copy_from_slice(&self.sequence.to_le_bytes());
        bytes[12] = self.port;
        bytes[13] = 0; // padding byte
        bytes[14..16].copy_from_slice(&self.flags.to_le_bytes());
        bytes[16..20].copy_from_slice(&self.timer_val.to_le_bytes());
        bytes[20] = self.universe;

        bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_values() {
        let header = KinetHeader::default();
        assert_eq!(header.kind, KinetHeaderType::DmxOut);
        assert_eq!(header.timer_val, u32::MAX);
    }

    #[test]
    fn test_to_bytes_dmx_out() {
        let header = KinetHeader {
            kind: KinetHeaderType::DmxOut,
            sequence: 128,
            port: 0,
            flags: 0,
            timer_val: u32::MAX,
            universe: 0,
        };

        let bytes = header.to_bytes();

        // Expected byte array breakdown for DmxOut:
        // [0..4] Magic (BE): 0x04, 0x01, 0xDC, 0x4A
        // [4..6] Version (LE): 0x01, 0x00
        // [6..8] Type/Kind (LE): 0x01, 0x01 (DmxOut)
        // [8..12] Sequence (LE): 128 -> 0x80, 0x00, 0x00, 0x00
        // [12] Port: 0x00
        // [13] Padding: 0x00
        // [14..16] Flags (LE): 0x00, 0x00
        // [16..20] Timer (LE): MAX -> 0xFF, 0xFF, 0xFF, 0xFF
        // [20] Universe: 0x00
        let expected: [u8; 21] = [
            0x04, 0x01, 0xDC, 0x4A, 0x01, 0x00, 0x01,
            0x01, // Notice the difference here vs DiscoverSupplies
            0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0x00,
        ];

        assert_eq!(
            bytes, expected,
            "DmxOut serialized bytes do not match expected layout"
        );
    }
}
