//! `docs/netcode.md` *The lobby*, §7, D-5: two peers over TCP on 127.0.0.1,
//! each pumping its own driver the way the app loop does.

use crate::seats::{assets, setup};
use l2_game::net::{self, Driver, Identity, Link, NetGame, Phase, Request};
use l2_game::Game;
use l2_kingdom::tables::Tables;
use l2_net::state_hash;
use std::sync::mpsc;
use std::time::{Duration, Instant};

const PATIENCE: Duration = Duration::from_secs(20);

fn seat(name: &str, shield: u8, first: Request) -> Game {
    let mut game = Game::new(1);
    game.net.identity = Some(Identity { name: name.into(), shield, setup: setup() });
    game.net.requests.push(first);
    game
}

/// Pumps `service` until it hands back a world, as `App::pump_net` does.
fn run(mut game: Game, tables: Tables, port: Option<mpsc::Sender<u16>>) -> Result<Game, String> {
    let Some(mut assets) = assets() else { return Err("no install".into()) };
    assets.wall_clock = Some(if port.is_some() { 1_000 } else { 9_999_999 });
    let mut driver: Option<Driver> = None;
    let mut port = port;
    let (mut readied, mut started) = (false, false);
    let t0 = Instant::now();
    while t0.elapsed() < PATIENCE {
        if let Some(world) = net::service(&mut driver, &mut game, &assets, tables) {
            return Ok(world);
        }
        if let Phase::Failed(why) = &game.net.phase {
            return Err(why.clone());
        }
        if let (Some(tx), Some(d)) = (&port, &driver) {
            let bound = d.local_addr().unwrap();
            assert!(bound.ip().is_loopback(), "the test host bound {bound}");
            tx.send(bound.port()).unwrap();
            port = None;
        }
        let link = &mut game.net;
        if link.phase == Phase::Lobby && !link.hosting && !readied {
            link.requests.push(Request::Ready(true));
            readied = true;
        }
        if link.hosting && link.roster.all_ready() && !started {
            link.requests.push(Request::Start);
            started = true;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    Err("timed out".into())
}

fn two_peers(client_tables: Tables) -> (Result<Game, String>, Result<Game, String>) {
    let (tx, rx) = mpsc::channel();
    let host = seat("Richard", 4, Request::Host { addr: crate::loopback::ANY_PORT });
    let host = std::thread::spawn(move || run(host, Tables::DEFAULT, Some(tx)));
    let Ok(port) = rx.recv_timeout(PATIENCE) else {
        return (host.join().unwrap(), Err("the host never listened".into()));
    };
    let client = seat("Matilda", 4, Request::Join { address: format!("127.0.0.1:{port}") });
    let client = std::thread::spawn(move || run(client, client_tables, None)).join().unwrap();
    // A refused client leaves the host waiting; its Err is then the timeout.
    let host = if client.is_ok() { host.join().unwrap() } else { Err("abandoned".into()) };
    (host, client)
}

/// Ablations: `setup` in `Start`'s codec; `name_the_lords`' seated arm;
/// `Mismatch::Seed` in `on_join`'s filter; `Driver::me`'s name lookup.
#[test]
fn two_peers_over_tcp_host_join_ready_and_start_one_world() {
    if assets().is_none() {
        l2_testkit::skip!("no install");
    }
    let (a, b) = two_peers(Tables::DEFAULT);
    let (a, b) = (a.expect("the host's world"), b.expect("the client's world"));
    assert_eq!((a.player, b.player), (1, 2), "each peer drives its own seat");
    assert!(a.multiplayer && b.multiplayer);
    assert_eq!(a.net.phase, Phase::Started);
    for g in [&a, &b] {
        assert_eq!(g.player_names[1].as_str(), "Richard");
        assert_eq!(g.player_names[2].as_str(), "Matilda");
        let realms = &g.kingdom.realms;
        assert_eq!((realms[1].shield_index, realms[2].shield_index), (4, 1), "D-3b");
        assert!(!realms[4].in_play, "the host's three lords, not the client's five");
    }
    let (a, b) = (NetGame::new(a), NetGame::new(b));
    assert_eq!(state_hash(&a), state_hash(&b), "turn 0 differs between the peers");
}

/// D-12. Ablation: `ruleset_hash` in `net::hello`; `refusal` in `Driver::pump`.
#[test]
fn a_mismatched_ruleset_is_refused_on_the_joining_screen() {
    if assets().is_none() {
        l2_testkit::skip!("no install");
    }
    let mut modded = Tables::DEFAULT;
    modded.season[0].death_rate += 1;
    let (_, client) = two_peers(modded);
    let why = client.expect_err("a modded client joined");
    assert!(why.contains("mod set differs"), "{why}");
    let link = Link { phase: Phase::Failed(why), ..Link::default() };
    let items = l2_game::screens::lobby::LobbyScreen::items(&link);
    assert_eq!(items[0], l2_game::screens::lobby::Item::Host, "the screen offers Host/Join again");
}
