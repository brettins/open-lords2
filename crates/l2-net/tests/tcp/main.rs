
mod transport;
mod simulation;

#[path = "../common/mod.rs"]
mod common;

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use l2_net::{
    PeerId, TcpTransport, Transport,
};

const PATIENCE: Duration = Duration::from_secs(30);

fn host() -> (TcpTransport, SocketAddr) {
    let host = TcpTransport::listen("127.0.0.1:0").expect("binding a loopback port");
    let addr = host.local_addr().expect("a listener knows its address");
    (host, addr)
}

fn deadline() -> Instant {
    Instant::now() + PATIENCE
}

fn breathe() {
    std::thread::sleep(Duration::from_millis(1));
}

#[track_caller]
fn wait_for_message(net: &mut TcpTransport) -> (PeerId, Vec<u8>) {
    let until = deadline();
    loop {
        if let Some(message) = net.poll() {
            return message;
        }
        assert!(Instant::now() < until, "nothing arrived within {PATIENCE:?}");
        breathe();
    }
}

#[track_caller]
fn wait_for_peers(net: &mut TcpTransport, count: usize) {
    let until = deadline();
    while net.peers().len() < count {
        net.accept_pending().expect("accepting");
        assert!(
            Instant::now() < until,
            "only {} of {count} peers connected within {PATIENCE:?}",
            net.peers().len()
        );
        breathe();
    }
}

#[track_caller]
fn wait_for_departure(net: &mut TcpTransport) -> Vec<PeerId> {
    let until = deadline();
    loop {
        while net.poll().is_some() {}
        let gone = net.take_disconnected();
        if !gone.is_empty() {
            return gone;
        }
        assert!(Instant::now() < until, "no departure within {PATIENCE:?}");
        breathe();
    }
}

