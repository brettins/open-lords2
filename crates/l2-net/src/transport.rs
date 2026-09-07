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
//! **There is no TCP implementation in this crate**, and that is a
//! decision rather than an omission. The whole of `l2-net` has to pass
//! its tests on a bare checkout with no game install and no network. A
//! socket implementation whose tests are skipped by default is code
//! that compiles and is never run, which for a networking layer is the
//! worst of both worlds: it looks finished and it has never worked.
//! What is here instead is everything a TCP implementation would need
//! and everything the layers above it can be tested against:
//!
//! * [`Transport`] — the trait from §7, unchanged apart from a
//!   `peers()` method argued for below.
//! * [`FrameReader`] / [`frame`] — length-prefix framing, which §7 puts
//!   *above* the trait so a datagram implementation and a stream
//!   implementation present the same interface. This is the part of a
//!   TCP transport that is actually easy to get wrong — a message split
//!   across two `read` calls, or two messages arriving in one — and it
//!   is fully tested here, byte-splitting included.
//! * [`Loopback`] — a complete in-process network with controllable
//!   latency, reordering and partitioning, so that [`Session`] and the
//!   desync detector are exercised for real rather than in principle.
//!
//! [`Session`]: crate::Session
//!
//! # For whoever writes the socket implementation
//!
//! Two details from §7 that are easy to get wrong and expensive to
//! diagnose, repeated here because they belong next to the trait:
//!
//! * **`set_nodelay(true)` on every socket.** Nagle's algorithm buffers
//!   small writes waiting for an ACK, which is exactly wrong for one
//!   small packet every 100 ms. It can add most of a round trip to
//!   every tick and it presents as "the network is slow" when it is the
//!   local stack holding the data.
//! * **Read sockets on their own thread**, handing complete messages to
//!   a queue that the simulation drains at one fixed point in the tick.
//!   The simulation must never ask "has anything arrived yet?"
//!   mid-tick: how much has arrived by any given instant is exactly the
//!   scheduler-dependent value D-5 forbids from reaching `step()`.
//!   [`Transport::poll`] is shaped for that — it drains, it does not
//!   block, and [`Session`] calls it before stepping and not during.

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

    fn broadcast(&mut self, bytes: &[u8]) -> Result<(), TransportError> {
        for peer in self.peers() {
            self.send(peer, bytes)?;
        }
        Ok(())
    }
}

/// The largest message this crate will frame or accept.
///
/// One mebibyte. A tick packet is tens of bytes; a whole-world snapshot
/// for late join (§5) is the only thing that could approach this, and
/// the original's entire game state is about 955 KB (`docs/decisions.md`
/// D2). The limit exists so that a corrupt or hostile length prefix
/// cannot make us allocate a gigabyte before we notice.
pub const MAX_FRAME: usize = 1 << 20;

/// Length-prefix a message: four little-endian bytes, then the payload.
///
/// Framing lives above [`Transport`] rather than inside it precisely so
/// that both kinds of implementation present the same interface (§7).
pub fn frame(payload: &[u8]) -> Result<Vec<u8>, TransportError> {
    if payload.len() > MAX_FRAME {
        return Err(TransportError::FrameTooLong { len: payload.len() });
    }
    let mut out = Vec::with_capacity(payload.len() + 4);
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.extend_from_slice(payload);
    Ok(out)
}

/// Reassembles messages from a byte stream.
///
/// A stream transport does not deliver messages; it delivers bytes, in
/// whatever sizes the kernel felt like. Feed everything that arrives to
/// [`FrameReader::feed`] and take whole messages out with
/// [`FrameReader::next_message`]. Splitting a four-byte length prefix
/// across two reads is not a rare case — it is the case that happens on
/// the day of the demo — so `tests/transport.rs` feeds a stream one
/// byte at a time and expects the same messages out.
#[derive(Debug, Default)]
pub struct FrameReader {
    buf: Vec<u8>,
    /// How far into `buf` the consumed messages reach. Draining from
    /// the front of a `Vec` on every message would be quadratic on a
    /// busy stream; compaction happens once the consumed prefix is
    /// worth reclaiming.
    consumed: usize,
}

impl FrameReader {
    pub fn new() -> FrameReader {
        FrameReader::default()
    }

    pub fn feed(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
    }

    /// Bytes held but not yet formed into a message.
    pub fn buffered(&self) -> usize {
        self.buf.len() - self.consumed
    }

