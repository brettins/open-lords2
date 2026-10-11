//! One connection's life (C254): its first line decides whether it is a host's
//! control connection, a joiner, or a host's ACCEPT.

use std::io::{ErrorKind, Read};
use std::net::{SocketAddr, TcpStream};
use std::sync::mpsc::RecvTimeoutError;
use std::time::{Duration, Instant};

use crate::protocol::{parse_request, read_line, write_line, Answer, Request, LINE_MAX};
use crate::server::{log, Config, Shared};

pub(crate) fn run(mut stream: TcpStream, peer: SocketAddr, shared: Shared, config: Config) {
    let _ = stream.set_nodelay(true);
    if stream.set_read_timeout(Some(config.handshake)).is_err() {
        return;
    }
    let Ok(line) = read_line(&mut stream) else { return };
    match parse_request(&line) {
        Some(Request::Host) => host(stream, peer, shared, config),
        Some(Request::Join(code)) => join(stream, code, shared, config),
        Some(Request::Accept(token)) => accept(stream, token, shared),
        _ => {}
    }
}

fn refuse(stream: &mut TcpStream, why: &str) {
    let _ = write_line(stream, &Answer::No(why.into()).line());
}

/// C254: PING is answered PONG, an idle host is pinged every `config.ping`
/// (20 s), a silent one dropped after `config.silence` (90 s), and the code
/// lives exactly as long as this connection.
fn host(mut stream: TcpStream, peer: SocketAddr, shared: Shared, config: Config) {
    let Ok(writer) = stream.try_clone() else { return };
    let ctl = std::sync::Arc::new(std::sync::Mutex::new(writer));
    let issued = shared.lock().ok().and_then(|mut s| s.issue_code(ctl.clone()));
    let Some(code) = issued else {
        refuse(&mut stream, "too many games");
        return;
    };
    let said = ctl.lock().map(|mut w| write_line(&mut *w, &Answer::Code(code.clone()).line()));
    if !matches!(said, Ok(Ok(()))) {
        retire(&shared, &code);
        return;
    }
    log(&config, &format!("code issued {code} to {}", peer.ip()));
    let tick = (config.ping / 2).clamp(Duration::from_millis(10), Duration::from_secs(1));
    let _ = stream.set_read_timeout(Some(tick));
    let (mut heard, mut sent) = (Instant::now(), Instant::now());
    let mut line = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        match stream.read(&mut byte) {
            Ok(0) => break,
            Ok(_) => {
                heard = Instant::now();
                if byte[0] != b'\n' {
                    line.push(byte[0]);
                    if line.len() >= LINE_MAX {
                        break;
                    }
                    continue;
                }
                let text = String::from_utf8_lossy(&line).trim_end_matches('\r').to_string();
                line.clear();
                match parse_request(&text) {
                    Some(Request::Ping) => {
                        let pong = ctl.lock().map(|mut w| write_line(&mut *w, &Answer::Pong.line()));
                        if !matches!(pong, Ok(Ok(()))) {
                            break;
                        }
                    }
                    Some(Request::Pong) => {}
                    _ => break,
                }
            }
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
        if heard.elapsed() >= config.silence {
            break;
        }
        if heard.elapsed() >= config.ping && sent.elapsed() >= config.ping {
            let ping = ctl.lock().map(|mut w| write_line(&mut *w, &Answer::Ping.line()));
            if !matches!(ping, Ok(Ok(()))) {
                break;
            }
            sent = Instant::now();
        }
    }
    retire(&shared, &code);
    log(&config, &format!("close {code}"));
}

fn retire(shared: &Shared, code: &str) {
    if let Ok(mut s) = shared.lock() {
        s.retire_code(code);
    }
}

/// C254: `JOIN <token>` goes down the host's control connection; no ACCEPT
/// within `config.answer` (10 s) is `NO host did not answer`.
fn join(mut stream: TcpStream, code: String, shared: Shared, config: Config) {
    let enlisted = shared.lock().map(|mut s| s.enlist(&code));
    let enlisted = match enlisted {
        Ok(Ok(e)) => e,
        Ok(Err(why)) => return refuse(&mut stream, why),
        Err(_) => return,
    };
    log(&config, &format!("join {code}"));
    let told = enlisted.ctl.lock().map(|mut w| write_line(&mut *w, &Answer::Join(enlisted.token.clone()).line()));
    if !matches!(told, Ok(Ok(()))) {
        claim(&shared, &enlisted.token);
        return refuse(&mut stream, "host did not answer");
    }
    match enlisted.wait.recv_timeout(config.answer) {
        Ok(host_side) => {
            log(&config, &format!("pair {code}"));
            crate::pipe::pair(stream, host_side);
            log(&config, &format!("close pair {code}"));
        }
        Err(RecvTimeoutError::Timeout) => {
            claim(&shared, &enlisted.token);
            refuse(&mut stream, "host did not answer");
        }
        Err(RecvTimeoutError::Disconnected) => refuse(&mut stream, "host left"),
    }
}

fn claim(shared: &Shared, token: &str) -> Option<std::sync::mpsc::Sender<TcpStream>> {
    shared.lock().ok()?.claim(token)
}

fn accept(mut stream: TcpStream, token: String, shared: Shared) {
    match claim(&shared, &token) {
        Some(hand) => {
            let _ = hand.send(stream);
        }
        None => refuse(&mut stream, "unknown token"),
    }
}
