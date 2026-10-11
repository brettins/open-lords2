//! Opening the host's port on a home router (`docs/netcode.md` §7 requirement
//! 5, C252): NAT-PMP first, UPnP IGD second, each best effort. Blocking, so
//! the app calls it off its loop. Ours: the original had DirectPlay's lobby.

pub mod http;
pub mod pmp;
pub mod upnp;

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};
use std::time::Duration;

/// RFC 6886 §3.3 recommends two hours; the lobby takes no one after `Start`.
pub const LEASE: u32 = 7200;
const SSDP_WAIT: Duration = Duration::from_millis(1500);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NatError {
    /// No gateway spoke either protocol.
    NoAnswer,
    /// The gateway answered and said no; its words.
    Refused(String),
    Io(String),
}

impl std::fmt::Display for NatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NatError::NoAnswer => write!(f, "no router answered NAT-PMP or UPnP"),
            NatError::Refused(why) => write!(f, "the router refused: {why}"),
            NatError::Io(why) => write!(f, "{why}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Via {
    NatPmp { gateway: SocketAddr },
    Upnp(upnp::Igd),
}

impl Via {
    pub fn name(&self) -> &'static str {
        match self {
            Via::NatPmp { .. } => "NAT-PMP",
            Via::Upnp(_) => "UPnP",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mapping {
    pub internal_port: u16,
    pub external: SocketAddrV4,
    /// Seconds; 0 is a permanent lease.
    pub lifetime: u32,
    pub via: Via,
}

/// This machine's address on its LAN: the route to TEST-NET-1 (RFC 5737)
/// names the interface, and a UDP connect sends nothing.
pub fn lan_ip() -> Option<Ipv4Addr> {
    let s = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).ok()?;
    s.connect((Ipv4Addr::new(192, 0, 2, 1), 9)).ok()?;
    match s.local_addr().ok()?.ip() {
        std::net::IpAddr::V4(v4) if !v4.is_unspecified() => Some(v4),
        _ => None,
    }
}

#[cfg(target_os = "linux")]
fn routed_gateway() -> Option<Ipv4Addr> {
    // `/proc/net/route`: destination 0 is the default route, gateway little-endian hex.
    let table = std::fs::read_to_string("/proc/net/route").ok()?;
    table.lines().skip(1).find_map(|l| {
        let mut f = l.split_whitespace();
        let (_iface, dest, gw) = (f.next()?, f.next()?, f.next()?);
        (dest == "00000000").then(|| u32::from_str_radix(gw, 16).ok().map(|g| Ipv4Addr::from(g.swap_bytes())))?
    })
}

#[cfg(not(target_os = "linux"))]
fn routed_gateway() -> Option<Ipv4Addr> {
    None
}

/// Where a router may be: the routing table's default, then the LAN's .1 and .254.
pub fn gateway_candidates() -> Vec<Ipv4Addr> {
    let mut out = Vec::new();
    if let Some(g) = routed_gateway() {
        out.push(g);
    }
    if let Some(lan) = lan_ip() {
        let o = lan.octets();
        for last in [1, 254] {
            let g = Ipv4Addr::new(o[0], o[1], o[2], last);
            if !out.contains(&g) {
                out.push(g);
            }
        }
    }
    out
}

/// Maps TCP `port` to this machine on whichever router answers.
pub fn open_port(port: u16) -> Result<Mapping, NatError> {
    let gateways = gateway_candidates();
    let mut last = NatError::NoAnswer;
    for gw in gateways.iter().take(2) {
        let gateway = pmp::gateway_addr(*gw);
        match pmp::map_tcp(gateway, port, port, LEASE) {
            Ok(mapped) => {
                let ip = pmp::external_address(gateway)?;
                return Ok(Mapping {
                    internal_port: port,
                    external: SocketAddrV4::new(ip, mapped.external_port),
                    lifetime: mapped.lifetime,
                    via: Via::NatPmp { gateway },
                });
            }
            Err(NatError::NoAnswer) => {}
            Err(e) => last = e,
        }
    }
    for location in upnp::discover(&gateways, SSDP_WAIT)? {
        let igd = match upnp::Igd::from_location(&location) {
            Ok(igd) => igd,
            Err(e) => {
                last = e;
                continue;
            }
        };
        match igd.add_tcp(port, LEASE) {
            Ok(lifetime) => {
                let ip = igd.external_address()?;
                return Ok(Mapping {
                    internal_port: port,
                    external: SocketAddrV4::new(ip, port),
                    lifetime,
                    via: Via::Upnp(igd),
                });
            }
            Err(e) => last = e,
        }
    }
    Err(last)
}

pub fn close_port(mapping: &Mapping) -> Result<(), NatError> {
    match &mapping.via {
        Via::NatPmp { gateway } => pmp::map_tcp(*gateway, mapping.internal_port, 0, 0).map(|_| ()),
        Via::Upnp(igd) => igd.delete_tcp(mapping.external.port()),
    }
}
