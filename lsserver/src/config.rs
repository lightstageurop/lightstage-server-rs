use std::time::Duration;

pub const NUM_ARCS: usize = 12;
pub const LIGHTS_PER_ARC: usize = 14;
pub const PDS_SUBNET_BASE: &str = "10.37.211.";
pub const KINET_REFRESH_RATE_MS: Duration = Duration::from_millis(1_000 / 30); // 40Hz jitters a bit, idk why
