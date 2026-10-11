//! The HTTP/1.1 an Internet Gateway Device speaks (UPnP Device Architecture
//! 1.0 §2.1): one request, `Connection: close`, read to the end.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::Duration;

use super::NatError;

const CONNECT: Duration = Duration::from_secs(2);
const READ: Duration = Duration::from_secs(3);
const MAX_BODY: usize = 256 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Url {
    pub host: String,
    pub path: String,
}

impl Url {
    pub fn parse(url: &str) -> Option<Url> {
        let rest = url.strip_prefix("http://")?;
        let (host, path) = match rest.find('/') {
            Some(i) => (&rest[..i], &rest[i..]),
            None => (rest, "/"),
        };
        if host.is_empty() {
            return None;
        }
        Some(Url { host: host.to_string(), path: path.to_string() })
    }

    /// Device Architecture 1.0 §2.1: a control URL may be absolute, rooted or relative.
    pub fn join(&self, link: &str) -> Url {
        if let Some(u) = Url::parse(link) {
            return u;
        }
        let path = if link.starts_with('/') {
            link.to_string()
        } else {
            let dir = &self.path[..self.path.rfind('/').map_or(0, |i| i + 1)];
            format!("{dir}{link}")
        };
        Url { host: self.host.clone(), path }
    }

    pub fn host_port(&self) -> Option<SocketAddr> {
        let host = if self.host.contains(':') { self.host.clone() } else { format!("{}:80", self.host) };
        host.to_socket_addrs().ok()?.next()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    pub status: u16,
    pub body: String,
    /// Our address on the way to the device: `NewInternalClient`.
    pub local: SocketAddr,
}

pub fn get(url: &Url) -> Result<Response, NatError> {
    let req = format!("GET {} HTTP/1.1\r\nHOST: {}\r\nConnection: close\r\n\r\n", url.path, url.host);
    send(url, req.as_bytes())
}

pub fn post_soap(url: &Url, action: &str, body: &str) -> Result<Response, NatError> {
    let req = format!(
        "POST {} HTTP/1.1\r\nHOST: {}\r\nCONTENT-TYPE: text/xml; charset=\"utf-8\"\r\nCONTENT-LENGTH: {}\r\nSOAPACTION: \"{}\"\r\nConnection: close\r\n\r\n{}",
        url.path,
        url.host,
        body.len(),
        action,
        body
    );
    send(url, req.as_bytes())
}

fn send(url: &Url, request: &[u8]) -> Result<Response, NatError> {
    let addr = url.host_port().ok_or_else(|| NatError::Io(format!("{} does not resolve", url.host)))?;
    let mut s = TcpStream::connect_timeout(&addr, CONNECT).map_err(|e| NatError::Io(e.to_string()))?;
    s.set_read_timeout(Some(READ)).map_err(|e| NatError::Io(e.to_string()))?;
    s.set_write_timeout(Some(READ)).map_err(|e| NatError::Io(e.to_string()))?;
    let local = s.local_addr().map_err(|e| NatError::Io(e.to_string()))?;
    s.write_all(request).map_err(|e| NatError::Io(e.to_string()))?;
    let mut raw = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        match s.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                raw.extend_from_slice(&chunk[..n]);
                if raw.len() > MAX_BODY {
                    return Err(NatError::Io("the device's reply is too long".into()));
                }
                if complete(&raw) {
                    break;
                }
            }
            Err(e) if raw.is_empty() => return Err(NatError::Io(e.to_string())),
            Err(_) => break,
        }
    }
    let (status, body) = split(&raw).ok_or_else(|| NatError::Io("not an HTTP reply".into()))?;
    Ok(Response { status, body, local })
}

fn header_end(raw: &[u8]) -> Option<usize> {
    raw.windows(4).position(|w| w == b"\r\n\r\n").map(|i| i + 4)
}

fn header_value<'a>(head: &'a str, name: &str) -> Option<&'a str> {
    head.lines().find_map(|l| {
        let (k, v) = l.split_once(':')?;
        k.trim().eq_ignore_ascii_case(name).then(|| v.trim())
    })
}

/// `Content-Length` lets us stop before the device closes its side.
fn complete(raw: &[u8]) -> bool {
    let Some(end) = header_end(raw) else { return false };
    let head = String::from_utf8_lossy(&raw[..end]);
    match header_value(&head, "content-length").and_then(|v| v.parse::<usize>().ok()) {
        Some(len) => raw.len() >= end + len,
        None => header_value(&head, "transfer-encoding").is_some_and(|v| v.contains("chunked"))
            && raw.ends_with(b"0\r\n\r\n"),
    }
}

fn split(raw: &[u8]) -> Option<(u16, String)> {
    let end = header_end(raw)?;
    let head = String::from_utf8_lossy(&raw[..end]).into_owned();
    let status = head.lines().next()?.split_whitespace().nth(1)?.parse().ok()?;
    let body = &raw[end..];
    let chunked = header_value(&head, "transfer-encoding").is_some_and(|v| v.contains("chunked"));
    let body = if chunked { dechunk(body) } else { body.to_vec() };
    Some((status, String::from_utf8_lossy(&body).into_owned()))
}

/// RFC 9112 §7.1.
fn dechunk(mut body: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let Some(nl) = body.windows(2).position(|w| w == b"\r\n") else { break };
        let size = String::from_utf8_lossy(&body[..nl]);
        let size = size.split(';').next().unwrap_or("").trim();
        let Ok(n) = usize::from_str_radix(size, 16) else { break };
        if n == 0 || body.len() < nl + 2 + n {
            break;
        }
        out.extend_from_slice(&body[nl + 2..nl + 2 + n]);
        body = &body[(nl + 2 + n + 2).min(body.len())..];
    }
    out
}
