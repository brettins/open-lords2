//! A TCP implementation of [`Transport`], and the only part of this
//! crate that touches the operating system.
//!
//! `docs/netcode.md` §7 chose plain `std::net::TcpStream` behind the
//! [`Transport`] trait and argued it well: lockstep has no use for an
//! unreliable channel, TCP's head-of-line blocking is the stall the
//! design already has, and the standard library costs no dependency and
//! no C toolchain. This module is that choice, built.
//!
//! Until it existed, §7's analysis was *static* — the crates were read
//! What
//! follows is the part that had to be run to be believed.
//!
//! # The rule this module lives under
//!
//! **`std::net`, `std::time` and `std::thread` may appear here and in
//! `tests/tcp.rs`. They may not appear anywhere else in this crate, and
//! nothing here may be reachable from `step()`.**
//!
//! That boundary is the whole reason the rest of the crate is testable.
//! D-5 forbids the simulation from seeing wall-clock time or the
//! scheduler, and the surest way to obey a rule like that is to have no
//! clock to consult — which is exactly how [`Session`] is written. A
//! socket, by contrast, cannot avoid the scheduler: *how much has
//! arrived by now* is a scheduler-dependent quantity, and it is
//! the value D-5 says must never reach a tick. Keeping it on
//! this side of the trait is what lets `tests/lockstep.rs` run two
//! peers with no clock and no threads and still be a real test of the
//! thing that matters, while this file deals with the part that is
//! irreducibly non-deterministic.
//!
//! The transport therefore hands the session *messages*, never timing.
//! [`Session::advance`] blocks on a missing tick regardless of when it
//! arrives, so a slow socket changes how long the wall clock says a
//! session took and changes nothing about its checksums. The tests
//! below assert exactly that.
//!
//! # No reader thread — a deliberate departure from §7
//!
//! §7 recommends reading sockets on their own thread, handing complete
//! messages to a queue the simulation drains at a fixed point. The
//! *goal* is right and is honoured; the mechanism is not needed. Every
//! socket here is non-blocking, so [`TcpTransport::poll`] drains
//! whatever the kernel has and returns — it never blocks, which is what
//! the trait already promises and what a thread was going to buy.
//!
//! What a thread would have cost is worth stating, because it is the
//! reason this was not done:
//!
//! * a channel and a shared queue between the reader and the session,
//!   whose *interleaving* is the scheduler-dependent quantity D-5 is
//!   about;
//! * a shutdown protocol, because a blocked `read` does not notice that
//!   the game has quit;
//! * tests that are timing-dependent by construction, which is the
//!   category of test the whole crate is organised to avoid.
//!
//! One caller thread, one non-blocking drain per frame, at the same
//! fixed point in the loop `tests/lockstep.rs` already uses. If a
//! future transport needs threads (a blocking TLS handshake,
//! say), it goes behind this same trait and the session does not learn
//! about it.
//!
//! # Framing is done here
//!
//! [`Transport`]'s contract is that `send` takes one complete message
//! and `poll` returns one complete message. A stream delivers bytes in
//! whatever sizes the kernel felt like, so this module length-prefixes
//! with [`frame`] on the way out and reassembles with [`FrameReader`]
//! on the way in. That is the one piece of a TCP transport that was
//! already written and already tested against every byte-split of a
//! stream, and it is the piece that would otherwise have gone wrong.
//!
//! Note a wrinkle worth knowing: §7 and this module's parent say
//! framing belongs *above* the trait, and `tests/lockstep.rs` frames
//! above it before handing bytes to a [`Loopback`](crate::Loopback).
//! Both can be true — a caller that frames on top of a transport that
//! frames underneath pays eight bytes instead of four, and the
//! messages still come out whole — but only one of them is the trait's
//! actual contract, and it is the one written on the trait. Sockets are
//! framed here so that swapping [`Loopback`](crate::Loopback) for
//! [`TcpTransport`] changes nothing above the seam.
//!
//! [`Session`]: crate::Session
//! [`Session::advance`]: crate::Session::advance
//!
//! # A worked example
//!
//! Two transports in one process over a real loopback socket — which is
//! also how `tests/tcp.rs` is written, and it needs no game install, no
//! second machine and no fixed port.
//!
//! ```
//! use l2_net::{PeerId, TcpTransport, Transport};
//!
//! // Port 0 asks the OS for a free port, so the test cannot collide
//! // with a running game or with another test.
//! let mut host = TcpTransport::listen("127.0.0.1:0").unwrap();
//! let addr = host.local_addr().unwrap();
//!
//! let mut client = TcpTransport::connect(addr, PeerId(100)).unwrap();
//! client.send(PeerId(100), b"hello").unwrap();
//!
//! // The host learns about the connection and the message from the
//! // same drain. Spin because delivery is the kernel's business and
//! // not ours - see `tests/tcp.rs` for why this loop is honest.
//! let mut got = None;
//! for _ in 0..10_000 {
//!     if let Some(message) = host.poll() {
//!         got = Some(message);
//!         break;
//!     }
//!     std::thread::sleep(std::time::Duration::from_millis(1));
//! }
//! assert_eq!(got, Some((PeerId(0), b"hello".to_vec())));
//! assert_eq!(host.peers(), vec![PeerId(0)]);
//! ```

