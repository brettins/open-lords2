//! The transport seam, message framing, and an in-process loopback.
//!
//! # What is here and what is not
//!
//! `docs/netcode.md` §7 recommends plain `std::net::TcpStream` behind a
//! narrow trait, and argues it well: lockstep has no use for an
//! unreliable channel, and TCP's head-of-line blocking — the usual
//! disqualifier for game networking — costs nothing when tick `N + 1`
//! could not have been simulated before tick `N` anyway. The stall TCP
//! imposes is the stall the design already has.
//!
//! **The TCP implementation lives in [`tcp`](crate::tcp)**, and it was
//! written only once it could be tested the way everything else here
//! is: on a bare checkout, with no game install and no second machine.
//! That was the standing condition
//! it is the reason this module existed alone for as long as it did — a
//! socket implementation whose tests are skipped by default is code
//! that compiles and is never run, which for a networking layer is the
//! worst of both worlds: it looks finished and it has never worked.
//! Loopback sockets on an OS-assigned port turn out to satisfy the
//! condition exactly, so `tests/tcp.rs` runs on every `cargo test` with
//! nothing ignored and nothing conditional.
//!
//! What is here is the seam itself
//! tested against the [`Loopback`] with no sockets at all:
//!
//! * [`Transport`] — the trait from §7, unchanged apart from a
//!   `peers()` method argued for below.
//! * [`FrameReader`] / [`frame`] — length-prefix framing. This is the
//! part of a TCP transport that is easy to get wrong — a
//!   message split across two `read` calls, or two messages arriving in
//!   one — and it is fully tested here, byte-splitting included.
//!
//!   §7 says framing belongs *above* the trait, so that a datagram
//!   implementation and a stream implementation present the same
//!   interface. The trait below says the opposite in as many words:
//!   `send` takes one complete message and "a stream implementation
//!   uses [`FrameReader`] to make that true". **The trait is right and
//!   §7's phrasing is not**, and building [`tcp`](crate::tcp) is what
//!   settled it: framing *above* the trait means every caller must know
//!   it is talking to a stream, which is the one thing §7 says nothing
//!   above the seam may know. So [`tcp`](crate::tcp) frames internally,
//!   and callers hand it whole messages. A caller that frames anyway —
//!   `tests/lockstep.rs` does, over a [`Loopback`] that needs no
//!   framing — is not broken, it just pays eight prefix bytes instead
//!   of four.
//! * [`Loopback`] — a complete in-process network with controllable
//! latency, reordering and partitioning
//! desync detector are exercised for real.
//!
//! [`Session`]: crate::Session
//!
//! # For whoever writes the next socket implementation
//!
//! Two details from §7 that are easy to get wrong and expensive to
//! diagnose, repeated here because they belong next to the trait:
//!
//! * **`set_nodelay(true)` on every socket.** Nagle's algorithm buffers
//!   small writes waiting for an ACK, which is exactly wrong for one
//!   small packet every 100 ms. It can add most of a round trip to
//!   every tick and it presents as "the network is slow" when it is the
//!   local stack holding the data.
//! * **The simulation must never ask "has anything arrived yet?"
//!   mid-tick**: how much has arrived by any given instant is exactly
//!   the scheduler-dependent value D-5 forbids from reaching `step()`.
//!   [`Transport::poll`] is shaped for that — it drains, it does not
//!   block, and [`Session`] calls it before stepping and not during.
//!   §7 gets there with a reader thread; [`tcp`](crate::tcp) gets there
//!   with non-blocking sockets and no thread at all, which is the same
//!   guarantee with less machinery. Either is fine. What is not fine is
//!   a `poll` that can block, because the session calls it inside the
//!   frame.

mod frame_part;
pub use frame_part::*;
mod loopback;
pub use loopback::*;

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// A transport-level identity: "the other end of that connection".
///
/// Deliberately not the same type as
/// [`PlayerSlot`](crate::PlayerSlot). A slot is who someone is in the
/// game; a peer id is where their bytes come from. They differ the
/// moment anyone reconnects, and in the star topology §7 describes they
/// differ permanently — the host relays a command from player 3 to
/// player 4, so the peer it arrived from is the host, not player 3. A
/// single type for both would make that bug invisible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PeerId(pub u32);

impl core::fmt::Display for PeerId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "peer {}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportError {
    /// No such peer, or the connection is gone.
    NoSuchPeer(PeerId),
    /// The peer's connection has closed.
    Disconnected(PeerId),
    /// A frame longer than [`MAX_FRAME`].
    FrameTooLong { len: usize },
    /// Anything the implementation wants to report. A `String` rather
    /// than a wrapped `std::io::Error` so that this crate stays free of
    /// `std::io` and an implementation over something that is not a
    /// socket is not forced to invent one.
    Io(String),
}

impl core::fmt::Display for TransportError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            TransportError::NoSuchPeer(p) => write!(f, "no such peer: {p}"),
            TransportError::Disconnected(p) => write!(f, "{p} has disconnected"),
            TransportError::FrameTooLong { len } => {
                write!(f, "message of {len} bytes exceeds the {MAX_FRAME}-byte limit")
            }
            TransportError::Io(detail) => write!(f, "transport error: {detail}"),
        }
    }
}

impl std::error::Error for TransportError {}

/// Moves opaque messages between peers, and nothing else.
///
/// The trait is the one in §7 with one addition: `peers`. Without it
/// the caller has to keep its own list of who is connected in order to
/// broadcast, and a caller-side list is a list that can drift from the
/// transport's — which shows up as one player silently not receiving a
/// turn. The transport already knows; asking it is free.
///
/// **Message boundaries are the transport's problem.** `send` takes one
/// complete message and `poll` returns one complete message. A stream
/// implementation uses [`FrameReader`] to make that true; a datagram
/// implementation gets it from the medium. Nothing above this trait may
/// know which it is talking to.
pub trait Transport {
    fn send(&mut self, peer: PeerId, bytes: &[u8]) -> Result<(), TransportError>;

    /// The next complete message, or `None` if nothing has arrived.
    /// Never blocks.
    fn poll(&mut self) -> Option<(PeerId, Vec<u8>)>;

    /// Everyone currently connected, in a stable order.
    ///
    /// Stable so that a broadcast happens in the same order every time.
    /// That does not affect correctness — the receiving side orders
    /// commands by slot regardless (D-7) — but a transport that
    /// reordered its own peer list per call would make every network
    /// bug irreproducible.
    fn peers(&self) -> Vec<PeerId>;

    /// Send to every peer. Attempts all of them, then reports the first failure.
    ///
    /// Using `?` inside the loop — which this originally did — aborts on the
    /// first failing peer and silently skips every peer after it. A host whose
    /// player 2 has just dropped then never sends the turn to players 3, 4 and
    /// 5
    ///
    /// `Loopback` cannot produce that failure at all: a partitioned peer still
    /// returns `Ok`. So the whole suite passed while the bug sat here, and it
    /// took one real socket and one ordinary disconnection to expose it — worth
    /// remembering next time an in-process double looks like adequate coverage.
    fn broadcast(&mut self, bytes: &[u8]) -> Result<(), TransportError> {
        let mut first_err = None;
        for peer in self.peers() {
            if let Err(e) = self.send(peer, bytes) {
                if first_err.is_none() {
                    first_err = Some(e);
                }
            }
        }
        match first_err {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }
}

