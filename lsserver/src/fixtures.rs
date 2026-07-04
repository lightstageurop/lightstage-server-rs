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

pub trait DmxValue: Copy + Default {
    const CHANNELS: usize;

    fn write_to(self, dst: &mut [u8]);
}

impl DmxValue for u8 {
    const CHANNELS: usize = 1;

    fn write_to(self, dst: &mut [u8]) {
        dst[0] = self;
    }
}

impl DmxValue for u16 {
    const CHANNELS: usize = 2;

    fn write_to(self, dst: &mut [u8]) {
        dst[0..2].copy_from_slice(&self.to_be_bytes());
    }
}

pub trait Fixture {
    fn channels(&self) -> usize;

    fn address(&self) -> DmxAddress;
    fn write_to_universe(&self, buf: &mut [u8; 512]);
}

pub struct RgbFixture<T: DmxValue> {
    address: DmxAddress,
    pub r: T,
    pub g: T,
    pub b: T,
}

impl<T: DmxValue> RgbFixture<T> {
    const CHANNELS: usize = 3 * T::CHANNELS;

    pub fn new(address: DmxAddress) -> Option<Self> {
        if address.index() + Self::CHANNELS > 512 {
            return None;
        }

        Some(Self {
            address,
            r: T::default(),
            g: T::default(),
            b: T::default(),
        })
    }

    pub fn set_color(&mut self, r: T, g: T, b: T) {
        (self.r, self.g, self.b) = (r, g, b);
    }
}

impl<T: DmxValue> Fixture for RgbFixture<T> {
    fn channels(&self) -> usize {
        Self::CHANNELS
    }

    fn address(&self) -> DmxAddress {
        self.address
    }

    fn write_to_universe(&self, buf: &mut [u8; 512]) {
        let mut i = self.address.index();
        debug_assert!(i + Self::CHANNELS <= 512);
        self.r.write_to(&mut buf[i..i + T::CHANNELS]);
        i += T::CHANNELS;
        self.g.write_to(&mut buf[i..i + T::CHANNELS]);
        i += T::CHANNELS;
        self.b.write_to(&mut buf[i..i + T::CHANNELS]);
    }
}

pub struct WhiteFixture<T: DmxValue> {
    address: DmxAddress,
    pub warm: T,
    pub neutral: T,
    pub cool: T,
}

impl<T: DmxValue> WhiteFixture<T> {
    const CHANNELS: usize = 3 * T::CHANNELS;

    pub fn new(address: DmxAddress) -> Option<Self> {
        if address.index() + Self::CHANNELS > 512 {
            return None;
        }

        Some(Self {
            address,
            warm: T::default(),
            neutral: T::default(),
            cool: T::default(),
        })
    }

    pub fn set_white(&mut self, warm: T, neutral: T, cool: T) {
        (self.warm, self.neutral, self.cool) = (warm, neutral, cool);
    }
}

impl<T: DmxValue> Fixture for WhiteFixture<T> {
    fn channels(&self) -> usize {
        Self::CHANNELS
    }

    fn address(&self) -> DmxAddress {
        self.address
    }

    fn write_to_universe(&self, buf: &mut [u8; 512]) {
        let mut i = self.address.index();
        debug_assert!(i + Self::CHANNELS <= 512);
        self.warm.write_to(&mut buf[i..i + T::CHANNELS]);
        i += T::CHANNELS;
        self.neutral.write_to(&mut buf[i..i + T::CHANNELS]);
        i += T::CHANNELS;
        self.cool.write_to(&mut buf[i..i + T::CHANNELS]);
    }
}
