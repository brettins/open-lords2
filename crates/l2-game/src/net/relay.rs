//! The game's end of the join-code relay (`docs/netcode.md` §7 *Join codes
//! and the relay*, C254). Every socket call here runs on a thread or without
//! blocking: the `Driver` is pumped between frames (D-5).

use l2_relay::client::{self, Fault};
use l2_relay::protocol::{parse_answer, split_lines, write_line, Answer, Request};
use std::io::{ErrorKind, Read};
use std::net::TcpStream;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::OnceLock;
use std::time::Duration;

/// Empty until a relay is deployed; the lead sets it once (C254).
/// `open-lords2-relay` on fly.io (yyz), its dedicated IPv4: `tools/relay/README.md`.
pub const DEFAULT_RELAY: &str = "37.16.14.84:4714";
pub const RELAY_VAR: &str = "LORDS2_RELAY";
const CONNECT: Duration = Duration::from_secs(5);
/// A joiner waits this long for the host's ACCEPT (C254: the relay's own limit is 10 s).
pub const JOIN_WAIT: Duration = Duration::from_secs(12);
pub const NOT_CONFIGURED: &str =
    "No join-code service is configured. Type the host's address instead.";

static FLAG: OnceLock<String> = OnceLock::new();

/// `--relay host:port` (C254).
pub fn configure(address: &str) {
    let _ = FLAG.set(address.trim().to_string());
}

/// `--relay`, then `LORDS2_RELAY`, then [`DEFAULT_RELAY`]; empty means none (C254).
pub fn configured() -> Option<String> {
    let flag = FLAG.get().cloned();
    let env = std::env::var(RELAY_VAR).ok();
    [flag, env, Some(DEFAULT_RELAY.to_string())]
        .into_iter()
        .flatten()
        .map(|s| s.trim().to_string())
        .find(|s| !s.is_empty())
}

/// The joiner's thread: `JOIN 1 <code>` and the wait for `OK` (C254).
pub fn join(relay: &str, code: &str) -> Result<TcpStream, String> {
    client::join(relay, code, CONNECT, JOIN_WAIT).map_err(|fault| match fault {
        Fault::Refused(why) if why == "unknown code" => {
            format!("No game has the code {code}. Check it with the host.")
        }
        Fault::Refused(why) if why == "host did not answer" => format!(
            "The host of {code} did not answer in {} seconds. Ask them to try again.",
            JOIN_WAIT.as_secs()
        ),
        Fault::Refused(why) if why == "too many waiting" => {
            format!("Too many are joining {code} at once. Try again in a moment.")
        }
        Fault::Refused(why) if why == "host left" => {
            format!("The host of {code} left before you were in.")
        }
        Fault::Refused(why) => format!("The join-code service refused: {why}."),
        Fault::Unreachable(e) => format!(
            "The join-code service at {relay} did not answer ({e}). Type the host's address instead."
        ),
        Fault::Protocol(_) => format!("{relay} is not a join-code service."),
    })
}

fn host_words(relay: &str, fault: &Fault) -> String {
    const STILL: &str = "Others can still join by address.";
    match fault {
        Fault::Unreachable(_) => format!("The join-code service at {relay} did not answer. {STILL}"),
        Fault::Refused(why) => format!("The join-code service at {relay} refused: {why}. {STILL}"),
        Fault::Protocol(_) => format!("{relay} is not a join-code service. {STILL}"),
    }
}

/// A host's control connection (C254), opened on a thread, read without blocking.
pub struct Host {
    relay: String,
    opening: Option<Receiver<Result<(TcpStream, String), Fault>>>,
    control: Option<TcpStream>,
    buffer: Vec<u8>,
    code: Option<String>,
    status: Option<String>,
    accepting: Vec<Receiver<Result<TcpStream, Fault>>>,
}

impl Host {
    pub fn open(relay: String) -> Host {
        let (tx, rx) = mpsc::channel();
        let at = relay.clone();
        std::thread::spawn(move || {
            let _ = tx.send(client::host(&at, CONNECT));
        });
        Host {
            relay,
            opening: Some(rx),
            control: None,
            buffer: Vec::new(),
            code: None,
            status: None,
            accepting: Vec::new(),
        }
    }

    pub fn code(&self) -> Option<&str> {
        self.code.as_deref()
    }

    pub fn status(&self) -> Option<&str> {
        self.status.as_deref()
    }

    fn lost(&mut self, why: String) {
        self.control = None;
        self.code = None;
        self.status = Some(why);
    }

    /// One non-blocking pass (D-5): the streams the relay paired, for `add` (C254).
    pub fn pump(&mut self) -> Vec<TcpStream> {
        self.collect_opening();
        self.service_control();
        let mut paired = Vec::new();
        self.accepting.retain(|rx| match rx.try_recv() {
            Err(TryRecvError::Empty) => true,
            Ok(Ok(stream)) => {
                paired.push(stream);
                false
            }
            Ok(Err(_)) | Err(TryRecvError::Disconnected) => false,
        });
        paired
    }

    fn collect_opening(&mut self) {
        let Some(rx) = &self.opening else { return };
        let result = match rx.try_recv() {
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => {
                Err(Fault::Unreachable(ErrorKind::ConnectionAborted.into()))
            }
            Ok(r) => r,
        };
        self.opening = None;
        match result {
            Ok((stream, code)) => match stream.set_nonblocking(true) {
                Ok(()) => {
                    let _ = stream.set_read_timeout(None);
                    self.control = Some(stream);
                    self.code = Some(code);
                    self.status = None;
                }
                Err(e) => self.status = Some(host_words(&self.relay, &Fault::Unreachable(e))),
            },
            Err(fault) => self.status = Some(host_words(&self.relay, &fault)),
        }
    }

    fn service_control(&mut self) {
        let Some(control) = self.control.as_mut() else { return };
        let mut chunk = [0u8; 256];
        loop {
            match control.read(&mut chunk) {
                Ok(0) => return self.closed(),
                Ok(n) => self.buffer.extend_from_slice(&chunk[..n]),
                Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(_) => return self.closed(),
            }
        }
        let Some(lines) = split_lines(&mut self.buffer) else { return self.closed() };
        for line in lines {
            match parse_answer(&line) {
                Some(Answer::Ping) => {
                    let Some(control) = self.control.as_mut() else { return };
                    if write_line(control, &Request::Pong.line()).is_err() {
                        return self.closed();
                    }
                }
                Some(Answer::Join(token)) => self.accept(token),
                _ => {}
            }
        }
    }

    fn closed(&mut self) {
        let why = format!(
            "The join-code service at {} closed the connection. Others can still join by address.",
            self.relay
        );
        self.lost(why);
    }

    /// `ACCEPT 1 <token>` on a thread (C254); `pump` collects the `OK`.
    fn accept(&mut self, token: String) {
        let (tx, rx) = mpsc::channel();
        let relay = self.relay.clone();
        std::thread::spawn(move || {
            let _ = tx.send(client::accept(&relay, &token, CONNECT));
        });
        self.accepting.push(rx);
    }
}
