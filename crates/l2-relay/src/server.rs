//! The relay's listener and its table of live codes (C254). One thread per
//! connection; the table behind one mutex.

use std::collections::HashMap;
use std::io;
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::protocol::{CODE_LEN, MAX_CODES, MAX_WAITING, TOKEN_LEN};
use crate::rand;

/// The timings of C254: ping idle hosts every 20 s, drop one silent for 90 s,
/// give a host 10 s to ACCEPT, and a new connection 10 s to say its line.
#[derive(Debug, Clone)]
pub struct Config {
    pub ping: Duration,
    pub silence: Duration,
    pub answer: Duration,
    pub handshake: Duration,
    pub log: bool,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            ping: Duration::from_secs(20),
            silence: Duration::from_secs(90),
            answer: Duration::from_secs(10),
            handshake: Duration::from_secs(10),
            log: true,
        }
    }
}

pub(crate) type Shared = Arc<Mutex<State>>;

pub(crate) struct HostEntry {
    pub ctl: Arc<Mutex<TcpStream>>,
    pub waiting: usize,
}

pub(crate) struct Pending {
    pub code: String,
    pub hand: Sender<TcpStream>,
}

#[derive(Default)]
pub(crate) struct State {
    codes: HashMap<String, HostEntry>,
    tokens: HashMap<String, Pending>,
}

impl State {
    /// Unique among the live codes; `None` at [`MAX_CODES`] (C254: 256).
    pub fn issue_code(&mut self, ctl: Arc<Mutex<TcpStream>>) -> Option<String> {
        if self.codes.len() >= MAX_CODES {
            return None;
        }
        let code = loop {
            let c = rand::symbols(CODE_LEN);
            if !self.codes.contains_key(&c) {
                break c;
            }
        };
        self.codes.insert(code.clone(), HostEntry { ctl, waiting: 0 });
        Some(code)
    }

    /// C254: the code lives exactly as long as the control connection.
    pub fn retire_code(&mut self, code: &str) {
        self.codes.remove(code);
        self.tokens.retain(|_, p| p.code != code);
    }

    /// At most [`MAX_WAITING`] joiners per code (C254: 4, the game seats 5).
    pub fn enlist(&mut self, code: &str) -> Result<Enlisted, &'static str> {
        let host = self.codes.get_mut(code).ok_or("unknown code")?;
        if host.waiting >= MAX_WAITING {
            return Err("too many waiting");
        }
        host.waiting += 1;
        let ctl = host.ctl.clone();
        let token = loop {
            let t = rand::symbols(TOKEN_LEN);
            if !self.tokens.contains_key(&t) {
                break t;
            }
        };
        let (hand, wait) = mpsc::channel();
        self.tokens.insert(token.clone(), Pending { code: code.to_string(), hand });
        Ok(Enlisted { token, ctl, wait })
    }

    pub fn claim(&mut self, token: &str) -> Option<Sender<TcpStream>> {
        let pending = self.tokens.remove(token)?;
        if let Some(host) = self.codes.get_mut(&pending.code) {
            host.waiting = host.waiting.saturating_sub(1);
        }
        Some(pending.hand)
    }

    pub fn live_codes(&self) -> usize {
        self.codes.len()
    }
}

pub(crate) struct Enlisted {
    pub token: String,
    pub ctl: Arc<Mutex<TcpStream>>,
    pub wait: Receiver<TcpStream>,
}

pub struct Relay {
    listener: TcpListener,
    config: Config,
    state: Shared,
}

impl Relay {
    pub fn bind(addr: impl ToSocketAddrs) -> io::Result<Relay> {
        let listener = TcpListener::bind(addr)?;
        Ok(Relay { listener, config: Config::default(), state: Shared::default() })
    }

    pub fn with_config(mut self, config: Config) -> Relay {
        self.config = config;
        self
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.listener.local_addr()
    }

    pub fn live_codes(&self) -> usize {
        self.state.lock().map(|s| s.live_codes()).unwrap_or(0)
    }

    pub fn counter(&self) -> impl Fn() -> usize {
        let state = self.state.clone();
        move || state.lock().map(|s| s.live_codes()).unwrap_or(0)
    }

    pub fn serve(self) -> io::Result<()> {
        if self.config.log {
            println!("listening on {}", self.listener.local_addr()?);
        }
        loop {
            let (stream, peer) = match self.listener.accept() {
                Ok(accepted) => accepted,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::ConnectionAborted | io::ErrorKind::ConnectionReset
                    ) =>
                {
                    continue
                }
                Err(e) => return Err(e),
            };
            let (state, config) = (self.state.clone(), self.config.clone());
            std::thread::spawn(move || crate::session::run(stream, peer, state, config));
        }
    }

    pub fn spawn(self) -> JoinHandle<io::Result<()>> {
        std::thread::spawn(move || self.serve())
    }
}

pub(crate) fn log(config: &Config, event: &str) {
    if config.log {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        println!("{now} {event}");
    }
}
