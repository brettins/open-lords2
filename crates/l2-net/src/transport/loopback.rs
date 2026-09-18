#![allow(unused_imports)]
use super::*;
use super::frame_part::*;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};


#[derive(Debug)]
struct Queued {
    from: PeerId,
    bytes: Vec<u8>,
    ready_at: u64,
}

#[derive(Debug, Default)]
struct Mailbox {
    queue: VecDeque<Queued>,
    polls: u64,
    latency: u64,
    reorder: bool,
    partitioned: bool,
    held: Vec<Queued>,
    holding: bool,
}

#[derive(Debug, Default)]
struct Switch {
    boxes: Vec<(PeerId, Mailbox)>,
}

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
            switch.boxes.sort_by_key(|(id, _)| *id);
        }
    }

    pub fn endpoint(&self, me: PeerId) -> Endpoint {
        self.add_peer(me);
        Endpoint { me, net: self.clone() }
    }

    pub fn set_latency(&self, peer: PeerId, polls: u64) {
        self.with_box(peer, |b| b.latency = polls);
    }

    pub fn set_reorder(&self, peer: PeerId, reorder: bool) {
        self.with_box(peer, |b| b.reorder = reorder);
    }

    pub fn partition(&self, peer: PeerId, dropped: bool) {
        self.with_box(peer, |b| b.partitioned = dropped);
    }

    pub fn hold(&self, peer: PeerId, holding: bool) {
        self.with_box(peer, |mailbox| {
            mailbox.holding = holding;
            if !holding {
                let now = mailbox.polls;
                for mut queued in std::mem::take(&mut mailbox.held) {
                    queued.ready_at = now + mailbox.latency;
                    mailbox.queue.push_back(queued);
                }
            }
        });
    }

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
        self.switch.lock().unwrap_or_else(|e| e.into_inner())
    }
}

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