    pub fn next_message(&mut self) -> Result<Option<Vec<u8>>, TransportError> {
        let available = self.buffered();
        if available < 4 {
            return Ok(None);
        }
        let start = self.consumed;
        let len = u32::from_le_bytes([
            self.buf[start],
            self.buf[start + 1],
            self.buf[start + 2],
            self.buf[start + 3],
        ]) as usize;
        if len > MAX_FRAME {
            return Err(TransportError::FrameTooLong { len });
        }
        if available - 4 < len {
            return Ok(None);
        }
        let message = self.buf[start + 4..start + 4 + len].to_vec();
        self.consumed = start + 4 + len;
        if self.consumed == self.buf.len() {
            self.buf.clear();
            self.consumed = 0;
        } else if self.consumed >= 64 * 1024 {
            self.buf.drain(..self.consumed);
            self.consumed = 0;
        }
        Ok(Some(message))
    }
}

// ---------------------------------------------------------------------
// Loopback
// ---------------------------------------------------------------------

#[derive(Debug)]
struct Queued {
    from: PeerId,
    bytes: Vec<u8>,
    /// The recipient's poll count at which this becomes visible.
    ready_at: u64,
}

#[derive(Debug, Default)]
struct Mailbox {
    queue: VecDeque<Queued>,
    polls: u64,
    /// Extra polls before anything sent to this peer becomes visible.
    latency: u64,
    /// Deliver the newest ready message first.
    reorder: bool,
    /// Drop everything sent to this peer.
    partitioned: bool,
    /// Queue everything sent to this peer without delivering it, until
    /// released.
    held: Vec<Queued>,
    holding: bool,
}

#[derive(Debug, Default)]
struct Switch {
    boxes: Vec<(PeerId, Mailbox)>,
}

/// An in-process network: several [`Endpoint`]s that can talk to each
/// other, with no sockets and no threads.
///
/// This is what makes §6's central claim testable on one machine with
/// no network: *two simulations in one process, fed identical commands,
/// must produce identical checksums.* It is also where the interesting
/// failures are injected — [`Loopback::set_latency`],
/// [`Loopback::set_reorder`] and [`Loopback::partition`] cover the
/// three ways a real network makes a lockstep session behave
/// differently, and the session must survive all three with identical
/// results.
///
/// ```
/// # use l2_net::{Loopback, PeerId, Transport};
/// let net = Loopback::with_peers(&[PeerId(0), PeerId(1)]);
/// let mut a = net.endpoint(PeerId(0));
/// let mut b = net.endpoint(PeerId(1));
/// a.send(PeerId(1), b"hello").unwrap();
/// assert_eq!(b.poll(), Some((PeerId(0), b"hello".to_vec())));
/// ```
#[derive(Debug, Clone, Default)]
pub struct Loopback {
    switch: Arc<Mutex<Switch>>,
}

impl Loopback {
    pub fn new() -> Loopback {
        Loopback::default()
    }

    pub fn with_peers(peers: &[PeerId]) -> Loopback {
        let net = Loopback::new();
        for &peer in peers {
            net.add_peer(peer);
        }
        net
    }

    pub fn add_peer(&self, peer: PeerId) {
        let mut switch = self.lock();
        if !switch.boxes.iter().any(|(id, _)| *id == peer) {
            switch.boxes.push((peer, Mailbox::default()));
            // Sorted, so `peers()` is stable no matter what order they
            // were added in.
            switch.boxes.sort_by_key(|(id, _)| *id);
        }
    }

    /// An endpoint that sends *as* `me`.
    pub fn endpoint(&self, me: PeerId) -> Endpoint {
        self.add_peer(me);
        Endpoint { me, net: self.clone() }
    }

    /// Hold everything sent to `peer` for `polls` extra polls.
    ///
    /// Latency in polls rather than milliseconds, because a clock has
    /// no place in a deterministic test (D-5) and because "how many
    /// times did the loop go round before this arrived" is the quantity
    /// the session actually cares about.
    pub fn set_latency(&self, peer: PeerId, polls: u64) {
        self.with_box(peer, |b| b.latency = polls);
    }

    /// Deliver newest-ready-first to `peer`.
    ///
    /// The lockstep core must produce identical results with this on or
    /// off. If it ever does not, §4's "never in arrival order" has been
    /// violated somewhere.
    pub fn set_reorder(&self, peer: PeerId, reorder: bool) {
        self.with_box(peer, |b| b.reorder = reorder);
    }

