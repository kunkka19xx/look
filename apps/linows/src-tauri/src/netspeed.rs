//! The `/speed` commands: the shared `look-netspeed` measurement macOS reaches
//! through the FFI bridge, plus the LAN address the panel shows beside it.

use look_netspeed::SpeedReading;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};

/// Where the route lookup pretends to be headed. Nothing is sent to it.
const ROUTE_PROBE: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)), 80);

/// Runs on Tauri's blocking pool: the measurement blocks for many seconds.
#[tauri::command(async)]
pub fn speed_test() -> Result<SpeedReading, String> {
    look_netspeed::run().map_err(|error| error.message().to_string())
}

/// This machine's address on the local network. `connect` on a UDP socket only
/// asks the routing table which interface would carry the traffic, so no packet
/// leaves and nothing blocks. `None` when only loopback is up.
#[tauri::command]
pub fn local_ipv4() -> Option<String> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).ok()?;
    socket.connect(ROUTE_PROBE).ok()?;
    match socket.local_addr().ok()?.ip() {
        IpAddr::V4(address) if !address.is_loopback() && !address.is_unspecified() => {
            Some(address.to_string())
        }
        _ => None,
    }
}
