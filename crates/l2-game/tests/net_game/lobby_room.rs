//! C255: the lobby's three pages driven through `Screen::handle`: entry, the
//! code box, the room's Start game and I'm ready.

use l2_game::game::Assets;
use l2_game::input::{Event, Key};
use l2_game::net::{Link, Phase, Request};
use l2_game::screen::{Ctx, Screen, ScreenId, Transition};
use l2_game::screens::lobby::{Item, LobbyScreen, Page, CODE_CAPTION};
use l2_game::screens::setup::{SetupPage, SetupScreen};
use l2_game::Game;
use l2_net::{Player, PlayerSlot, Roster};

fn send(screen: &mut LobbyScreen, game: &mut Game, assets: &Assets, event: Event) -> Transition {
    let mut ctx = Ctx { game, assets };
    screen.handle(event, &mut ctx)
}

fn settle(screen: &mut LobbyScreen, game: &mut Game, assets: &Assets) {
    let mut ctx = Ctx { game, assets };
    screen.update(&mut ctx);
}

fn typed(screen: &mut LobbyScreen, game: &mut Game, assets: &Assets, s: &str) {
    for c in s.chars() {
        send(screen, game, assets, Event::Text(c));
    }
}

fn player(slot: u8, name: &str, is_host: bool, ready: bool) -> Player {
    Player { slot: PlayerSlot::new(slot), name: name.into(), ready, is_host, shield: slot + 1 }
}

fn room(hosting: bool, me: u8, players: Vec<Player>) -> Game {
    let mut game = Game::new(1);
    game.net = Link {
        phase: Phase::Lobby,
        hosting,
        roster: Roster { players },
        me: Some(PlayerSlot::new(me)),
        ..Link::default()
    };
    game
}

/// Red on ablation of`entry::ITEMS`; `Item::Host => Transition::Replace(...)` in `press`;
/// `network: page == SetupPage::Shield` in `SetupScreen::new`.
#[test]
fn the_entry_page_offers_host_join_and_back_and_nothing_else() {
    let (mut game, assets) = (Game::new(1), Assets::placeholder());
    let mut screen = LobbyScreen::new();
    settle(&mut screen, &mut game, &assets);
    assert_eq!(screen.page(), Page::Entry);
    let items = screen.items(&game.net);
    assert_eq!(items, vec![Item::Host, Item::Join, Item::Back]);
    let labels: Vec<&str> = items.iter().map(|i| i.label()).collect();
    assert_eq!(labels, vec!["Host a game", "Join a game", "Back"]);
    let rects = screen.rects(&game.net);
    assert_eq!(rects.len(), 3, "every button has a hit area");
    let host = rects[0].click();
    assert_eq!(send(&mut screen, &mut game, &assets, host), Transition::Replace(ScreenId::Setup(SetupPage::Shield)));
    assert!(SetupScreen::new(SetupPage::Shield).network(), "page 4 opened by Host a game is the host path");
    assert!(!SetupScreen::new(SetupPage::Title).network());
    assert_eq!(send(&mut screen, &mut game, &assets, Event::KeyDown(Key::Escape)), Transition::Pop);
    assert!(game.net.requests.is_empty(), "the entry page sends nothing");
}

/// Red on ablation of`l2_relay::ALPHABET.contains` in `JoinPage::type_event`, or its `to_ascii_uppercase`.
#[test]
fn the_code_box_upper_cases_and_refuses_what_is_not_in_the_alphabet() {
    let (mut game, assets) = (Game::new(1), Assets::placeholder());
    let mut screen = LobbyScreen::new();
    send(&mut screen, &mut game, &assets, Event::KeyDown(Key::Down));
    send(&mut screen, &mut game, &assets, Event::KeyDown(Key::Enter));
    assert_eq!(screen.page(), Page::Join, "Join a game is the second item");
    typed(&mut screen, &mut game, &assets, "ab0O1Ii!2c");
    assert_eq!(screen.code(), "AB2C", "0, O, 1, I, i and ! are refused; letters come out upper-case");
    typed(&mut screen, &mut game, &assets, "defg");
    assert_eq!(screen.code(), "AB2CDE", "six symbols and no more");
    send(&mut screen, &mut game, &assets, Event::KeyDown(Key::Backspace));
    assert_eq!(screen.code(), "AB2CD");
    assert_eq!(screen.items(&game.net), vec![Item::Connect, Item::UseAddress, Item::Back]);
}

