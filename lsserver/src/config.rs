use std::time::Duration;

#[derive(Clone, Copy)]
pub struct ServerConfig {
    pub num_arcs: usize,
    pub lights_per_arc: usize,
    pub kinet_port: u16,
    pub refresh_rate: Duration,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            num_arcs: 12,
            lights_per_arc: 14,
            kinet_port: 6038,
            refresh_rate: Duration::from_millis(1_000 / 30), // 40Hz jitters a bit, idk why
        }
    }
}
