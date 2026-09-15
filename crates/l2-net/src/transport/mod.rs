
mod frame_part;
pub use frame_part::*;
mod loopback;
pub use loopback::*;

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PeerId(pub u32);

impl core::fmt::Display for PeerId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "peer {}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportError {
    NoSuchPeer(PeerId),
    Disconnected(PeerId),
    FrameTooLong { len: usize },
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

pub trait Transport {
    fn send(&mut self, peer: PeerId, bytes: &[u8]) -> Result<(), TransportError>;

    fn poll(&mut self) -> Option<(PeerId, Vec<u8>)>;

    fn peers(&self) -> Vec<PeerId>;

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

