//! UPnP IGD: find the gateway by SSDP (UPnP Device Architecture 1.0 §1.2),
//! read its description, call `WANIPConnection` (IGD 1.0 `WANIPConnection:1`
//! service template §2) by SOAP.

use std::net::{Ipv4Addr, SocketAddrV4, UdpSocket};
use std::time::{Duration, Instant};

use super::http::{self, Url};
use super::NatError;

pub const SSDP: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(239, 255, 255, 250), 1900);
const SEARCH_TARGET: &str = "urn:schemas-upnp-org:device:InternetGatewayDevice:1";
/// IGD 1.0 §2.4.16: the services that carry `AddPortMapping`, preferred first.
const SERVICES: [&str; 3] = [
    "urn:schemas-upnp-org:service:WANIPConnection:2",
    "urn:schemas-upnp-org:service:WANIPConnection:1",
    "urn:schemas-upnp-org:service:WANPPPConnection:1",
];
pub const DESCRIPTION: &str = "open-lords2";

/// A gateway's port-mapping service, ready to be called.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Igd {
    pub control: Url,
    pub service: &'static str,
    /// The address this machine has on the gateway's side of us.
    pub local: Ipv4Addr,
}

/// Device Architecture 1.0 §1.2.2: M-SEARCH, answers carry `LOCATION`.
pub fn discover(unicast: &[Ipv4Addr], wait: Duration) -> Result<Vec<Url>, NatError> {
    let search = format!(
        "M-SEARCH * HTTP/1.1\r\nHOST: 239.255.255.250:1900\r\nMAN: \"ssdp:discover\"\r\nMX: {}\r\nST: {SEARCH_TARGET}\r\n\r\n",
        wait.as_secs().clamp(1, 5)
    );
    let sock = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).map_err(|e| NatError::Io(e.to_string()))?;
    let _ = sock.send_to(search.as_bytes(), SSDP);
    for ip in unicast {
        let _ = sock.send_to(search.as_bytes(), SocketAddrV4::new(*ip, 1900));
    }
    let mut found = Vec::new();
    let mut buf = [0u8; 4096];
    let until = Instant::now() + wait;
    while let Some(left) = until.checked_duration_since(Instant::now()) {
        sock.set_read_timeout(Some(left.max(Duration::from_millis(10))))
            .map_err(|e| NatError::Io(e.to_string()))?;
        let Ok((n, _)) = sock.recv_from(&mut buf) else { break };
        if let Some(url) = location(&String::from_utf8_lossy(&buf[..n])) {
            if !found.contains(&url) {
                found.push(url);
            }
        }
    }
    Ok(found)
}

pub fn location(reply: &str) -> Option<Url> {
    reply.lines().find_map(|l| {
        let (k, v) = l.split_once(':')?;
        k.trim().eq_ignore_ascii_case("location").then(|| Url::parse(v.trim())).flatten()
    })
}

fn element<'a>(xml: &'a str, name: &str) -> Option<&'a str> {
    let open = format!("<{name}>");
    let close = format!("</{name}>");
    let start = xml.find(&open)? + open.len();
    let end = start + xml[start..].find(&close)?;
    Some(xml[start..end].trim())
}

/// The `<controlURL>` of the first `<service>` whose type is in `SERVICES`.
pub fn control_url(description: &str, base: &Url) -> Option<(Url, &'static str)> {
    for service in SERVICES {
        let mut rest = description;
        while let Some(i) = rest.find("<service>") {
            let block = &rest[i..];
            let end = block.find("</service>").map_or(block.len(), |e| e + "</service>".len());
            let block = &block[..end];
            if element(block, "serviceType") == Some(service) {
                return element(block, "controlURL").map(|u| (base.join(u), service));
            }
            rest = &rest[i + end..];
        }
    }
    None
}

impl Igd {
    pub fn from_location(location: &Url) -> Result<Igd, NatError> {
        let page = http::get(location)?;
        if page.status != 200 {
            return Err(NatError::Io(format!("the gateway's description answered {}", page.status)));
        }
        let base = element(&page.body, "URLBase").and_then(Url::parse).unwrap_or_else(|| location.clone());
        let (control, service) = control_url(&page.body, &base)
            .ok_or_else(|| NatError::Refused("the gateway has no WANIPConnection service".into()))?;
        let local = match page.local.ip() {
            std::net::IpAddr::V4(v4) => v4,
            std::net::IpAddr::V6(_) => return Err(NatError::Io("IPv6 route to the gateway".into())),
        };
        Ok(Igd { control, service, local })
    }

    fn call(&self, action: &str, args: &str) -> Result<http::Response, NatError> {
        let body = format!(
            "<?xml version=\"1.0\"?><s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\" s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\"><s:Body><u:{action} xmlns:u=\"{}\">{args}</u:{action}></s:Body></s:Envelope>",
            self.service
        );
        http::post_soap(&self.control, &format!("{}#{action}", self.service), &body)
    }

    /// `AddPortMapping`; IGD 1.0 §2.4.16 errors 718 (conflict) and 725
    /// (permanent leases only), the second retried with lease 0.
    pub fn add_tcp(&self, port: u16, lease: u32) -> Result<u32, NatError> {
        let args = |lease: u32| {
            format!(
                "<NewRemoteHost></NewRemoteHost><NewExternalPort>{port}</NewExternalPort><NewProtocol>TCP</NewProtocol><NewInternalPort>{port}</NewInternalPort><NewInternalClient>{}</NewInternalClient><NewEnabled>1</NewEnabled><NewPortMappingDescription>{DESCRIPTION}</NewPortMappingDescription><NewLeaseDuration>{lease}</NewLeaseDuration>",
                self.local
            )
        };
        let reply = self.call("AddPortMapping", &args(lease))?;
        if reply.status == 200 {
            return Ok(lease);
        }
        if lease != 0 && element(&reply.body, "errorCode") == Some("725") {
            let reply = self.call("AddPortMapping", &args(0))?;
            if reply.status == 200 {
                return Ok(0);
            }
            return Err(refusal(&reply));
        }
        Err(refusal(&reply))
    }

    pub fn delete_tcp(&self, port: u16) -> Result<(), NatError> {
        let args = format!(
            "<NewRemoteHost></NewRemoteHost><NewExternalPort>{port}</NewExternalPort><NewProtocol>TCP</NewProtocol>"
        );
        let reply = self.call("DeletePortMapping", &args)?;
        if reply.status == 200 {
            Ok(())
        } else {
            Err(refusal(&reply))
        }
    }

    pub fn external_address(&self) -> Result<Ipv4Addr, NatError> {
        let reply = self.call("GetExternalIPAddress", "")?;
        if reply.status != 200 {
            return Err(refusal(&reply));
        }
        element(&reply.body, "NewExternalIPAddress")
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| NatError::Refused("the gateway gave no external address".into()))
    }
}

fn refusal(reply: &http::Response) -> NatError {
    let code = element(&reply.body, "errorCode").unwrap_or("?");
    let text = element(&reply.body, "errorDescription").unwrap_or("");
    NatError::Refused(format!("UPnP {code} {text}").trim().to_string())
}
