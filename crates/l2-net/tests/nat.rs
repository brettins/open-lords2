//! The host's port mapping (C252) against gateways faked on loopback, and
//! the home router when `LORDS2_NAT_LIVE` is set.

use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, UdpSocket};
use std::sync::mpsc;
use std::time::Duration;

use l2_net::nat::http::Url;
use l2_net::nat::{pmp, upnp, NatError};

/// RFC 6886 §3.3: a gateway that maps whatever it is asked and remembers it.
fn fake_pmp(reply_code: u16) -> (SocketAddr, mpsc::Receiver<Vec<u8>>) {
    let sock = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let addr = sock.local_addr().unwrap();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = [0u8; 64];
        while let Ok((n, from)) = sock.recv_from(&mut buf) {
            let req = buf[..n].to_vec();
            let mut out = vec![0, req[1] | 128];
            out.extend_from_slice(&reply_code.to_be_bytes());
            out.extend_from_slice(&1234u32.to_be_bytes());
            match req[1] {
                0 => out.extend_from_slice(&[203, 0, 113, 7]),
                _ => {
                    out.extend_from_slice(&req[4..6]);
                    out.extend_from_slice(&req[6..8]);
                    out.extend_from_slice(&req[8..12]);
                }
            }
            sock.send_to(&out, from).unwrap();
            let _ = tx.send(req);
        }
    });
    (addr, rx)
}

#[test]
fn pmp_maps_tcp_and_reports_the_external_address() {
    let (gw, seen) = fake_pmp(0);
    let mapped = pmp::map_tcp(gw, 4712, 4712, 7200).unwrap();
    assert_eq!(mapped.external_port, 4712);
    assert_eq!(mapped.lifetime, 7200);
    let req = seen.recv().unwrap();
    assert_eq!(req, vec![0, 2, 0, 0, 0x12, 0x68, 0x12, 0x68, 0, 0, 0x1C, 0x20]);
    assert_eq!(pmp::external_address(gw).unwrap(), Ipv4Addr::new(203, 0, 113, 7));
    assert_eq!(seen.recv().unwrap(), vec![0, 0]);
}

#[test]
fn pmp_names_a_refusal_and_gives_up_on_silence() {
    let (gw, _seen) = fake_pmp(2);
    assert_eq!(
        pmp::map_tcp(gw, 4712, 4712, 7200),
        Err(NatError::Refused("NAT-PMP 2: not authorized".into()))
    );
    let silent = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let addr = silent.local_addr().unwrap();
    assert_eq!(pmp::external_address(addr), Err(NatError::NoAnswer));
}

const DESCRIPTION: &str = r#"<?xml version="1.0"?><root xmlns="urn:schemas-upnp-org:device-1-0">
<device><deviceType>urn:schemas-upnp-org:device:InternetGatewayDevice:1</deviceType>
<serviceList><service><serviceType>urn:schemas-upnp-org:service:Layer3Forwarding:1</serviceType>
<controlURL>/ctl/L3F</controlURL></service></serviceList>
<deviceList><device><deviceType>urn:schemas-upnp-org:device:WANDevice:1</deviceType>
<deviceList><device><deviceType>urn:schemas-upnp-org:device:WANConnectionDevice:1</deviceType>
<serviceList><service><serviceType>urn:schemas-upnp-org:service:WANIPConnection:1</serviceType>
<controlURL>/ctl/IPConn</controlURL></service></serviceList></device></deviceList></device></deviceList>
</device></root>"#;

#[test]
fn the_wan_ip_connection_control_url_is_found_in_a_description() {
    let base = Url::parse("http://192.168.1.1:5000/rootDesc.xml").unwrap();
    let (url, service) = upnp::control_url(DESCRIPTION, &base).unwrap();
    assert_eq!(url, Url::parse("http://192.168.1.1:5000/ctl/IPConn").unwrap());
    assert_eq!(service, "urn:schemas-upnp-org:service:WANIPConnection:1");
    assert_eq!(upnp::control_url("<root></root>", &base), None);
    assert_eq!(base.join("ctl/x"), Url::parse("http://192.168.1.1:5000/ctl/x").unwrap());
    assert_eq!(base.join("http://10.0.0.1/y"), Url::parse("http://10.0.0.1/y").unwrap());
}

#[test]
fn an_ssdp_reply_names_its_location_whatever_the_case() {
    let reply = "HTTP/1.1 200 OK\r\nCACHE-CONTROL: max-age=120\r\nlocation: http://192.168.1.1:1900/igd.xml\r\nST: urn:schemas-upnp-org:device:InternetGatewayDevice:1\r\n\r\n";
    assert_eq!(upnp::location(reply), Url::parse("http://192.168.1.1:1900/igd.xml"));
    assert_eq!(upnp::location("HTTP/1.1 200 OK\r\n\r\n"), None);
}

fn soap_ok(action: &str, inner: &str) -> String {
    let body = format!(
        "<s:Envelope><s:Body><u:{action}Response>{inner}</u:{action}Response></s:Body></s:Envelope>"
    );
    format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len())
}

