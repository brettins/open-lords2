
mod transport;
use transport::*;

use std::io::ErrorKind;
use std::net::TcpListener;

use crate::transport::{PeerId, TransportError};

const READ_BUDGET: usize = 256 * 1024;

const READ_CHUNK: usize = 64 * 1024;

pub const MAX_OUTBOX: usize = 4 * 1024 * 1024;

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

#[derive(Debug)]
pub struct TcpTransport {
    listener: Option<TcpListener>,
    conns: Vec<Connection>,
    next_id: u32,
    cursor: usize,
    joined: Vec<PeerId>,
    gone: Vec<PeerId>,
    faults: Vec<(Option<PeerId>, TransportError)>,
    scratch: Vec<u8>,
}

