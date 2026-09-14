//! The TCP transport, over real sockets.
//!
//! **Every test in this file opens sockets, and every one of them runs
//! on a bare checkout.** Nothing here is `#[ignore]`d, nothing is gated
//! on an environment variable, nothing needs a game install and nothing
//! needs a second machine: both ends are on the loopback interface, in
//! this process, driven by the test. That was the standing condition
//! for writing a socket implementation at all — `src/transport/mod.rs`
//! spent a long time arguing that a transport whose tests are skipped
//! by default is code that looks finished and has never worked — and it
//! turns out to cost nothing.
//!
//! Two mechanics make that true and are worth copying:
//!
//! * **Port 0.** Every listener asks the OS for a free port, so nothing
//!   here collides with a running game, with another test, or with the
//!   same test running twice. `DEFAULT_PORT` is never bound.
//! * **`127.0.0.1`, never `0.0.0.0`.** Binding the loopback address
//!   specifically keeps the Windows firewall out of it; binding all
//!   interfaces raises a prompt the first time, which is exactly the
//!   "environment-dependent test" this file exists to avoid.
//!
//! # Why there is a clock in this file and nowhere else
//!
//! `std::time` and `std::thread::sleep` appear below, and they appear
//! in `src/tcp/mod.rs`'s neighbourhood and nowhere else in the crate. That
//! boundary is the point.
//!
//! Delivery is the kernel's business: when a byte written on one socket
//! becomes readable on another is scheduler-dependent, and no amount of
//! care makes it otherwise. So the socket tests wait, and waiting needs
//! a clock. What must *not* happen is that quantity reaching the
//! simulation — D-5 — and the tests below assert precisely that it does
//! not: [`the_socket_changes_the_timing_and_not_the_checksums`] runs
//! one schedule over real sockets and the identical schedule over the
//! in-process [`Loopback`], and requires the two to produce the same
//! checksum for every tick. The network decides *when*; it never
//! decides *what*.
//!
//! That is why `tests/lockstep.rs` remains the more important file. It
//! has no clock at all, so it can assert things about ordering that a
//! socket test can only sample.
//!
//! [`the_socket_changes_the_timing_and_not_the_checksums`]:
//!     fn.the_socket_changes_the_timing_and_not_the_checksums.html

mod transport;
pub use transport::*;
mod simulation;
pub use simulation::*;

#[path = "../common/mod.rs"]
mod common;

use std::io::Write;
use std::net::{SocketAddr, TcpStream};
use std::time::{Duration, Instant};

use common::{Order, ToySim};
use l2_net::{
    decode_all, frame, Advance, Canonical, Config, Endpoint, Fixed, FrameReader, HaltReason,
    Hello, Loopback, Message, Mismatch, PeerId, PlayerSlot, Session, SessionError, TcpTransport,
    Tick, Transport, TransportError, MAX_FRAME, MAX_OUTBOX,
};

/// How long any test will wait for the loopback interface before
/// calling it a failure.
///
/// Absurdly generous — everything here completes in milliseconds — because
/// the only thing this bound is for is turning a hang into a readable
/// failure on a machine under load. A tight timeout would make the
/// suite flaky, which is worse than slow.
const PATIENCE: Duration = Duration::from_secs(30);

/// A listener on an OS-assigned loopback port, and its address.
fn host() -> (TcpTransport, SocketAddr) {
    let host = TcpTransport::listen("127.0.0.1:0").expect("binding a loopback port");
    let addr = host.local_addr().expect("a listener knows its address");
    (host, addr)
}

fn deadline() -> Instant {
    Instant::now() + PATIENCE
}

fn breathe() {
    // One millisecond.
    // the crate depends on this number; it only trades CPU for latency
    // in the tests.
    std::thread::sleep(Duration::from_millis(1));
}

/// Poll until a message arrives, or fail.
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

/// Accept until `count` peers are connected, or fail.
///
/// Uses `accept_pending` so that a message racing
/// the connection is not swallowed.
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

/// Poll until the transport reports a departure, or fail.
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