/// Red on ablation of`Item::Connect` in `press`; `prefs.kept.player_name` in `settle`.
#[test]
fn enter_on_the_join_page_sends_request_join_with_the_code_and_who_you_are() {
    let (mut game, assets) = (Game::new(1), Assets::placeholder());
    let mut screen = LobbyScreen::new();
    send(&mut screen, &mut game, &assets, Event::KeyDown(Key::Down));
    send(&mut screen, &mut game, &assets, Event::KeyDown(Key::Enter));
    send(&mut screen, &mut game, &assets, Event::KeyDown(Key::Enter));
    assert!(game.net.requests.is_empty(), "an empty box joins nothing");
    typed(&mut screen, &mut game, &assets, "qwerty");
    assert_eq!(send(&mut screen, &mut game, &assets, Event::KeyDown(Key::Enter)), Transition::Stay);
    assert_eq!(game.net.requests, vec![Request::Join { address: "QWERTY".into() }]);
    let id = game.net.identity.clone().expect("the joiner has a name and a shield");
    assert_eq!(id.name, game.prefs.kept.player_name.as_str());
    assert_eq!(id.shield, 1);
}

/// Red on ablation of`Item::UseAddress | Item::UseCode` in `press`; `Item::Back` on the join page.
#[test]
fn use_an_address_instead_swaps_the_box_for_the_host_port_field_and_back_returns_to_entry() {
    let (mut game, assets) = (Game::new(1), Assets::placeholder());
    let mut screen = LobbyScreen::new();
    send(&mut screen, &mut game, &assets, Event::KeyDown(Key::Down));
    send(&mut screen, &mut game, &assets, Event::KeyDown(Key::Enter));
    let link = screen.rects(&game.net)[1].click();
    send(&mut screen, &mut game, &assets, link);
    assert!(screen.by_address());
    assert_eq!(screen.items(&game.net), vec![Item::Connect, Item::UseCode, Item::Back]);
    send(&mut screen, &mut game, &assets, Event::KeyDown(Key::Enter));
    assert_eq!(game.net.requests, vec![Request::Join { address: Link::default_address() }]);
    game.net.requests.clear();
    game.net.phase = Phase::Failed("No answer.".into());
    assert_eq!(send(&mut screen, &mut game, &assets, Event::KeyDown(Key::Escape)), Transition::Stay);
    assert_eq!(screen.page(), Page::Entry);
    assert_eq!(game.net.requests, vec![Request::Leave], "a failed join is released on Back");
}

/// Red on ablation of`waiting_for` in `enabled`, or in the `Item::Start` arm of `press`.
#[test]
fn the_hosts_start_game_waits_for_the_seat_that_is_not_ready() {
    let assets = Assets::placeholder();
    let mut game = room(true, 0, vec![player(0, "Richard", true, true), player(1, "John", false, false)]);
    let mut screen = LobbyScreen::new();
    settle(&mut screen, &mut game, &assets);
    assert_eq!(screen.page(), Page::Room);
    assert_eq!(screen.items(&game.net), vec![Item::Start, Item::Leave]);
    assert_eq!(Item::Start.label(), "Start game");
    assert!(!screen.enabled(Item::Start, &game.net));
    assert_eq!(l2_game::screens::lobby::waiting_for(&game.net).as_deref(), Some("John"));
    send(&mut screen, &mut game, &assets, Event::KeyDown(Key::Enter));
    assert!(game.net.requests.is_empty(), "a greyed button does nothing");
    game.net.roster.players[1].ready = true;
    assert!(screen.enabled(Item::Start, &game.net));
    assert_eq!(l2_game::screens::lobby::waiting_for(&game.net), None);
    send(&mut screen, &mut game, &assets, Event::KeyDown(Key::Enter));
    assert_eq!(game.net.requests, vec![Request::Start]);
}

