#![allow(unused_imports)]
use super::*;
use super::loopback::*;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

pub const MAX_FRAME: usize = 1 << 20;

pub fn frame(payload: &[u8]) -> Result<Vec<u8>, TransportError> {
    if payload.len() > MAX_FRAME {
        return Err(TransportError::FrameTooLong { len: payload.len() });
    }
    let mut out = Vec::with_capacity(payload.len() + 4);
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.extend_from_slice(payload);
    Ok(out)
}

#[derive(Debug, Default)]
pub struct FrameReader {
    buf: Vec<u8>,
    consumed: usize,
}

impl FrameReader {
    pub fn new() -> FrameReader {
        FrameReader::default()
    }

    pub fn feed(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
    }

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

