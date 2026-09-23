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