    /// Drop everything sent to `peer` from now on — **lossy**, and
    /// unrecoverable.
    ///
    /// This models a link that discards. Note what it does *to a
    /// lockstep session*, which `tests/lockstep.rs` demonstrates: a
    /// dropped tick packet is not a hiccup, it is the end of the
    /// session, because tick `N` can never be simulated without it and
    /// nothing in the design retransmits. That is not a flaw in the
    /// session — it is the reason `docs/netcode.md` §7 lists reliable,
    /// ordered delivery as a *requirement* rather than a preference,
    /// and the reason lockstep has no use for an unreliable channel.
    ///
    /// Use [`Loopback::hold`] to model the case a reliable transport
    /// actually produces.
    pub fn partition(&self, peer: PeerId, dropped: bool) {
        self.with_box(peer, |b| b.partitioned = dropped);
    }

    /// Stop delivering to `peer`, but **keep** what is sent — a stalled
    /// connection rather than a lossy one.
    ///
    /// This is what a peer going quiet looks like over TCP: nothing is
    /// lost, everything is late, and the moment the link recovers the
    /// backlog arrives in order. It is the case §4's stalling rule is
    /// written for, and the one a session must be able to resume from.
    pub fn hold(&self, peer: PeerId, holding: bool) {
        self.with_box(peer, |mailbox| {
            mailbox.holding = holding;
            if !holding {
                // Released in the order they were sent, which is what
                // an ordered transport guarantees.
                let now = mailbox.polls;
                for mut queued in std::mem::take(&mut mailbox.held) {
                    queued.ready_at = now + mailbox.latency;
                    mailbox.queue.push_back(queued);
                }
            }
        });
    }

    /// Messages queued for `peer` and not yet polled, held ones
    /// included.
    pub fn pending(&self, peer: PeerId) -> usize {
        let switch = self.lock();
        switch
            .boxes
            .iter()
            .find(|(id, _)| *id == peer)
            .map(|(_, b)| b.queue.len() + b.held.len())
            .unwrap_or(0)
    }

    fn with_box(&self, peer: PeerId, f: impl FnOnce(&mut Mailbox)) {
        self.add_peer(peer);
        let mut switch = self.lock();
        if let Some((_, mailbox)) = switch.boxes.iter_mut().find(|(id, _)| *id == peer) {
            f(mailbox);
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Switch> {
        // A poisoned mutex here means a test panicked while holding it.
        // Recovering is right: the data is a queue of byte vectors and
        // cannot be left in an inconsistent state by a panic elsewhere,
        // and turning one test failure into a cascade of unwrap panics
        // hides the original.
        self.switch.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// One peer's view of a [`Loopback`].
#[derive(Debug, Clone)]
pub struct Endpoint {
    me: PeerId,
    net: Loopback,
}

impl Endpoint {
    pub fn id(&self) -> PeerId {
        self.me
    }
}

impl Transport for Endpoint {
    fn send(&mut self, peer: PeerId, bytes: &[u8]) -> Result<(), TransportError> {
        if bytes.len() > MAX_FRAME {
            return Err(TransportError::FrameTooLong { len: bytes.len() });
        }
        let mut switch = self.net.lock();
        let Some((_, mailbox)) = switch.boxes.iter_mut().find(|(id, _)| *id == peer) else {
            return Err(TransportError::NoSuchPeer(peer));
        };
        if mailbox.partitioned {
            // Silently dropped, exactly as a real partition does. The
            // sender of a lost packet gets no error either.
            return Ok(());
        }
        let ready_at = mailbox.polls + mailbox.latency;
        let queued = Queued { from: self.me, bytes: bytes.to_vec(), ready_at };
        if mailbox.holding {
            mailbox.held.push(queued);
        } else {
            mailbox.queue.push_back(queued);
        }
        Ok(())
    }

    fn poll(&mut self) -> Option<(PeerId, Vec<u8>)> {
        let mut switch = self.net.lock();
        let (_, mailbox) = switch.boxes.iter_mut().find(|(id, _)| *id == self.me)?;
        mailbox.polls += 1;
        let now = mailbox.polls;
        let ready: Vec<usize> = mailbox
            .queue
            .iter()
            .enumerate()
            .filter(|(_, q)| q.ready_at < now)
            .map(|(i, _)| i)
            .collect();
        let index = if mailbox.reorder { *ready.last()? } else { *ready.first()? };
        let queued = mailbox.queue.remove(index)?;
        Some((queued.from, queued.bytes))
    }

    fn peers(&self) -> Vec<PeerId> {
        let switch = self.net.lock();
        switch.boxes.iter().map(|(id, _)| *id).filter(|id| *id != self.me).collect()
    }
}
