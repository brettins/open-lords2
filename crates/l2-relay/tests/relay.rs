//! Protocol v1 against an in-process relay on 127.0.0.1 (C254).

use std::io::{ErrorKind, Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

use l2_relay::client::{self, Fault};
use l2_relay::protocol::{self, parse_answer, parse_request, read_line, write_line, Answer, Request};
use l2_relay::{Config, Relay, ALPHABET, CODE_LEN, LINE_MAX, TOKEN_LEN};

const T: Duration = Duration::from_secs(5);

fn relay(config: Config) -> String {
    let relay = Relay::bind("127.0.0.1:0").expect("a loopback port");
    let at = relay.local_addr().unwrap().to_string();
    relay.with_config(config).spawn();
    at
}

fn quiet() -> Config {
    Config { log: false, ..Config::default() }
}

fn is_code(text: &str) -> bool {
    text.len() == CODE_LEN && text.bytes().all(|b| ALPHABET.contains(&b))
}

/// Red on ablation of `issue_code` in `State`; `Answer::Code` in `session::host`.
#[test]
fn a_host_gets_a_six_symbol_code_from_the_alphabet() {
    let at = relay(quiet());
    let (_ctl, code) = client::host(&at, T).expect("HOST 1");
    assert!(is_code(&code), "{code:?}");
    let (_ctl2, other) = client::host(&at, T).unwrap();
    assert!(is_code(&other) && other != code, "{other:?}");
}

/// Red on ablation of `"unknown code"` in `State::enlist`.
#[test]
fn a_wrong_code_is_refused_by_name() {
    let at = relay(quiet());
    let (_ctl, _code) = client::host(&at, T).unwrap();
    match client::join(&at, "ZZZZZZ", T, T) {
        Err(Fault::Refused(why)) => assert_eq!(why, "unknown code"),
        other => panic!("{other:?}"),
    }
}

fn read_join_token(ctl: &mut TcpStream) -> String {
    match parse_answer(&read_line(ctl).expect("a line on the control connection")) {
        Some(Answer::Join(token)) => token,
        other => panic!("{other:?}"),
    }
}

/// Red on ablation of `pipe::pair`'s second `copy` (one direction only); `Answer::Ok`
/// on either side.
#[test]
fn a_right_code_pairs_and_bytes_flow_both_ways() {
    let at = relay(quiet());
    let (mut ctl, code) = client::host(&at, T).unwrap();
    let joining = {
        let at = at.clone();
        std::thread::spawn(move || client::join(&at, &code.to_ascii_lowercase(), T, T))
    };
    let token = read_join_token(&mut ctl);
    assert_eq!(token.len(), TOKEN_LEN);
    assert!(token.bytes().all(|b| ALPHABET.contains(&b)));
    let mut host_side = client::accept(&at, &token, T).expect("ACCEPT 1");
    let mut joiner_side = joining.join().unwrap().expect("paired");
    host_side.write_all(b"from the host").unwrap();
    joiner_side.write_all(b"from the joiner").unwrap();
    let mut got = [0u8; 15];
    joiner_side.read_exact(&mut got[..13]).unwrap();
    assert_eq!(&got[..13], b"from the host");
    host_side.read_exact(&mut got).unwrap();
    assert_eq!(&got, b"from the joiner");
    let big = vec![0xA5u8; 200 * 1024];
    let reader = {
        let mut joiner_side = joiner_side.try_clone().unwrap();
        std::thread::spawn(move || {
            let mut all = vec![0u8; 200 * 1024];
            joiner_side.read_exact(&mut all).unwrap();
            all
        })
    };
    host_side.write_all(&big).unwrap();
    assert_eq!(reader.join().unwrap(), big, "200 KiB through a 64 KiB pipe");
}

/// Red on ablation of the two `shutdown` calls at the end of `pipe::copy`.
#[test]
fn the_pair_closes_when_one_side_closes() {
    let at = relay(quiet());
    let (mut ctl, code) = client::host(&at, T).unwrap();
    let joining = {
        let at = at.clone();
        std::thread::spawn(move || client::join(&at, &code, T, T))
    };
    let token = read_join_token(&mut ctl);
    let host_side = client::accept(&at, &token, T).unwrap();
    let mut joiner_side = joining.join().unwrap().unwrap();
    drop(host_side);
    joiner_side.set_read_timeout(Some(T)).unwrap();
    let mut byte = [0u8; 1];
    let end = joiner_side.read(&mut byte);
    assert!(matches!(end, Ok(0)) || end.is_err(), "{end:?}");
}

/// Red on ablation of `recv_timeout(config.answer)` in `session::join` made `recv()`.
#[test]
fn a_joiner_times_out_when_the_host_never_accepts() {
    let at = relay(Config { answer: Duration::from_millis(300), ..quiet() });
    let (mut ctl, code) = client::host(&at, T).unwrap();
    let t0 = Instant::now();
    let refused = client::join(&at, &code, T, T);
    let _token = read_join_token(&mut ctl);
    match refused {
        Err(Fault::Refused(why)) => assert_eq!(why, "host did not answer"),
        other => panic!("{other:?}"),
    }
    assert!(t0.elapsed() < Duration::from_secs(3), "{:?}", t0.elapsed());
}

/// Red on ablation of the host's control connection dropped keeps the code alive.
#[test]
fn the_code_dies_with_the_control_connection() {
    let relay = Relay::bind("127.0.0.1:0").unwrap().with_config(quiet());
    let at = relay.local_addr().unwrap().to_string();
    let live = relay.counter();
    relay.spawn();
    let (ctl, code) = client::host(&at, T).unwrap();
    assert_eq!(live(), 1);
    drop(ctl);
    let until = Instant::now() + T;
    while live() != 0 && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(live(), 0, "the code outlived its connection");
    match client::join(&at, &code, T, T) {
        Err(Fault::Refused(why)) => assert_eq!(why, "unknown code"),
        other => panic!("{other:?}"),
    }
}

/// Red on ablation of `line.len() >= LINE_MAX` in `protocol::read_line`.
#[test]
fn an_over_long_handshake_line_is_closed_without_an_answer() {
    let at = relay(quiet());
    let mut raw = TcpStream::connect(&at).unwrap();
    raw.set_read_timeout(Some(T)).unwrap();
    raw.write_all(&[b'A'; 2 * LINE_MAX]).unwrap();
    let mut byte = [0u8; 1];
    let end = raw.read(&mut byte);
    assert!(matches!(end, Ok(0)) || end.is_err(), "{end:?}");
    let mut gibberish = TcpStream::connect(&at).unwrap();
    gibberish.set_read_timeout(Some(T)).unwrap();
    write_line(&mut gibberish, "GET / HTTP/1.1").unwrap();
    let end = gibberish.read(&mut byte);
    assert!(matches!(end, Ok(0)) || end.is_err(), "{end:?}");
}

/// Red on ablation of `Request::Ping` in `session::host`; `config.ping` set to an hour.
#[test]
fn the_control_connection_answers_ping_and_pings_an_idle_host() {
    let at = relay(Config { ping: Duration::from_millis(100), ..quiet() });
    let (mut ctl, _code) = client::host(&at, T).unwrap();
    write_line(&mut ctl, &Request::Ping.line()).unwrap();
    assert_eq!(parse_answer(&read_line(&mut ctl).unwrap()), Some(Answer::Pong));
    assert_eq!(parse_answer(&read_line(&mut ctl).unwrap()), Some(Answer::Ping));
}

/// Red on ablation of `config.silence` ignored in `session::host`.
#[test]
fn a_silent_host_is_dropped() {
    let config = Config { ping: Duration::from_millis(50), silence: Duration::from_millis(200), ..quiet() };
    let at = relay(config);
    let (mut ctl, _code) = client::host(&at, T).unwrap();
    let until = Instant::now() + T;
    let mut sink = [0u8; 64];
    loop {
        match ctl.read(&mut sink) {
            Ok(0) => break,
            Ok(_) => {}
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
        assert!(Instant::now() < until, "still open after {T:?}");
    }
}

/// Red on ablation of any arm of `parse_request`; the `words.next().is_none()` tail.
#[test]
fn the_alphabet_and_the_lines() {
    assert_eq!(ALPHABET.len(), 32);
    for forbidden in b"0O1I" {
        assert!(!ALPHABET.contains(forbidden));
    }
    assert_eq!(protocol::as_code(" abcdef\n"), Some("ABCDEF".into()));
    assert_eq!(protocol::as_code("ABCDE"), None);
    assert_eq!(protocol::as_code("ABCDEF1"), None);
    assert_eq!(protocol::as_code("ABCD0F"), None);
    assert_eq!(protocol::as_code("127.0.0.1:4712"), None);
    assert_eq!(parse_request("HOST 1"), Some(Request::Host));
    assert_eq!(parse_request("HOST 2"), None);
    assert_eq!(parse_request("JOIN 1 ABCDEF"), Some(Request::Join("ABCDEF".into())));
    assert_eq!(parse_request("JOIN 1 ABCDEF extra"), None);
    assert_eq!(parse_request("ACCEPT 1 ABCDEFGH"), Some(Request::Accept("ABCDEFGH".into())));
    assert_eq!(parse_request("ACCEPT 1 ABCDEF"), None);
    assert_eq!(parse_answer("CODE ABCDEF"), Some(Answer::Code("ABCDEF".into())));
    assert_eq!(parse_answer("NO unknown code"), Some(Answer::No("unknown code".into())));
    assert_eq!(Answer::Join("ABCDEFGH".into()).line(), "JOIN ABCDEFGH");
    let mut buffer = b"PING\r\nJOIN ABCDEFGH\nPAR".to_vec();
    let lines = protocol::split_lines(&mut buffer).unwrap();
    assert_eq!(lines, vec!["PING".to_string(), "JOIN ABCDEFGH".into()]);
    assert_eq!(buffer, b"PAR");
    let mut long = vec![b'A'; LINE_MAX];
    assert_eq!(protocol::split_lines(&mut long), None);
}
