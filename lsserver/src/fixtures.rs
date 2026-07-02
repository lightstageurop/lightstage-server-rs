#[derive(Clone, Copy, PartialEq, Eq)]
pub struct DmxAddress(u16);

impl DmxAddress {
    pub fn new(channel: u16) -> Option<Self> {
        if (1..=512).contains(&channel) {
            Some(Self(channel))
        } else {
            None
        }
    }

    pub fn index(self) -> usize {
        self.0 as usize - 1
    }
}

pub trait Fixture {
    fn channels(&self) -> usize;

    fn address(&self) -> DmxAddress;
    fn write_to_universe(&self, buf: &mut [u8; 512]);
}

pub struct RgbFixture {
    address: DmxAddress,
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl RgbFixture {
    const CHANNELS: usize = 3;

    pub fn new(address: DmxAddress) -> Option<Self> {
        if address.index() + Self::CHANNELS > 512 {
            return None;
        }

        Some(Self {
            address,
            r: 0,
            g: 0,
            b: 0,
        })
    }

    pub fn set_color(&mut self, r: u8, g: u8, b: u8) {
        (self.r, self.g, self.b) = (r, g, b);
    }
}

impl Fixture for RgbFixture {
    fn channels(&self) -> usize {
        Self::CHANNELS
    }

    fn address(&self) -> DmxAddress {
        self.address
    }

    fn write_to_universe(&self, buf: &mut [u8; 512]) {
        let i = self.address.index();
        debug_assert!(i + Self::CHANNELS <= 512);
        buf[i..i + 3].copy_from_slice(&[self.r, self.b, self.g]);
    }
}

pub struct WhiteFixture {
    address: DmxAddress,
    pub warm: u8,
    pub neutral: u8,
    pub cool: u8,
}

impl WhiteFixture {
    const CHANNELS: usize = 3;

    pub fn new(address: DmxAddress) -> Option<Self> {
        if address.index() + Self::CHANNELS > 512 {
            return None;
        }

        Some(Self {
            address,
            warm: 0,
            neutral: 0,
            cool: 0,
        })
    }

    pub fn set_white(&mut self, warm: u8, neutral: u8, cool: u8) {
        (self.warm, self.neutral, self.cool) = (warm, neutral, cool);
    }
}

impl Fixture for WhiteFixture {
    fn channels(&self) -> usize {
        Self::CHANNELS
    }

    fn address(&self) -> DmxAddress {
        self.address
    }

    fn write_to_universe(&self, buf: &mut [u8; 512]) {
        let i = self.address.index();
        debug_assert!(i + Self::CHANNELS <= 512);
        buf[i..i + 3].copy_from_slice(&[self.warm, self.cool, self.neutral]);
    }
}
