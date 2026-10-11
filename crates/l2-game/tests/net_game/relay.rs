//! Join codes through an in-process relay on 127.0.0.1 (`docs/netcode.md` §7
//! *Join codes and the relay*, C254).

use crate::seats::setup;
use l2_game::net::{self, relay, Driver, Identity, Link, Phase, PortMap, Request};
use l2_game::screens::lobby::LobbyScreen;
use l2_game::Game;
use l2_kingdom::tables::Tables;
use l2_kingdom::Quirks;
use l2_net::PlayerSlot;
use l2_relay::{Config, Relay};
use std::time::{Duration, Instant};

const PATIENCE: Duration = Duration::from_secs(15);

fn relay_at() -> String {
    let relay = Relay::bind("127.0.0.1:0").expect("a loopback port");
    let at = relay.local_addr().unwrap().to_string();
    relay.with_config(Config { log: false, ..Config::default() }).spawn();
    at
}

fn seat(name: &str, shield: u8, auto: Option<u8>) -> (Game, Identity) {
    let mut game = Game::new(1);
    let id = Identity { name: name.into(), shield, setup: setup() };
    game.net.identity = Some(id.clone());
    game.net.auto = auto;
    (game, id)
}

fn hello(id: &Identity, seed: u64) -> l2_net::Hello {
    let settings = id.setup.settings(1, Quirks::default());
    net::hello(&settings, &Tables::DEFAULT, seed, PlayerSlot::new(0))
}

fn host_driver(id: &Identity, gold: i32, relay: Option<String>) -> Driver {
    let hello = hello(id, 0x1025);
    let q = Quirks::default();
    Driver::host(crate::loopback::ANY_PORT, hello, &id.name, id.shield, &id.setup, gold, q)
        .expect("listening")
        .with_relay(relay)
}

fn joiner_driver(id: &Identity, text: &str, relay: Option<&str>) -> Result<Driver, String> {
    Driver::join_via(text, relay, hello(id, 0), &id.name, id.shield, Quirks::default())
}

