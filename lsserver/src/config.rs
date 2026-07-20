//! Light stage server configuration

use std::net::{IpAddr, Ipv4Addr};

use clap::Parser;
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, Parser)]
#[command(author, version, about, long_about = None)]
pub struct CliConfig {
    /// Maximum DMX refresh rate (in Hz) [default: 30]
    #[arg(long, short = 'r')]
    pub max_refresh_rate: Option<u64>,
    /// REST API bind address [default: 0.0.0.0]
    #[arg(long, short, env = "LSAPI_IP")]
    pub ip: Option<IpAddr>,
    /// REST API port [default: 8080]
    #[arg(long, short, env = "LSAPI_PORT")]
    pub port: Option<u16>,
}

/// Configuration parameters for the light stage server.
///
/// Defines physical layout of the arcs/fixtures, network configuration of REST API, `KiNET`, etc.
#[derive(Debug, Clone, Copy, ToSchema, Serialize)]
pub struct ServerConfig {
    pub num_arcs: usize,
    pub lights_per_arc: usize,
    pub kinet_port: u16,
    pub heartbeat_port: u16,
    /// `KiNET` / `DmxOut` refresh rate
    pub refresh_rate_ms: u64,
    /// Axum REST API bind address
    #[schema(value_type = String, example = "127.0.0.1")]
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
            heartbeat_port: 6045,
            refresh_rate_ms: 1_000 / 30, // 40Hz jitters a bit, idk why
            api_ip: Ipv4Addr::UNSPECIFIED.into(),
            api_port: 8080,
        }
    }
}

impl From<CliConfig> for ServerConfig {
    fn from(cli: CliConfig) -> Self {
        let def = Self::default();

        Self {
            refresh_rate_ms: cli
                .max_refresh_rate
                .map_or(def.refresh_rate_ms, |r| 1_000 / r),
            api_ip: cli.ip.unwrap_or(def.api_ip),
            api_port: cli.port.unwrap_or(def.api_port),
            ..def
        }
    }
}
