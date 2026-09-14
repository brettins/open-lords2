#![allow(unused_imports)]
use super::*;
use super::frame_part::*;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

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
/// differently
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
    /// Latency in polls, because a clock has
    /// no place in a deterministic test (D-5) and because "how many
    /// times did the loop go round before this arrived" is the quantity
    /// the session cares about.
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
    /// ordered delivery as a *requirement*,
    /// and the reason lockstep has no use for an unreliable channel.
    ///
    /// Use [`Loopback::hold`] to model the case a reliable transport
    /// produces.
    pub fn partition(&self, peer: PeerId, dropped: bool) {
        self.with_box(peer, |b| b.partitioned = dropped);
    }

    /// Stop delivering to `peer`, but **keep** what is sent — a stalled
    /// connection.
    ///
    /// This is what a peer going quiet looks like over TCP: nothing is
    /// lost, everything is late
    /// backlog arrives in order. It is the case §4's stalling rule is
    /// written for
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
            // Silently dropped. The
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

