#![allow(unused_imports)]
use super::*;
use super::connection::*;
use super::*;
use std::collections::VecDeque;
use std::io::{ErrorKind, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::time::Duration;
use crate::transport::{frame, FrameReader, PeerId, Transport, TransportError};

impl TcpTransport {
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

    pub fn connect(
        addr: impl ToSocketAddrs,
        id: PeerId,
    ) -> Result<TcpTransport, TransportError> {
        let stream = TcpStream::connect(addr).map_err(|e| io(&e))?;
        TcpTransport::adopt(stream, id)
    }

    pub fn connect_timeout(
        addr: SocketAddr,
        id: PeerId,
        timeout: Duration,
    ) -> Result<TcpTransport, TransportError> {
        let stream = TcpStream::connect_timeout(&addr, timeout).map_err(|e| io(&e))?;
        TcpTransport::adopt(stream, id)
    }

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

    pub fn local_addr(&self) -> Option<SocketAddr> {
        self.listener.as_ref().and_then(|l| l.local_addr().ok())
    }

    pub fn peer_addr(&self, peer: PeerId) -> Option<SocketAddr> {
        self.conns.iter().find(|c| c.id == peer)?.stream.peer_addr().ok()
    }

    pub fn nodelay(&self, peer: PeerId) -> Option<bool> {
        self.conns.iter().find(|c| c.id == peer)?.stream.nodelay().ok()
    }

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

    pub fn take_joined(&mut self) -> Vec<PeerId> {
        std::mem::take(&mut self.joined)
    }

    pub fn take_disconnected(&mut self) -> Vec<PeerId> {
        std::mem::take(&mut self.gone)
    }

    pub fn take_faults(&mut self) -> Vec<(Option<PeerId>, TransportError)> {
        std::mem::take(&mut self.faults)
    }

    pub fn pending_out(&self, peer: PeerId) -> usize {
        self.conns.iter().find(|c| c.id == peer).map_or(0, |c| c.pending_out())
    }

    pub fn pending_in(&self, peer: PeerId) -> usize {
        self.conns.iter().find(|c| c.id == peer).map_or(0, |c| c.inbox.len())
    }

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

    pub fn disconnect(&mut self, peer: PeerId) {
        if let Some(index) = self.conns.iter().position(|c| c.id == peer) {
            let conn = self.conns.remove(index);
            let _ = conn.stream.shutdown(std::net::Shutdown::Both);
            self.cursor = 0;
        }
    }

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

        self.reap();
        None
    }

    fn peers(&self) -> Vec<PeerId> {
        self.conns.iter().filter(|c| !c.dead).map(|c| c.id).collect()
    }

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


