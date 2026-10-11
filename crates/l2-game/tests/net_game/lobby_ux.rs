//! net-join-blocks, net-player-ux: a join never freezes the window, a failed one
//! says why in the player's words, and a host is told where it is reached.

use crate::seats::setup;
use l2_game::net::{self, Driver, Identity, Link, Phase, Request};
use l2_game::screens::lobby::{Item, LobbyScreen};
use l2_game::Game;
use l2_kingdom::tables::Tables;
use std::time::{Duration, Instant};

fn joiner(address: String) -> Game {
    let mut game = Game::new(1);
    game.net.identity = Some(Identity { name: "Matilda".into(), shield: 4, setup: setup() });
    game.net.requests.push(Request::Join { address });
    game
}

/// Runs the lobby until it fails; the longest single pass is returned.
fn until_failed(game: &mut Game, driver: &mut Option<Driver>) -> (String, Duration) {
    let mut longest = Duration::ZERO;
    let t0 = Instant::now();
    while t0.elapsed() < Duration::from_secs(10) {
        let pass = Instant::now();
        let _ = net::lobby(driver, game, None, Tables::DEFAULT);
        longest = longest.max(pass.elapsed());
        if let Phase::Failed(why) = &game.net.phase {
            return (why.clone(), longest);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("the join never failed; phase {:?}", game.net.phase);
}

/// Ablation: `connect` called on the UI thread in `Driver::join` (a pass
/// blocks for the connect); `Phase::Connecting` in `net::lobby`.
#[test]
fn a_join_to_a_closed_port_connects_off_the_ui_thread_and_says_why_it_failed() {
    let port = std::net::TcpListener::bind(crate::loopback::ANY_PORT).unwrap().local_addr().unwrap().port();
    let mut game = joiner(format!("127.0.0.1:{port}"));
    let mut driver: Option<Driver> = None;
    let _ = net::lobby(&mut driver, &mut game, None, Tables::DEFAULT);
    assert!(matches!(game.net.phase, Phase::Connecting(_)), "{:?}", game.net.phase);
    assert_eq!(LobbyScreen::items(&game.net), vec![Item::Leave], "only Leave while connecting");
    assert!(LobbyScreen::message(&game.net)[0].starts_with("Connecting to 127.0.0.1:"));
    let (why, longest) = until_failed(&mut game, &mut driver);
    assert!(longest < Duration::from_millis(250), "one pass held the window {longest:?}");
    assert!(why.contains("refused the connection") || why.starts_with("No answer from"), "{why}");
}

/// Ablation: `bad` in `connect` (the resolver's own text reaches the screen).
#[test]
fn a_join_to_something_that_is_not_an_address_says_how_to_type_one() {
    let mut game = joiner("the castle".into());
    let (why, _) = until_failed(&mut game, &mut None);
    assert!(why.starts_with("\"the castle\" is not an address."), "{why}");
    assert!(LobbyScreen::message(&game.net).join(" ").contains("192.168.1.20:4712"));
}

/// Ablation: `link.reach = d.reach()` in `net::lobby`; `reach_of`'s loopback arm.
#[test]
fn a_host_is_shown_the_address_others_type() {
    let mut game = Game::new(1);
    game.net.identity = Some(Identity { name: "Richard".into(), shield: 4, setup: setup() });
    game.net.requests.push(Request::Host { addr: crate::loopback::ANY_PORT });
    let mut driver: Option<Driver> = None;
    let _ = net::lobby(&mut driver, &mut game, None, Tables::DEFAULT);
    assert_eq!(game.net.phase, Phase::Lobby);
    let bound = driver.as_ref().and_then(|d| d.local_addr()).expect("listening");
    assert_eq!(game.net.reach, vec![bound]);
    let said = LobbyScreen::message(&game.net).join(" ");
    assert!(said.starts_with(&format!("Others join at {bound}.")), "{said}");
    assert!(said.contains("firewall"), "{said}");
    let lan: std::net::SocketAddr = "192.168.1.20:4712".parse().unwrap();
    let link = Link { phase: Phase::Lobby, hosting: true, reach: vec![lan], ..Link::default() };
    assert!(LobbyScreen::message(&link)[0].starts_with("Others join at 192.168.1.20:4712."));
}

/// Ablation: `link.port = d.port()` in `net::lobby`; the `PortMap` arms of
/// `LobbyScreen::message` (C252).
#[test]
fn a_host_is_told_what_its_router_did_with_the_port() {
    use l2_game::net::PortMap;
    let lan: std::net::SocketAddr = "192.168.1.20:4712".parse().unwrap();
    let at = |port: PortMap| {
        let link = Link { phase: Phase::Lobby, hosting: true, reach: vec![lan], port, ..Link::default() };
        LobbyScreen::message(&link).join(" ")
    };
    assert!(at(PortMap::Asking).contains("Asking your router to open port 4712"), "{}", at(PortMap::Asking));
    let open = at(PortMap::Open { public: "203.0.113.7:4712".parse().unwrap(), via: "NAT-PMP", lifetime: 7200 });
    assert!(open.starts_with("On your network, others join at 192.168.1.20:4712. Over the internet, at 203.0.113.7:4712 (NAT-PMP opened the port for 2 hours)."), "{open}");
    assert!(open.contains("firewall"));
    let shut = at(PortMap::Shut("no router answered NAT-PMP or UPnP".into()));
    assert!(shut.contains("Over the internet: no router answered NAT-PMP or UPnP. Forward TCP port 4712"), "{shut}");
    assert_eq!(Link::default().port, PortMap::Off);
}

/// Ablation: the `link.auto` block in `net::lobby` (C253).
#[test]
fn players_n_readies_a_joiner_and_starts_the_host_at_n_seats() {
    let mut host = Game::new(1);
    host.net.identity = Some(Identity { name: "Richard".into(), shield: 4, setup: setup() });
    host.net.auto = Some(2);
    host.net.requests.push(Request::Host { addr: crate::loopback::ANY_PORT });
    let mut hd: Option<Driver> = None;
    let _ = net::lobby(&mut hd, &mut host, None, Tables::DEFAULT);
    let bound = hd.as_ref().and_then(|d| d.local_addr()).expect("listening");
    let mut joiner = Game::new(1);
    joiner.net.identity = Some(Identity { name: "John".into(), shield: 2, setup: setup() });
    joiner.net.auto = Some(1);
    joiner.net.requests.push(Request::Join { address: bound.to_string() });
    let mut jd: Option<Driver> = None;
    let until = Instant::now() + Duration::from_secs(10);
    let (mut hs, mut js) = (None, None);
    while Instant::now() < until && (hs.is_none() || js.is_none()) {
        hs = hs.or(net::lobby(&mut hd, &mut host, None, Tables::DEFAULT));
        js = js.or(net::lobby(&mut jd, &mut joiner, None, Tables::DEFAULT));
        assert!(!matches!(joiner.net.phase, Phase::Failed(_)), "{:?}", joiner.net.phase);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(hs.is_some(), "host never started: {:?}", host.net.roster);
    assert_eq!(hs.map(|s| s.seed), js.map(|s| s.seed), "both seats start the same game");
}