fn soap_error(code: u16, text: &str) -> String {
    let body = format!("<s:Envelope><s:Body><s:Fault><detail><UPnPError><errorCode>{code}</errorCode><errorDescription>{text}</errorDescription></UPnPError></detail></s:Fault></s:Body></s:Envelope>");
    format!("HTTP/1.1 500 Internal Server Error\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len())
}

/// An IGD on loopback: the description, then SOAP answers in the given
/// order; every request's head and body go back on the channel.
fn fake_igd(answers: Vec<String>, chunked: bool) -> (Url, mpsc::Receiver<String>) {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut answers = answers.into_iter();
        for stream in listener.incoming() {
            let mut s = stream.unwrap();
            s.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
            let mut raw = Vec::new();
            let mut chunk = [0u8; 4096];
            while let Ok(n) = s.read(&mut chunk) {
                raw.extend_from_slice(&chunk[..n]);
                let text = String::from_utf8_lossy(&raw);
                if let Some(h) = text.find("\r\n\r\n") {
                    let len = text[..h]
                        .lines()
                        .find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap()))
                        .unwrap_or(0);
                    if raw.len() >= h + 4 + len {
                        break;
                    }
                }
                if n == 0 {
                    break;
                }
            }
            let text = String::from_utf8_lossy(&raw).into_owned();
            let reply = if text.starts_with("GET ") {
                if chunked {
                    let half = DESCRIPTION.len() / 2;
                    format!(
                        "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n{:x}\r\n{}\r\n{:x}\r\n{}\r\n0\r\n\r\n",
                        half, &DESCRIPTION[..half], DESCRIPTION.len() - half, &DESCRIPTION[half..]
                    )
                } else {
                    format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{DESCRIPTION}", DESCRIPTION.len())
                }
            } else {
                answers.next().expect("an answer for every SOAP call")
            };
            s.write_all(reply.as_bytes()).unwrap();
            let _ = tx.send(text);
        }
    });
    (Url::parse(&format!("http://127.0.0.1:{port}/rootDesc.xml")).unwrap(), rx)
}

#[test]
fn upnp_adds_a_mapping_asks_the_external_address_and_deletes_it() {
    let (location, seen) = fake_igd(
        vec![
            soap_ok("AddPortMapping", ""),
            soap_ok("GetExternalIPAddress", "<NewExternalIPAddress>198.51.100.4</NewExternalIPAddress>"),
            soap_ok("DeletePortMapping", ""),
        ],
        true,
    );
    let igd = upnp::Igd::from_location(&location).unwrap();
    assert_eq!(igd.local, Ipv4Addr::LOCALHOST);
    assert_eq!(igd.control.path, "/ctl/IPConn");
    assert_eq!(igd.add_tcp(4712, 7200), Ok(7200));
    assert_eq!(igd.external_address(), Ok(Ipv4Addr::new(198, 51, 100, 4)));
    assert_eq!(igd.delete_tcp(4712), Ok(()));
    let _description = seen.recv().unwrap();
    let add = seen.recv().unwrap();
    assert!(add.starts_with("POST /ctl/IPConn HTTP/1.1\r\n"), "{add}");
    assert!(add.contains("SOAPACTION: \"urn:schemas-upnp-org:service:WANIPConnection:1#AddPortMapping\""));
    assert!(add.contains("<NewInternalClient>127.0.0.1</NewInternalClient>"));
    assert!(add.contains("<NewExternalPort>4712</NewExternalPort><NewProtocol>TCP</NewProtocol>"));
    assert!(add.contains("<NewLeaseDuration>7200</NewLeaseDuration>"));
    assert!(seen.recv().unwrap().contains("#GetExternalIPAddress\""));
    assert!(seen.recv().unwrap().contains("<u:DeletePortMapping"));
}

#[test]
fn upnp_falls_back_to_a_permanent_lease_on_725_and_names_other_refusals() {
    let (location, seen) = fake_igd(
        vec![
            soap_error(725, "OnlyPermanentLeasesSupported"),
            soap_ok("AddPortMapping", ""),
            soap_error(718, "ConflictInMappingEntry"),
        ],
        false,
    );
    let igd = upnp::Igd::from_location(&location).unwrap();
    assert_eq!(igd.add_tcp(4712, 7200), Ok(0));
    let _description = seen.recv().unwrap();
    assert!(seen.recv().unwrap().contains("<NewLeaseDuration>7200</NewLeaseDuration>"));
    assert!(seen.recv().unwrap().contains("<NewLeaseDuration>0</NewLeaseDuration>"));
    assert_eq!(
        igd.add_tcp(4713, 0),
        Err(NatError::Refused("UPnP 718 ConflictInMappingEntry".into()))
    );
}

#[test]
fn the_home_router_opens_and_closes_a_port_when_asked_live() {
    if std::env::var_os("LORDS2_NAT_LIVE").is_none() {
        return;
    }
    let mapping = l2_net::open_port(4712).expect("a router that speaks NAT-PMP or UPnP");
    eprintln!("mapped by {} to {} for {} s", mapping.via.name(), mapping.external, mapping.lifetime);
    assert!(!mapping.external.ip().is_private(), "{:?}", mapping.external);
    l2_net::close_port(&mapping).expect("the mapping closes");
}
