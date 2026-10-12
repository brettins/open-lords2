//! The path a player takes, end to end: host and joiner through a relay by code,
//! `net::service` builds each world, `into_transport` hands the sockets to `Live`,
//! and turns run. The owner's first relay game halted at once (2026-10-11).

use crate::seats::{assets, setup};
use l2_game::net::{self, Driver, Identity, Live};
use l2_game::Game;
use l2_kingdom::tables::Tables;
use l2_relay::{Config, Relay};
use std::time::{Duration, Instant};

fn seat(name: &str, shield: u8, auto: Option<u8>) -> Game {
    let mut game = Game::new(1);
    game.net.identity = Some(Identity { name: name.into(), shield, setup: setup() });
    game.net.auto = auto;
    game
}

fn play_through(at: &str) {
    let Some(assets) = assets() else { l2_testkit::skip!("no install") };
    let tables = Tables::DEFAULT;
    let mut host = seat("Richard", 4, Some(2));
    let mut hd: Option<Driver> = None;
    host.net.requests.push(net::Request::Host { addr: crate::loopback::ANY_PORT });
    net::relay::configure(at);
    let until = Instant::now() + Duration::from_secs(15);
    while host.net.code.is_none() {
        assert!(net::service(&mut hd, &mut host, &assets, tables).is_none());
        assert!(Instant::now() < until, "no code: {:?}", host.net.phase);
        std::thread::sleep(Duration::from_millis(5));
    }
    let code = host.net.code.clone().unwrap();
    let mut joiner = seat("John", 2, Some(1));
    let mut jd: Option<Driver> = None;
    joiner.net.requests.push(net::Request::Join { address: code.clone() });
    let (mut hw, mut jw) = (None, None);
    while hw.is_none() || jw.is_none() {
        hw = hw.or_else(|| net::service(&mut hd, &mut host, &assets, tables));
        jw = jw.or_else(|| net::service(&mut jd, &mut joiner, &assets, tables));
        assert!(Instant::now() < until, "no start: {:?} / {:?}", host.net.phase, joiner.net.phase);
        std::thread::sleep(Duration::from_millis(5));
    }
    let (mut hw, mut jw) = (hw.unwrap(), jw.unwrap());
    let mut hl = Live::new(hd.take().unwrap().into_transport().unwrap(), &mut hw, 1).expect("host seat");
    let mut jl = Live::new(jd.take().unwrap().into_transport().unwrap(), &mut jw, 1).expect("joiner seat");
    let until = Instant::now() + Duration::from_secs(10);
    while hl.session.tick().0 < 200 || jl.session.tick().0 < 200 {
        hl.pump(&mut hw);
        jl.pump(&mut jw);
        assert_eq!(hw.net.halted, None, "host halted at tick {}", hl.session.tick().0);
        assert_eq!(jw.net.halted, None, "joiner halted at tick {}", jl.session.tick().0);
        assert!(Instant::now() < until, "stuck: host {} joiner {}", hl.session.tick().0, jl.session.tick().0);
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn a_game_joined_by_code_through_a_relay_plays_two_hundred_ticks() {
    let relay = Relay::bind("127.0.0.1:0").expect("a loopback port");
    let at = relay.local_addr().unwrap().to_string();
    relay.with_config(Config { log: false, ..Config::default() }).spawn();
    play_through(&at);
}

/// The deployed relay, when asked: `LORDS2_RELAY_LIVE=1`.
#[test]
fn a_game_joined_by_code_through_the_live_relay_plays_two_hundred_ticks() {
    if std::env::var_os("LORDS2_RELAY_LIVE").is_none() {
        return;
    }
    play_through(net::relay::DEFAULT_RELAY);
}
