//! `l2-relay [--listen 0.0.0.0:4714]` (C254). One line per event on stdout;
//! never a game byte.

use l2_relay::Relay;

const DEFAULT_LISTEN: &str = "0.0.0.0:4714";

fn usage() -> ! {
    eprintln!("usage: l2-relay [--listen <addr:port>]   (default {DEFAULT_LISTEN})");
    std::process::exit(2)
}

fn main() {
    let mut listen = DEFAULT_LISTEN.to_string();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--listen" => listen = args.next().unwrap_or_else(|| usage()),
            _ => usage(),
        }
    }
    let relay = match Relay::bind(&listen) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("l2-relay: cannot listen on {listen}: {e}");
            std::process::exit(1)
        }
    };
    if let Err(e) = relay.serve() {
        eprintln!("l2-relay: {e}");
        std::process::exit(1)
    }
}
