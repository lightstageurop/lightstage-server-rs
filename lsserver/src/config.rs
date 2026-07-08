use std::net::{IpAddr, Ipv4Addr};

/// Configuration parameters for the light stage server.
///
/// Defines physical layout of the arcs/fixtures, network configuration of REST API, `KiNET`, etc.
#[derive(Clone, Copy)]
pub struct ServerConfig {
    pub num_arcs: usize,
    pub lights_per_arc: usize,
    pub kinet_port: u16,
    /// `KiNET` / `DmxOut` refresh rate
    pub refresh_rate_ms: u64,
    /// Axum REST API bind address
    pub api_ip: IpAddr,
    /// Axum REST API port
    pub api_port: u16,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            num_arcs: 12,
            lights_per_arc: 14,
            kinet_port: 6038,
            refresh_rate_ms: 1_000 / 30, // 40Hz jitters a bit, idk why
            api_ip: Ipv4Addr::UNSPECIFIED.into(),
            api_port: 8080,
        }
    }
}