/// Red on ablation ofthe `ready` pick in `room::items`; `Item::Leave` in `press`.
#[test]
fn the_joiners_button_toggles_between_im_ready_and_not_ready() {
    let assets = Assets::placeholder();
    let mut game = room(false, 1, vec![player(0, "Richard", true, true), player(1, "John", false, false)]);
    let mut screen = LobbyScreen::new();
    settle(&mut screen, &mut game, &assets);
    assert_eq!(screen.page(), Page::Room);
    assert_eq!(screen.items(&game.net), vec![Item::Ready, Item::Leave]);
    assert_eq!(Item::Ready.label(), "I'm ready");
    send(&mut screen, &mut game, &assets, Event::KeyDown(Key::Enter));
    assert_eq!(game.net.requests, vec![Request::Ready(true)]);
    game.net.requests.clear();
    game.net.roster.players[1].ready = true;
    assert_eq!(screen.items(&game.net), vec![Item::NotReady, Item::Leave]);
    assert_eq!(Item::NotReady.label(), "Not ready");
    let main = screen.rects(&game.net)[0].click();
    send(&mut screen, &mut game, &assets, main);
    assert_eq!(game.net.requests, vec![Request::Ready(false)]);
    game.net.requests.clear();
    assert_eq!(send(&mut screen, &mut game, &assets, Event::KeyDown(Key::Escape)), Transition::Pop);
    assert_eq!(game.net.requests, vec![Request::Leave]);
}

/// Red on ablation ofthe `link.code` arm of `message`; `reach_text`'s relay prefix.
#[test]
fn the_room_shows_the_code_line_when_the_link_has_one() {
    let lan: std::net::SocketAddr = "192.168.1.20:4712".parse().unwrap();
    let link = Link { phase: Phase::Lobby, hosting: true, reach: vec![lan], code: Some("ABCDEF".into()), ..Link::default() };
    let lines = l2_game::screens::lobby::message(&link);
    assert_eq!(lines[0], "Join code: ABCDEF");
    assert_eq!(CODE_CAPTION, "Tell your friends this code");
    let none = Link { code: None, relay: Some("No code today.".into()), ..link.clone() };
    let said = l2_game::screens::lobby::message(&none).join(" ");
    assert!(said.starts_with("No code today. Others join at 192.168.1.20:4712."), "{said}");
    assert!(l2_game::screens::lobby::reach_text(&Link { hosting: false, ..link }).is_none());
}

/// Red on ablation of`pending_join` in `settle`; `JoinPage::seed`'s `as_code` arm.
#[test]
fn a_join_already_on_the_link_opens_the_join_page_with_its_code_or_address() {
    let assets = Assets::placeholder();
    for (target, code, by_address) in [("abcdef", "ABCDEF", false), ("10.0.0.2:4712", "", true)] {
        let mut game = Game::new(1);
        game.net.requests.push(Request::Join { address: target.into() });
        let mut screen = LobbyScreen::new();
        settle(&mut screen, &mut game, &assets);
        assert_eq!(screen.page(), Page::Join, "{target}");
        assert_eq!(screen.code(), code, "{target}");
        assert_eq!(screen.by_address(), by_address, "{target}");
    }
    let mut game = Game::new(1);
    game.net.requests.push(Request::Host { addr: crate::loopback::ANY_PORT });
    let mut screen = LobbyScreen::new();
    settle(&mut screen, &mut game, &assets);
    assert_eq!(screen.page(), Page::Room, "a pending Host is the room");
}

/// Red on ablation of`shield_rect().contains` in `JoinPage::click`.
#[test]
fn clicking_the_shield_on_the_join_page_cycles_through_the_five() {
    let (mut game, assets) = (Game::new(1), Assets::placeholder());
    let mut screen = LobbyScreen::new();
    send(&mut screen, &mut game, &assets, Event::KeyDown(Key::Down));
    send(&mut screen, &mut game, &assets, Event::KeyDown(Key::Enter));
    assert_eq!(screen.shield(), 1);
    let mut seen = Vec::new();
    for _ in 0..5 {
        send(&mut screen, &mut game, &assets, Event::Click { x: 0x1C8 + 30, y: 0xE8 + 30 });
        seen.push(screen.shield());
    }
    assert_eq!(seen, vec![2, 3, 4, 5, 1]);
}
