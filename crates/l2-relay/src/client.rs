//! The game's side of protocol v1 (C254): blocking calls, each meant for a
//! thread of its own; the player's words for a [`Fault`] belong to the game.

use std::fmt;
use std::io;
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use crate::protocol::{parse_answer, read_line, write_line, Answer, Request};

#[derive(Debug)]
pub enum Fault {
    Unreachable(io::Error),
    Refused(String),
    Protocol(String),
}

impl fmt::Display for Fault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Fault::Unreachable(e) => write!(f, "unreachable: {e}"),
            Fault::Refused(why) => write!(f, "refused: {why}"),
            Fault::Protocol(line) => write!(f, "not a relay: {line:?}"),
        }
    }
}

impl From<io::Error> for Fault {
    fn from(e: io::Error) -> Fault {
        Fault::Unreachable(e)
    }
}

pub fn open(relay: &str, timeout: Duration) -> io::Result<TcpStream> {
    let bad = || io::Error::new(io::ErrorKind::InvalidInput, format!("{relay:?} is not host:port"));
    let to = relay.to_socket_addrs().map_err(|_| bad())?.next().ok_or_else(bad)?;
    let stream = TcpStream::connect_timeout(&to, timeout)?;
    stream.set_nodelay(true)?;
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;
    Ok(stream)
}

fn ask(stream: &mut TcpStream, request: &Request) -> Result<Answer, Fault> {
    write_line(stream, &request.line())?;
    let line = read_line(stream)?;
    parse_answer(&line).ok_or(Fault::Protocol(line))
}

fn paired(mut stream: TcpStream, request: &Request, wait: Duration) -> Result<TcpStream, Fault> {
    stream.set_read_timeout(Some(wait))?;
    match ask(&mut stream, request)? {
        Answer::Ok => {
            stream.set_read_timeout(None)?;
            stream.set_write_timeout(None)?;
            Ok(stream)
        }
        Answer::No(why) => Err(Fault::Refused(why)),
        other => Err(Fault::Protocol(other.line())),
    }
}

/// `HOST 1` → `CODE <code>` (C254).
pub fn host(relay: &str, timeout: Duration) -> Result<(TcpStream, String), Fault> {
    let mut stream = open(relay, timeout)?;
    match ask(&mut stream, &Request::Host)? {
        Answer::Code(code) => Ok((stream, code)),
        Answer::No(why) => Err(Fault::Refused(why)),
        other => Err(Fault::Protocol(other.line())),
    }
}

/// `JOIN 1 <code>` → `OK` once the host accepted, within `wait` (C254).
pub fn join(relay: &str, code: &str, timeout: Duration, wait: Duration) -> Result<TcpStream, Fault> {
    let stream = open(relay, timeout)?;
    paired(stream, &Request::Join(code.into()), wait)
}

/// `ACCEPT 1 <token>` → `OK` (C254).
pub fn accept(relay: &str, token: &str, timeout: Duration) -> Result<TcpStream, Fault> {
    let stream = open(relay, timeout)?;
    paired(stream, &Request::Accept(token.into()), timeout)
}
