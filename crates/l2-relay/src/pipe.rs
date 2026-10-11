//! Bytes both ways between a paired joiner and host (C254): one 64 KiB buffer
//! per direction, `write_all` before the next read, so nothing more is held.

use std::io::{ErrorKind, Read, Write};
use std::net::{Shutdown, TcpStream};

use crate::protocol::{write_line, Answer};

const BUFFER: usize = 64 * 1024;

/// `OK` to both, then copy until either side closes (C254).
pub(crate) fn pair(mut a: TcpStream, mut b: TcpStream) {
    for s in [&mut a, &mut b] {
        let _ = s.set_read_timeout(None);
        let _ = s.set_write_timeout(None);
    }
    let ok = Answer::Ok.line();
    if write_line(&mut a, &ok).is_err() || write_line(&mut b, &ok).is_err() {
        return;
    }
    let (Ok(a2), Ok(b2)) = (a.try_clone(), b.try_clone()) else { return };
    let back = std::thread::spawn(move || copy(b2, a2));
    copy(a, b);
    let _ = back.join();
}

fn copy(mut from: TcpStream, mut to: TcpStream) {
    let mut buffer = vec![0u8; BUFFER];
    loop {
        let read = match from.read(&mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(n) => n,
        };
        if let Err(e) = to.write_all(&buffer[..read]) {
            if e.kind() != ErrorKind::Interrupted {
                break;
            }
        }
    }
    let _ = from.shutdown(Shutdown::Both);
    let _ = to.shutdown(Shutdown::Both);
}
