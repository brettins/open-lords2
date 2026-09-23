//! net-tests-firewall-prompts: a test that binds every interface raises a Windows
//! firewall prompt per test binary. Tests bind 127.0.0.1; only the app's lobby
//! screen binds every interface (`Request::every_interface`).

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::path::Path;

/// What a test hosts on: loopback, any free port.
pub const ANY_PORT: SocketAddr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0));

const BINDS: [&str; 6] =
    ["::listen(", "Driver::host(", "Request::Host {", "TcpListener::bind(", "UdpSocket::bind(", "every_interface("];
const LOOPBACK: [&str; 3] = ["127.0.0.1", "LOCALHOST", "ANY_PORT"];

fn rust_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// Ablation: `Request::Host { addr: ANY_PORT }` in `lobby_tcp::two_peers` back to
/// `Request::every_interface(0)`.
#[test]
fn every_bind_in_a_test_names_a_loopback_address() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("crates/");
    let mut files = Vec::new();
    for krate in std::fs::read_dir(crates).expect("crates/").flatten() {
        rust_files(&krate.path().join("tests"), &mut files);
    }
    assert!(files.len() > 20, "found only {} test files", files.len());
    let mut open = Vec::new();
    for file in files.iter().filter(|f| !f.ends_with("net_game/loopback.rs")) {
        let text = std::fs::read_to_string(file).expect("a test source");
        for (n, line) in text.lines().enumerate() {
            let code = line.split("//").next().unwrap_or("");
            let binds = BINDS.iter().any(|b| code.contains(b)) && !code.contains("fn ");
            if binds && (code.contains("every_interface") || !LOOPBACK.iter().any(|l| code.contains(l))) {
                open.push(format!("{}:{}: {}", file.display(), n + 1, line.trim()));
            }
        }
    }
    assert!(open.is_empty(), "binds that are not loopback:\n{}", open.join("\n"));
}
