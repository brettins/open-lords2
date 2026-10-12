//! C261: a joiner that reads `Start` and the host's first tick packets in one pass
//! keeps the packets for its session. Found by three real processes on 127.0.0.1: one
//! start in six stuck with the host at tick 3 and the joiner at tick 0.

use crate::kit::{self, Seat};
use crate::seats::assets;
use l2_game::net::{Phase, Request};
use std::time::Duration;

/// Ablation: the `break` after `LobbyEvent::Started` in `Driver::pump` (the joiner
/// never leaves tick 0).
#[test]
fn ticks_behind_start_in_one_read_reach_the_session() {
    kit::big_stack(burst);
}

fn burst() {
    let Some(assets) = assets() else { l2_testkit::skip!("no install") };
    let _scope = kit::scope("burst");
    let (host, port) = kit::host(&assets);
    let mut seats = vec![host, Seat::joiner("Matilda", 1, port)];
    kit::until(&mut seats, &assets, "the joiner is seated", |s| s[1].game.net.me.is_some() && s[1].game.net.phase == Phase::Lobby);
    seats[1].game.net.requests.push(Request::Ready(true));
    kit::until(&mut seats, &assets, "both ready", |s| s[0].game.net.roster.players.iter().filter(|p| p.ready).count() == 2);
    seats[0].game.net.requests.push(Request::Start);
    for _ in 0..200 {
        seats[0].pump(&assets);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(seats[0].live.is_some(), "the host is playing and has sent its first ticks");
    std::thread::sleep(Duration::from_millis(100));
    kit::until(&mut seats, &assets, "both play on", |s| s[1].tick() > 40 && s[0].tick() > 40);
}
