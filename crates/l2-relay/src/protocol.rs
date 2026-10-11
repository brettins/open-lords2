//! The handshake lines of protocol v1 (C254). ASCII, `\n`-terminated, at most
//! [`LINE_MAX`] bytes; after `OK` the connection carries the game's own bytes.

use std::io::{self, ErrorKind, Read, Write};

/// 32 symbols, 5 bits each: no `0`/`O`, no `1`/`I` (C254).
pub const ALPHABET: &[u8; 32] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
pub const CODE_LEN: usize = 6;
pub const TOKEN_LEN: usize = 8;
pub const LINE_MAX: usize = 64;
pub const VERSION: u32 = 1;
/// At most this many live codes (C254).
pub const MAX_CODES: usize = 256;
/// At most this many joiners waiting on one code: the game seats 5 (C254).
pub const MAX_WAITING: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    Host,
    Join(String),
    Accept(String),
    Ping,
    Pong,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    Ok,
    Code(String),
    Join(String),
    No(String),
    Ping,
    Pong,
}

fn in_alphabet(text: &str, len: usize) -> bool {
    text.len() == len && text.bytes().all(|b| ALPHABET.contains(&b))
}

/// `text` trimmed and upper-cased when it is [`CODE_LEN`] symbols of
/// [`ALPHABET`]; an address never is, having a `.` or a `:` (C254).
pub fn as_code(text: &str) -> Option<String> {
    let code = text.trim().to_ascii_uppercase();
    in_alphabet(&code, CODE_LEN).then_some(code)
}

pub fn is_token(text: &str) -> bool {
    in_alphabet(text, TOKEN_LEN)
}

pub fn parse_request(line: &str) -> Option<Request> {
    let mut words = line.split(' ');
    let verb = words.next()?;
    let version_ok = |v: Option<&str>| v.and_then(|v| v.parse::<u32>().ok()) == Some(VERSION);
    let request = match verb {
        "PING" => Request::Ping,
        "PONG" => Request::Pong,
        "HOST" if version_ok(words.next()) => Request::Host,
        "JOIN" if version_ok(words.next()) => Request::Join(as_code(words.next()?)?),
        "ACCEPT" if version_ok(words.next()) => {
            let token = words.next()?;
            if !is_token(token) {
                return None;
            }
            Request::Accept(token.to_string())
        }
        _ => return None,
    };
    words.next().is_none().then_some(request)
}

pub fn parse_answer(line: &str) -> Option<Answer> {
    let (verb, rest) = line.split_once(' ').unwrap_or((line, ""));
    Some(match verb {
        "OK" if rest.is_empty() => Answer::Ok,
        "PING" if rest.is_empty() => Answer::Ping,
        "PONG" if rest.is_empty() => Answer::Pong,
        "CODE" => Answer::Code(as_code(rest)?),
        "JOIN" if is_token(rest) => Answer::Join(rest.to_string()),
        "NO" => Answer::No(rest.to_string()),
        _ => return None,
    })
}

impl Answer {
    pub fn line(&self) -> String {
        match self {
            Answer::Ok => "OK".into(),
            Answer::Code(c) => format!("CODE {c}"),
            Answer::Join(t) => format!("JOIN {t}"),
            Answer::No(why) => format!("NO {why}"),
            Answer::Ping => "PING".into(),
            Answer::Pong => "PONG".into(),
        }
    }
}

impl Request {
    pub fn line(&self) -> String {
        match self {
            Request::Host => format!("HOST {VERSION}"),
            Request::Join(c) => format!("JOIN {VERSION} {c}"),
            Request::Accept(t) => format!("ACCEPT {VERSION} {t}"),
            Request::Ping => "PING".into(),
            Request::Pong => "PONG".into(),
        }
    }
}

/// One byte at a time: the bytes after `OK`'s `\n` are the game's (C254).
/// Over [`LINE_MAX`] without a `\n` is `InvalidData` (C254: a 64-byte line limit).
pub fn read_line(from: &mut impl Read) -> io::Result<String> {
    let mut line = Vec::with_capacity(LINE_MAX);
    let mut byte = [0u8; 1];
    loop {
        match from.read(&mut byte) {
            Ok(0) => return Err(ErrorKind::UnexpectedEof.into()),
            Ok(_) if byte[0] == b'\n' => break,
            Ok(_) => line.push(byte[0]),
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
        if line.len() >= LINE_MAX {
            return Err(io::Error::new(ErrorKind::InvalidData, "line over 64 bytes"));
        }
    }
    if line.last() == Some(&b'\r') {
        line.pop();
    }
    String::from_utf8(line).map_err(|_| io::Error::new(ErrorKind::InvalidData, "not ASCII"))
}

pub fn write_line(to: &mut impl Write, line: &str) -> io::Result<()> {
    let mut bytes = line.as_bytes().to_vec();
    bytes.push(b'\n');
    to.write_all(&bytes)?;
    to.flush()
}

/// Whole lines out of a non-blocking stream's bytes; `None` past [`LINE_MAX`] (C254).
pub fn split_lines(buffer: &mut Vec<u8>) -> Option<Vec<String>> {
    let mut lines = Vec::new();
    while let Some(end) = buffer.iter().position(|&b| b == b'\n') {
        if end >= LINE_MAX {
            return None;
        }
        let mut line: Vec<u8> = buffer.drain(..=end).collect();
        line.pop();
        if line.last() == Some(&b'\r') {
            line.pop();
        }
        lines.push(String::from_utf8_lossy(&line).into_owned());
    }
    (buffer.len() < LINE_MAX).then_some(lines)
}
