#![allow(unused_imports)]
use super::*;

use std::collections::VecDeque;
use std::io::{ErrorKind, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::time::Duration;
use crate::transport::{frame, FrameReader, PeerId, Transport, TransportError};

/// One connection, and everything half-delivered in either direction.
#[derive(Debug)]
pub(super) struct Connection {
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
/// patience this code reads. It matters in
    /// practice: connecting to a host that is up but not listening —
    /// the wrong port typed into a dialog, or a firewall dropping
    /// — blocks for around twenty seconds on
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
/// in this game" screen.
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
/// Reported because they happen inside
    /// [`Transport::poll`], whose signature is `Option` and not
    /// `Result` — a drain loop must not have to distinguish "nothing
    /// arrived" from "something broke" on every iteration. Pair it with
    /// [`take_disconnected`](TcpTransport::take_disconnected): every
    /// per-peer fault here also produces a departure there.
    ///
    /// The `Option` is `None` for a failure of the *listener*, which
/// belongs to no peer. It is an `Option`
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
    /// trait's promise.
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
/// Excluded the moment the link fails
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

