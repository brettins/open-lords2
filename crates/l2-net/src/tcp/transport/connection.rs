#![allow(unused_imports)]
use super::*;
use super::transport::*;
use super::*;
use std::collections::VecDeque;
use std::io::{ErrorKind, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::time::Duration;
use crate::transport::{frame, FrameReader, PeerId, Transport, TransportError};

#[derive(Debug)]
pub(crate) struct Connection {
    pub(crate) id: PeerId,
    pub(crate) stream: TcpStream,
    reader: FrameReader,
    pub(crate) inbox: VecDeque<Vec<u8>>,
    pub(super) out: Vec<u8>,
    out_done: usize,
    pub(crate) dead: bool,
}

impl Connection {
    pub(super) fn new(id: PeerId, stream: TcpStream) -> Result<Connection, TransportError> {
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

    pub(crate) fn pending_out(&self) -> usize {
        self.out.len() - self.out_done
    }

    pub(crate) fn flush(&mut self) -> Result<(), TransportError> {
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

    pub(super) fn fill(&mut self, scratch: &mut [u8]) -> Result<(), TransportError> {
        let mut budget = READ_BUDGET;
        let mut failure = None;
        while budget > 0 {
            match self.stream.read(scratch) {
                Ok(0) => {
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
                    self.dead = true;
                    self.reader = FrameReader::new();
                    failure = Some(e);
                    break;
                }
            }
        }
        if self.dead && failure.is_none() && self.reader.buffered() > 0 {
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