mod transport;
pub use transport::*;

use std::collections::VecDeque;
use std::io::{ErrorKind, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::time::Duration;

use crate::transport::{frame, FrameReader, PeerId, Transport, TransportError};

/// How much this transport will read from one socket in one visit.
///
/// A bound so that one peer flooding
/// the link cannot make a single [`TcpTransport::poll`] run for
/// unbounded time inside a caller's frame. The next poll picks up where
/// this one stopped, and the round-robin cursor means it is somebody
/// else's turn first.
const READ_BUDGET: usize = 256 * 1024;

/// The read buffer handed to the kernel, reused across polls.
const READ_CHUNK: usize = 64 * 1024;

/// The most unsent bytes one peer may accumulate before the connection
/// is declared dead.
///
/// [`TcpTransport::send`] never blocks, so when the kernel's send
/// buffer is full the remainder is queued here — and a peer that has
/// stopped reading entirely would otherwise grow that queue until the
/// process dies. Four mebibytes is far above anything legitimate: a
/// tick packet is tens of bytes, and the largest message the design has
/// is a whole-world snapshot for late join, about 955 KB
/// (`docs/decisions.md` D2). Four of those queued unread means the peer
/// is not coming back, and saying so is better than an allocation
/// failure with no explanation.
pub const MAX_OUTBOX: usize = 4 * 1024 * 1024;

/// The errors that mean "this peer is gone"
/// wrong".
///
/// Worth spelling out because the obvious form of the check — treat a
/// zero-length read as the end and everything else as an error — is
/// wrong on Windows. A peer that closes while data it never read is
/// still sitting in its receive buffer causes an **RST**, not a FIN,
/// and the other end sees `ConnectionReset` (WSAECONNRESET, 10054)
/// where it expected `Ok(0)`. That is the ordinary way a game exits:
/// the player alt-F4s mid-tick with our last packet unread. Reporting
/// it as an I/O fault instead of a disconnect would turn the most
/// common ending a session has into an unexplained error.
fn classify(id: PeerId, e: &std::io::Error) -> TransportError {
    match e.kind() {
        ErrorKind::ConnectionReset
        | ErrorKind::ConnectionAborted
        | ErrorKind::BrokenPipe
        | ErrorKind::NotConnected
        | ErrorKind::UnexpectedEof => TransportError::Disconnected(id),
        _ => TransportError::Io(format!("{id}: {e}")),
    }
}

fn io(e: &std::io::Error) -> TransportError {
    TransportError::Io(e.to_string())
}

/// [`Transport`] over TCP: one optional listener and a connection per
/// peer.
///
/// The same type is used by both ends. A host calls [`listen`] and
/// grows peers as they arrive; a client calls [`connect`] and has
/// exactly one. §7's star topology for five players is a host whose
/// transport has four connections and relays between them — which is
/// invisible above the trait, and is why [`PeerId`] and
/// [`PlayerSlot`](crate::PlayerSlot) are different types.
///
/// # Peer ids are assigned in accept order and never reused
///
/// The first peer a host accepts is `PeerId(0)`, the second `PeerId(1)`,
/// and a player who drops and reconnects gets a *new* id
/// their old one. That is deliberate: a reconnecting player is the same
/// [`PlayerSlot`](crate::PlayerSlot) on a different connection, and
/// reusing the id would let a stale reference to the dead connection
/// quietly address the new one.
///
/// # What this type does not do
///
/// No NAT traversal, no discovery, no encryption, no reconnection. §7
/// and §8 are explicit about all four: the host must be reachable, the
/// address is typed by a human, and a modified client is not a threat
/// this design defends against. A reconnect is a new connection and a
/// new [`PeerId`], and stitching it back to a slot is the caller's
/// decision because only the caller knows whether the session can still
/// be caught up.
///
/// [`listen`]: TcpTransport::listen
/// [`connect`]: TcpTransport::connect
#[derive(Debug)]
pub struct TcpTransport {
    listener: Option<TcpListener>,
    /// Sorted by id, which accept order already produces; kept sorted
    /// so [`Transport::peers`] is stable per its contract.
    conns: Vec<Connection>,
    next_id: u32,
    /// Round-robin start for `poll`, so one chatty peer cannot starve
    /// the others out of a five-player session.
    cursor: usize,
    joined: Vec<PeerId>,
    gone: Vec<PeerId>,
    faults: Vec<(Option<PeerId>, TransportError)>,
    scratch: Vec<u8>,
}

