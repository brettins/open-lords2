#![allow(unused_imports)]
use super::*;
use super::loopback::*;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

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
/// Framing lives above [`Transport`]
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

