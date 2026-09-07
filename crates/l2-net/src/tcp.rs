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
//! rather than compiled and no socket had ever been opened. What
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
//! precisely the value D-5 says must never reach a tick. Keeping it on
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
//! future transport genuinely needs threads (a blocking TLS handshake,
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
//! frames underneath simply pays eight bytes instead of four, and the
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

use std::collections::VecDeque;
use std::io::{ErrorKind, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::time::Duration;

use crate::transport::{frame, FrameReader, PeerId, Transport, TransportError};

/// How much this transport will read from one socket in one visit.
///
/// A bound rather than "until `WouldBlock`" so that one peer flooding
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

/// One connection, and everything half-delivered in either direction.
#[derive(Debug)]
struct Connection {
    id: PeerId,
    stream: TcpStream,
    /// Inbound reassembly. The tested one; see [`FrameReader`].
    reader: FrameReader,
    /// Complete messages, waiting for the caller to poll.
    inbox: VecDeque<Vec<u8>>,
    /// Bytes we have accepted from `send` and the kernel has not taken.
    out: Vec<u8>,
    /// How far into `out` the kernel has taken. Same compaction
    /// reasoning as [`FrameReader`]: draining from the front of a `Vec`
    /// on every write is quadratic on a busy link.
    out_done: usize,
    /// The peer closed, or the link failed. Either way nothing more
    /// will be sent on it; anything already in `inbox` is still
    /// delivered.
    dead: bool,
}

impl Connection {
    fn new(id: PeerId, stream: TcpStream) -> Result<Connection, TransportError> {
        // §7, and the single most expensive detail to get wrong: Nagle
        // buffers small writes waiting for an ACK, which is exactly
        // wrong for one small packet every 100 ms. It presents as "the
        // network is slow" when it is the local stack holding the data.
        stream.set_nodelay(true).map_err(|e| classify(id, &e))?;
        stream.set_nonblocking(true).map_err(|e| classify(id, &e))?;
        Ok(Connection {
            id,
            stream,
            reader: FrameReader::new(),
            inbox: VecDeque::new(),
            out: Vec::new(),
            out_done: 0,
            dead: false,
        })
    }

    fn pending_out(&self) -> usize {
        self.out.len() - self.out_done
    }

    /// Push as much of the outbox as the kernel will take. Never
    /// blocks.
    fn flush(&mut self) -> Result<(), TransportError> {
        while self.out_done < self.out.len() {
            match self.stream.write(&self.out[self.out_done..]) {
                Ok(0) => {
                    self.dead = true;
                    return Err(TransportError::Disconnected(self.id));
                }
                Ok(written) => self.out_done += written,
                Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                Err(e) => {
                    self.dead = true;
                    return Err(classify(self.id, &e));
                }
            }
        }
        if self.out_done == self.out.len() {
            self.out.clear();
            self.out_done = 0;
        } else if self.out_done >= 64 * 1024 {
            self.out.drain(..self.out_done);
            self.out_done = 0;
        }
        Ok(())
    }

    /// Read what has arrived and turn it into whole messages.
    fn fill(&mut self, scratch: &mut [u8]) -> Result<(), TransportError> {
        let mut budget = READ_BUDGET;
        let mut failure = None;
        while budget > 0 {
            match self.stream.read(scratch) {
                Ok(0) => {
                    // A clean close. Whatever is already in `reader`
                    // and `inbox` is still ours to deliver — that is
                    // what "reliable, ordered" means at the end of a
                    // connection as much as in the middle of one.
                    self.dead = true;
                    break;
                }
                Ok(read) => {
                    self.reader.feed(&scratch[..read]);
                    budget = budget.saturating_sub(read);
                }
                Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                Err(e) => {
                    self.dead = true;
                    failure = Some(classify(self.id, &e));
                    break;
                }
            }
        }
        loop {
            match self.reader.next_message() {
                Ok(Some(message)) => self.inbox.push_back(message),
                Ok(None) => break,
                Err(e) => {
                    // A length prefix we will not believe. The stream
                    // is unrecoverable from here — we do not know where
                    // the next message starts — so the connection goes,
                    // which is the point of the check: refuse before
                    // allocating, not after.
                    self.dead = true;
                    self.reader = FrameReader::new();
                    failure = Some(e);
                    break;
                }
            }
        }
        if self.dead && failure.is_none() && self.reader.buffered() > 0 {
            // The peer closed in the middle of a message. Those bytes
            // are unusable and are dropped either way; what matters is
            // that it is *said*, because the silent version of this is
            // a session that waits forever for a tick whose packet was
            // half-written when the process died — and "waiting
            // forever" is correct behaviour here, which makes it
            // indistinguishable from an ordinary stall unless somebody
            // reports the truncation.
            failure = Some(TransportError::Io(format!(
                "{} closed with {} bytes of an incomplete message",
                self.id,
                self.reader.buffered()
            )));
        }
        match failure {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }
}

/// The errors that mean "this peer is gone" rather than "something went
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
/// and a player who drops and reconnects gets a *new* id rather than
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

impl TcpTransport {
    /// Listen for peers. `addr` may be `"127.0.0.1:0"` to let the OS
    /// pick a free port, which is what the tests do and why they cannot
    /// collide with a running game.
    ///
    /// The listener is non-blocking: [`poll`](Transport::poll) accepts
    /// whoever is waiting and returns, so a host never stalls its own
    /// frame waiting for a connection that may never come.
    pub fn listen(addr: impl ToSocketAddrs) -> Result<TcpTransport, TransportError> {
        let listener = TcpListener::bind(addr).map_err(|e| io(&e))?;
        listener.set_nonblocking(true).map_err(|e| io(&e))?;
        Ok(TcpTransport {
            listener: Some(listener),
            conns: Vec::new(),
            next_id: 0,
            cursor: 0,
            joined: Vec::new(),
            gone: Vec::new(),
            faults: Vec::new(),
            scratch: vec![0u8; READ_CHUNK],
        })
    }

    /// Connect to a host, calling it `id`.
    ///
    /// The caller names the peer because a client has exactly one and
    /// there is nothing to discover: `PeerId(0)` is the obvious choice
    /// and any other is equally fine.
    pub fn connect(
        addr: impl ToSocketAddrs,
        id: PeerId,
    ) -> Result<TcpTransport, TransportError> {
        let stream = TcpStream::connect(addr).map_err(|e| io(&e))?;
        TcpTransport::adopt(stream, id)
    }

    /// Connect, giving up after `timeout`.
    ///
    /// The only [`Duration`] in the crate, and it is the caller's
    /// patience rather than a clock this code reads. It matters in
    /// practice: connecting to a host that is up but not listening —
    /// the wrong port typed into a dialog, or a firewall dropping
    /// rather than refusing — blocks for around twenty seconds on
    /// Windows before the OS gives up, which a player reads as a hang.
    pub fn connect_timeout(
        addr: SocketAddr,
        id: PeerId,
        timeout: Duration,
    ) -> Result<TcpTransport, TransportError> {
        let stream = TcpStream::connect_timeout(&addr, timeout).map_err(|e| io(&e))?;
        TcpTransport::adopt(stream, id)
    }

    /// Wrap a connection somebody else made. For a host that accepts on
    /// its own and for tests; the ordinary paths are [`listen`] and
    /// [`connect`].
    ///
    /// [`listen`]: TcpTransport::listen
    /// [`connect`]: TcpTransport::connect
    pub fn adopt(stream: TcpStream, id: PeerId) -> Result<TcpTransport, TransportError> {
        Ok(TcpTransport {
            listener: None,
            conns: vec![Connection::new(id, stream)?],
            next_id: id.0.saturating_add(1),
            cursor: 0,
            joined: vec![id],
            gone: Vec::new(),
            faults: Vec::new(),
            scratch: vec![0u8; READ_CHUNK],
        })
    }

    /// The address being listened on, `None` for a client.
    ///
    /// The port is the interesting part when `listen` was given port 0.
    pub fn local_addr(&self) -> Option<SocketAddr> {
        self.listener.as_ref().and_then(|l| l.local_addr().ok())
    }

    /// The remote address of a peer, for logs and for the "who is
    /// actually in this game" screen.
    pub fn peer_addr(&self, peer: PeerId) -> Option<SocketAddr> {
        self.conns.iter().find(|c| c.id == peer)?.stream.peer_addr().ok()
    }

    /// Whether Nagle is off on a peer's socket. Diagnostic, and the one
    /// §7 detail worth being able to assert in a test.
    pub fn nodelay(&self, peer: PeerId) -> Option<bool> {
        self.conns.iter().find(|c| c.id == peer)?.stream.nodelay().ok()
    }

    /// Accept everyone waiting, and return the ids they were given.
    ///
    /// [`poll`](Transport::poll) does this too, so a caller that drains
    /// every frame never has to call it. It is public because a lobby
    /// screen wants to accept without polling for game traffic yet.
    pub fn accept_pending(&mut self) -> Result<Vec<PeerId>, TransportError> {
        let mut accepted = Vec::new();
        let Some(listener) = &self.listener else { return Ok(accepted) };
        loop {
            match listener.accept() {
                Ok((stream, _addr)) => {
                    let id = PeerId(self.next_id);
                    self.next_id = self.next_id.wrapping_add(1);
                    let conn = Connection::new(id, stream)?;
                    self.conns.push(conn);
                    self.conns.sort_by_key(|c| c.id);
                    self.joined.push(id);
                    accepted.push(id);
                }
                Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                // A connection that arrived and went away again before
                // we accepted it — a port scanner, or a player who
                // cancelled. The pending connection is consumed either
                // way, so this terminates; the listener is fine and the
                // next one is somebody else.
                Err(e)
                    if matches!(
                        e.kind(),
                        ErrorKind::ConnectionAborted | ErrorKind::ConnectionReset
                    ) =>
                {
                    continue
                }
                Err(e) => return Err(io(&e)),
            }
        }
        Ok(accepted)
    }

    /// Peers that have connected since this was last called.
    ///
    /// The counterpart to [`take_disconnected`], and the reason
    /// [`Transport::peers`] is enough for a caller to keep its
    /// connection list from drifting (errata 7): the list is *here*,
    /// and both directions of change are reported.
    ///
    /// [`take_disconnected`]: TcpTransport::take_disconnected
    pub fn take_joined(&mut self) -> Vec<PeerId> {
        std::mem::take(&mut self.joined)
    }

    /// Peers that have gone since this was last called.
    ///
    /// A peer is reported here only once **everything it sent has been
    /// delivered**. A connection that closes with messages still in
    /// flight keeps returning them from `poll` first; the departure is
    /// announced after the last of them. Without that ordering a caller
    /// that halts on disconnect would throw away the final tick packets
    /// its peer legitimately sent, which is the difference between a
    /// clean end to a session and a spurious desync report.
    ///
    /// The trait cannot express this — it moves messages and nothing
    /// else — so it is an inherent method. A caller that only ever
    /// holds a `&mut dyn Transport` sees the departure as a peer
    /// vanishing from [`Transport::peers`], which is enough to stop
    /// broadcasting to them but not enough to say *why*.
    pub fn take_disconnected(&mut self) -> Vec<PeerId> {
        std::mem::take(&mut self.gone)
    }

    /// Errors that killed a connection, since this was last called.
    ///
    /// Reported rather than returned because they happen inside
    /// [`Transport::poll`], whose signature is `Option` and not
    /// `Result` — a drain loop must not have to distinguish "nothing
    /// arrived" from "something broke" on every iteration. Pair it with
    /// [`take_disconnected`](TcpTransport::take_disconnected): every
    /// per-peer fault here also produces a departure there.
    ///
    /// The `Option` is `None` for a failure of the *listener*, which
    /// belongs to no peer. It is an `Option` rather than a reserved
    /// [`PeerId`] value because a sentinel id is a value that looks
    /// like a peer to every piece of code that does not know about the
    /// convention.
    pub fn take_faults(&mut self) -> Vec<(Option<PeerId>, TransportError)> {
        std::mem::take(&mut self.faults)
    }

    /// Bytes accepted by [`Transport::send`] that the kernel has not
    /// taken yet.
    ///
    /// Normally zero. It grows when a peer stops reading, and at
    /// [`MAX_OUTBOX`] the connection is declared dead.
    pub fn pending_out(&self, peer: PeerId) -> usize {
        self.conns.iter().find(|c| c.id == peer).map_or(0, |c| c.pending_out())
    }

    /// Complete messages received and not yet polled.
    pub fn pending_in(&self, peer: PeerId) -> usize {
        self.conns.iter().find(|c| c.id == peer).map_or(0, |c| c.inbox.len())
    }

    /// Push every outbox as far as the kernel will allow. Never blocks.
    ///
    /// [`Transport::poll`] does this first, so a caller with the usual
    /// send-then-drain loop never needs it. Call it before dropping a
    /// transport if the last thing sent matters — a dropped socket does
    /// not deliver what the kernel never took.
    pub fn flush(&mut self) -> Result<(), TransportError> {
        let mut first = None;
        for conn in &mut self.conns {
            if conn.dead {
                continue;
            }
            if let Err(e) = conn.flush() {
                self.faults.push((Some(conn.id), e.clone()));
                first.get_or_insert(e);
            }
        }
        match first {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    /// Close one connection deliberately — a kick, or a session the
    /// caller has decided is over.
    ///
    /// Not reported by [`take_disconnected`](TcpTransport::take_disconnected):
    /// the caller already knows. Anything the peer sent and we have not
    /// polled is discarded, which is the difference between hanging up
    /// and being hung up on.
    pub fn disconnect(&mut self, peer: PeerId) {
        if let Some(index) = self.conns.iter().position(|c| c.id == peer) {
            let conn = self.conns.remove(index);
            let _ = conn.stream.shutdown(std::net::Shutdown::Both);
            self.cursor = 0;
        }
    }

    /// Drop connections that are dead and fully drained, announcing
    /// each one exactly once.
    fn reap(&mut self) {
        let mut departed = Vec::new();
        self.conns.retain(|c| {
            let finished = c.dead && c.inbox.is_empty();
            if finished {
                departed.push(c.id);
            }
            !finished
        });
        if !departed.is_empty() {
            self.gone.extend(departed);
            self.cursor = 0;
        }
    }
}

impl Transport for TcpTransport {
    /// Frame and queue one message.
    ///
    /// Never blocks and never sends part of a message: either the whole
    /// thing is ours to deliver or the call fails. When the kernel's
    /// send buffer is full the remainder waits in the outbox and goes
    /// out on the next [`poll`](Transport::poll) or
    /// [`flush`](TcpTransport::flush) — a partial `write` on a
    /// non-blocking socket is ordinary, not an error, and a transport
    /// that treated it as one would lose a message on the first busy
    /// link it met.
    fn send(&mut self, peer: PeerId, bytes: &[u8]) -> Result<(), TransportError> {
        let framed = frame(bytes)?;
        let Some(conn) = self.conns.iter_mut().find(|c| c.id == peer) else {
            return Err(TransportError::NoSuchPeer(peer));
        };
        if conn.dead {
            return Err(TransportError::Disconnected(peer));
        }
        if conn.pending_out() + framed.len() > MAX_OUTBOX {
            conn.dead = true;
            let e = TransportError::Io(format!(
                "{peer} has not read {MAX_OUTBOX} bytes of backlog; treating it as gone"
            ));
            self.faults.push((Some(peer), e.clone()));
            return Err(e);
        }
        conn.out.extend_from_slice(&framed);
        match conn.flush() {
            Ok(()) => Ok(()),
            Err(e) => {
                self.faults.push((Some(peer), e.clone()));
                Err(e)
            }
        }
    }

    /// The next complete message from any peer, or `None`.
    ///
    /// Accepts new connections, pushes pending writes, reads what has
    /// arrived, and returns one message. Never blocks — which is the
    /// trait's promise and the reason there is no reader thread.
    ///
    /// Call it in a loop until `None`, at **one fixed point per tick**.
    /// Not mid-tick: how much has arrived by a given instant is the
    /// scheduler-dependent value D-5 forbids from reaching `step()`.
    fn poll(&mut self) -> Option<(PeerId, Vec<u8>)> {
        if let Err(e) = self.accept_pending() {
            self.faults.push((None, e));
        }
        let _ = self.flush();

        let mut scratch = std::mem::take(&mut self.scratch);
        for index in 0..self.conns.len() {
            let conn = &mut self.conns[index];
            if conn.dead || !conn.inbox.is_empty() {
                continue;
            }
            if let Err(e) = conn.fill(&mut scratch) {
                let id = conn.id;
                self.faults.push((Some(id), e));
            }
        }
        self.scratch = scratch;

        let count = self.conns.len();
        for step in 0..count {
            let index = (self.cursor + step) % count;
            if let Some(message) = self.conns[index].inbox.pop_front() {
                self.cursor = (index + 1) % count;
                return Some((self.conns[index].id, message));
            }
        }

        // Nothing to deliver: now is the moment a departed peer has
        // definitely said everything it was going to say.
        self.reap();
        None
    }

    /// Everyone connected, in id order, dead connections excluded.
    ///
    /// Excluded the moment the link fails rather than when it is
    /// reaped, so a caller that broadcasts from this list never
    /// addresses a socket that is already gone. Errata 7 is the whole
    /// argument for this method existing: the transport knows, and a
    /// caller keeping its own list is a caller whose list drifts.
    fn peers(&self) -> Vec<PeerId> {
        self.conns.iter().filter(|c| !c.dead).map(|c| c.id).collect()
    }

    /// Send to every peer, attempting all of them even if one fails.
    ///
    /// **This overrides the trait's default deliberately, and the
    /// default is worth a warning.** [`Transport::broadcast`]'s
    /// provided body uses `?`, so the first failing peer aborts the
    /// loop and everybody after it in the list is silently skipped. On
    /// the [`Loopback`](crate::Loopback) that cannot happen — a
    /// partitioned peer returns `Ok` — but on a real socket it happens
    /// on the first mid-turn quit: the host broadcasts a turn, player 2
    /// has just dropped, and players 3, 4 and 5 never receive it. That
    /// is exactly the "one player silently stops receiving turns"
    /// failure errata 7 added [`peers`](Transport::peers) to prevent,
    /// arriving through the other door.
    ///
    /// Here every live peer is attempted and the first error is
    /// returned afterwards, so a caller that ignores the error still
    /// gets a correct broadcast and one that handles it still learns.
    fn broadcast(&mut self, bytes: &[u8]) -> Result<(), TransportError> {
        let mut first = None;
        for peer in self.peers() {
            if let Err(e) = self.send(peer, bytes) {
                first.get_or_insert(e);
            }
        }
        match first {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }
}
