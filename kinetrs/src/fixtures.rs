/// Configuration items for a fixture
///
/// This was reverse engineered from iColor MR gen3 fixtures.
/// Other fixtures may be different.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FixtureConfiguration {
    DmxAddress = 0x41,   // zero indexed
    DimmingCurve = 0x6d, // 0x00=linear, 0x01=normal, 0x02=tungsten
    Resolution = 0x6e,   // 0x01=8b, 0x00=16b
    StartupChannel1 = 0x43,
    StartupChannel2 = 0x44,
    StartupChannel3 = 0x45,
}
