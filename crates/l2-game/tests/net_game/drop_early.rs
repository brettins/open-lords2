//! mp-leftovers: a seat lost before its first tick packet is waited for at once (C260),
//! not only once `TIMEOUT` passes.

use crate::kit::{self, Seat};
use crate::seats::assets;
use l2_game::net::{Phase, Request, QUIET};
use std::time::{Duration, Instant};

/// Ablation: `live.peers = game.net.seat_peers.clone()` in `Live::new` (the host
/// knows John's connection only from his first packet, so his drop goes unseen).
#[test]
fn a_joiner_lost_before_its_first_packet_is_waited_for_at_once() {
    let Some(assets) = assets() else { l2_testkit::skip!("no install") };
    let (host, port) = kit::host(&assets);
    let mut seats = vec![host, Seat::joiner("Matilda", 1, port), Seat::joiner("John", 2, port)];
    let t0 = Instant::now();
    let (mut readied, mut started) = ([false; 3], false);
    while seats.iter().any(|s| s.live.is_none()) {
        assert!(t0.elapsed() < Duration::from_secs(30), "no start");
        for (i, s) in seats.iter_mut().enumerate() {
            if i == 2 && s.live.is_some() {
                continue;
            }
            s.pump(&assets);
            let link = &mut s.game.net;
            if i > 0 && link.phase == Phase::Lobby && link.me.is_some() && !readied[i] {
                link.requests.push(Request::Ready(true));
                readied[i] = true;
            }
        }
        let link = &mut seats[0].game.net;
        if !started && link.roster.len() == 3 && link.roster.players.iter().all(|p| p.ready) {
            link.requests.push(Request::Start);
            started = true;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    drop(seats.pop());
    for _ in 0..QUIET / 2 {
        kit::run(&mut seats, &assets, 1);
        if !seats[0].game.net.waiting.is_empty() {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(seats[0].game.net.waiting, vec!["John".to_string()]);
    assert!(seats[0].live().session.halt_reason().is_none());
}
