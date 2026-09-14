#![allow(unused_imports)]
use super::*;
use super::transport::*;
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
    pub(super) fn new(id: PeerId, stream: TcpStream) -> Result<Connection, TransportError> {
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