fn pump_until(
    game: &mut Game,
    driver: &mut Option<Driver>,
    done: impl Fn(&Game) -> bool,
) -> Option<l2_net::Start> {
    let until = Instant::now() + PATIENCE;
    while Instant::now() < until {
        let started = net::lobby(driver, game, None, Tables::DEFAULT);
        if started.is_some() || done(game) {
            return started;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("not reached in {PATIENCE:?}: {:?}", game.net.phase);
}

/// Red on ablation of `relay.pump()`'s `transport.add` in `Driver::pump`, of
/// `relay::join`'s arm in `join_via`, or of `link.code = d.code()` in `net::lobby`.
#[test]
fn a_host_gets_a_code_and_a_joiner_by_code_reaches_start_with_it() {
    let at = relay_at();
    let (mut host, host_id) = seat("Richard", 4, Some(2));
    let mut hd = Some(host_driver(&host_id, host.chosen_gold, Some(at.clone())));
    pump_until(&mut host, &mut hd, |g| g.net.code.is_some());
    let code = host.net.code.clone().unwrap();
    assert_eq!(code.len(), 6, "{code}");
    assert!(code.bytes().all(|b| l2_relay::ALPHABET.contains(&b)), "{code}");
    assert_eq!(host.net.relay, None);
    let shown = LobbyScreen::message(&host.net);
    assert_eq!(shown[0], format!("Join code: {code}"));
    assert!(shown[1].starts_with("Others join at 127.0.0.1:"), "{shown:?}");

    let (mut joiner, joiner_id) = seat("John", 2, Some(1));
    let typed = format!(" {} ", code.to_ascii_lowercase());
    let mut jd = Some(joiner_driver(&joiner_id, &typed, Some(&at)).expect("a join by code"));
    joiner.net.phase = Phase::Connecting(typed);
    let until = Instant::now() + PATIENCE;
    let (mut hs, mut js) = (None, None);
    while Instant::now() < until && (hs.is_none() || js.is_none()) {
        hs = hs.or(net::lobby(&mut hd, &mut host, None, Tables::DEFAULT));
        js = js.or(net::lobby(&mut jd, &mut joiner, None, Tables::DEFAULT));
        assert!(!matches!(joiner.net.phase, Phase::Failed(_)), "{:?}", joiner.net.phase);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(hs.is_some(), "host never started: {:?}", host.net.roster);
    assert_eq!(hs.map(|s| s.seed), js.map(|s| s.seed), "both seats start the same game");
    assert_eq!(host.net.roster.len(), 2);
    assert!(host.net.roster.players.iter().any(|p| p.name == "John"));
}

/// Red on ablation of `(Some(_), None) => Err(NOT_CONFIGURED)` in `join_via`.
#[test]
fn a_code_with_no_relay_configured_says_to_type_an_address() {
    let (_, id) = seat("Matilda", 4, None);
    let why = joiner_driver(&id, "ABCDEF", None).err().expect("refused at once");
    assert_eq!(why, relay::NOT_CONFIGURED);
    assert!(why.contains("Type the host's address instead."));
    // With a relay configured (DEFAULT_RELAY, LORDS2_RELAY) the request would
    // reach the internet from a test; the offline refusal is the case checked.
    if relay::configured().is_some() {
        return;
    }
    let (mut game, _) = seat("Matilda", 4, None);
    game.net.requests.push(Request::Join { address: "abcdef".into() });
    let mut driver: Option<Driver> = None;
    let _ = net::lobby(&mut driver, &mut game, None, Tables::DEFAULT);
    assert_eq!(game.net.phase, Phase::Failed(relay::NOT_CONFIGURED.into()));
    assert!(driver.is_none());
}

/// Red on ablation of the `"unknown code"` arm of `relay::join`.
#[test]
fn a_wrong_code_names_the_code_and_the_host() {
    let at = relay_at();
    let (mut game, id) = seat("Matilda", 4, None);
    let mut jd = Some(joiner_driver(&id, "ZZZZZZ", Some(&at)).expect("a join by code"));
    let t0 = Instant::now();
    pump_until(&mut game, &mut jd, |g| matches!(g.net.phase, Phase::Failed(_)));
    let Phase::Failed(why) = &game.net.phase else { unreachable!() };
    assert_eq!(why, "No game has the code ZZZZZZ. Check it with the host.");
    assert!(t0.elapsed() < Duration::from_secs(5), "{:?}", t0.elapsed());
    let said = LobbyScreen::message(&game.net).join(" ");
    assert!(said.contains("ZZZZZZ"), "{said}");
}

/// Red on ablation of `host_words` reaching `status` in `relay::Host::collect_opening`,
/// or of `link.relay = d.relay_status()` in `net::lobby`.
#[test]
fn a_host_whose_relay_is_down_keeps_hosting_by_address_and_says_so() {
    let listener = std::net::TcpListener::bind(crate::loopback::ANY_PORT).unwrap();
    let at = listener.local_addr().unwrap().to_string();
    drop(listener);
    let (mut host, id) = seat("Richard", 4, None);
    let mut hd = Some(host_driver(&id, host.chosen_gold, Some(at.clone())));
    pump_until(&mut host, &mut hd, |g| g.net.relay.is_some());
    assert_eq!(host.net.phase, Phase::Lobby);
    assert_eq!(host.net.code, None);
    let why = host.net.relay.clone().unwrap();
    assert!(why.starts_with(&format!("The join-code service at {at} did not answer.")), "{why}");
    assert!(why.ends_with("Others can still join by address."), "{why}");
    let shown = LobbyScreen::message(&host.net).join(" ");
    assert!(shown.starts_with("The join-code service at"), "{shown}");
    assert!(shown.contains("Others join at 127.0.0.1:"), "{shown}");
    let bound = hd.as_ref().and_then(|d| d.local_addr()).expect("still listening");
    assert!(std::net::TcpStream::connect(bound).is_ok(), "the port still takes a joiner");
}

/// Red on ablation of the `Phase::Lobby && link.hosting` guard in `LobbyScreen::message`.
#[test]
fn the_code_line_is_the_hosts_and_comes_first() {
    let lan: std::net::SocketAddr = "192.168.1.20:4712".parse().unwrap();
    let link = Link {
        phase: Phase::Lobby,
        hosting: true,
        reach: vec![lan],
        port: PortMap::Off,
        code: Some("ABCDEF".into()),
        relay: None,
        ..Link::default()
    };
    let lines = LobbyScreen::message(&link);
    assert_eq!(lines[0], "Join code: ABCDEF");
    assert!(lines[1].starts_with("Others join at 192.168.1.20:4712."), "{lines:?}");
    let client = Link { hosting: false, ..link.clone() };
    assert!(LobbyScreen::message(&client).is_empty());
    let none = Link { code: None, ..link };
    assert!(LobbyScreen::message(&none)[0].starts_with("Others join at"));
    assert_eq!(Link::default().code, None);
    let deployed: Result<std::net::SocketAddr, _> = relay::DEFAULT_RELAY.parse();
    assert!(deployed.is_ok(), "DEFAULT_RELAY is the deployed relay's host:port: {:?}", relay::DEFAULT_RELAY);
}
