use std::net::SocketAddr;

use kinetrs::KinetHeader;


pub struct KinetPowerSupply {
    remote_adr: SocketAddr,
    header: KinetHeader,
}

pub struct LightStage {
    rgb_supplies: Vec<KinetPowerSupply>,
    white_supplies: Vec<KinetPowerSupply>,
}
