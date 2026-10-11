//! NAT-PMP, RFC 6886: twelve-byte requests to the gateway's UDP 5351.

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};
use std::time::Duration;

use super::NatError;

pub const PORT: u16 = 5351;
/// RFC 6886 §3.1: first wait 250 ms, doubling; we stop after the third.
const WAITS: [Duration; 3] =
    [Duration::from_millis(250), Duration::from_millis(500), Duration::from_millis(1000)];
const OP_EXTERNAL: u8 = 0;
const OP_MAP_TCP: u8 = 2;
const RESPONSE: u8 = 128;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mapped {
    pub external_port: u16,
    pub lifetime: u32,
}

/// RFC 6886 §3.5 result codes.
pub fn result_name(code: u16) -> &'static str {
    match code {
        0 => "success",
        1 => "unsupported version",
        2 => "not authorized",
        3 => "network failure",
        4 => "out of resources",
        5 => "unsupported opcode",
        _ => "unknown result",
    }
}

fn exchange(gateway: SocketAddr, request: &[u8], want: usize) -> Result<Vec<u8>, NatError> {
    let sock = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).map_err(|e| NatError::Io(e.to_string()))?;
    sock.connect(gateway).map_err(|e| NatError::Io(e.to_string()))?;
    let mut buf = [0u8; 64];
    for wait in WAITS {
        sock.send(request).map_err(|e| NatError::Io(e.to_string()))?;
        sock.set_read_timeout(Some(wait)).map_err(|e| NatError::Io(e.to_string()))?;
        match sock.recv(&mut buf) {
            Ok(n) if n >= want && buf[0] == 0 && buf[1] == request[1] | RESPONSE => {
                let code = u16::from_be_bytes([buf[2], buf[3]]);
                if code != 0 {
                    return Err(NatError::Refused(format!("NAT-PMP {code}: {}", result_name(code))));
                }
                return Ok(buf[..n].to_vec());
            }
            Ok(_) => continue,
            Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {
                continue
            }
            Err(e) => return Err(NatError::Io(e.to_string())),
        }
    }
    Err(NatError::NoAnswer)
}

/// RFC 6886 §3.2: opcode 0 answers with the external IPv4 at bytes 8..12.
pub fn external_address(gateway: SocketAddr) -> Result<Ipv4Addr, NatError> {
    let reply = exchange(gateway, &[0, OP_EXTERNAL], 12)?;
    Ok(Ipv4Addr::new(reply[8], reply[9], reply[10], reply[11]))
}

/// RFC 6886 §3.3: opcode 2 maps TCP; `lifetime` 0 with `external` 0 deletes.
pub fn map_tcp(
    gateway: SocketAddr,
    internal: u16,
    external: u16,
    lifetime: u32,
) -> Result<Mapped, NatError> {
    let mut req = [0u8; 12];
    req[1] = OP_MAP_TCP;
    req[4..6].copy_from_slice(&internal.to_be_bytes());
    req[6..8].copy_from_slice(&external.to_be_bytes());
    req[8..12].copy_from_slice(&lifetime.to_be_bytes());
    let reply = exchange(gateway, &req, 16)?;
    Ok(Mapped {
        external_port: u16::from_be_bytes([reply[10], reply[11]]),
        lifetime: u32::from_be_bytes([reply[12], reply[13], reply[14], reply[15]]),
    })
}

pub fn gateway_addr(ip: Ipv4Addr) -> SocketAddr {
    SocketAddr::V4(SocketAddrV4::new(ip, PORT))
}
