//! C260: a joiner whose connection drops is waited for by every seat; it comes back on
//! the same install and the host's world, or the host lets the computer play. Real TCP
//! on 127.0.0.1, the real lobby, the waiting box driven as a player drives it.

use crate::kit::{self, Seat};
use crate::seats::assets;
use l2_game::game::Assets;
use l2_game::input::{Event, Key};
use l2_game::net::{Phase, TIMEOUT};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::net_wait::NetWaitScreen;
use std::time::{Duration, Instant};

/// The process goes without a goodbye; every other seat shows whom it waits for.
fn drop_seat(seats: &mut Vec<Seat>, assets: &Assets, i: usize) -> String {
    let name = seats[i].game.net.identity.as_ref().unwrap().name.clone();
    drop(seats.remove(i));
    let want = vec![name.clone()];
    for _ in 0..TIMEOUT / 4 {
        if seats.iter().all(|x| x.game.net.waiting == want) {
            return name;
        }
        kit::run(seats, assets, 1);
        std::thread::sleep(Duration::from_millis(1));
    }
    panic!("the drop was not noticed before the silence timeout");
}

/// Pumps everyone until `seat` has a session or its join failed.
fn knock(seats: &mut Vec<Seat>, assets: &Assets, seat: Seat) -> Result<(), String> {
    seats.push(seat);
    let t0 = Instant::now();
    while seats.last().unwrap().live.is_none() {
        assert!(t0.elapsed() < Duration::from_secs(30), "no answer to the rejoin");
        kit::run(seats, assets, 1);
        if let Phase::Failed(why) = &seats.last().unwrap().game.net.phase {
            return Err(why.clone());
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    Ok(())
}

/// Ablations: `self.away(slot)` in `Live::dropped` (only the silence timeout notices);
/// `self.waiting.contains(&s)` in `Live::knock` (refused); the `unsimulated` resend in
/// `Live::admit` (the seats stall apart); the `NetWait` hold in `Machine::keep_halt_on_top`;
/// C262 `world.tips.resume()` in `link::rejoined` (the Objectives scroll again).
#[test]
fn a_dropped_joiner_pauses_every_seat_then_rejoins_and_later_hashes_agree() {
    kit::big_stack(drop_and_rejoin);
}

fn drop_and_rejoin() {
    let Some(assets) = assets() else { l2_testkit::skip!("no install") };
    let _scope = kit::scope("rejoin");
    let (mut seats, port) = kit::game(&assets, 3);
    let realm = seats[1].game.player;
    drop_seat(&mut seats, &assets, 1);
    kit::run(&mut seats, &assets, 50);
    let frozen: Vec<u32> = seats.iter().map(Seat::tick).collect();
    kit::run(&mut seats, &assets, 300);
    assert_eq!(seats.iter().map(Seat::tick).collect::<Vec<_>>(), frozen, "a seat played on");
    let mut m = Machine::new(ScreenId::Campaign);
    let mut ctx = Ctx { game: &mut seats[1].game, assets: &assets };
    m.update(&mut ctx);
    assert_eq!(m.top_id(), Some(ScreenId::NetWait), "John is shown the wait");
    assert_eq!(NetWaitScreen::heading(&ctx), "Waiting for Matilda");
    kit::shot(&mut m, &ctx, "wait-joiner");
    m.handle(Event::KeyDown(Key::Escape), &mut ctx);
    m.update(&mut ctx);
    assert_eq!(m.top_id(), Some(ScreenId::NetWait), "Escape reached under the box");

    knock(&mut seats, &assets, Seat::joiner("Matilda", 1, port)).expect("taken back");
    let back = seats.last().unwrap();
    assert_eq!(back.game.player, realm, "back on its own realm");
    let mut seen = Machine::new(ScreenId::Campaign);
    for _ in 0..60 {
        kit::run(&mut seats, &assets, 1);
        let back = seats.last_mut().unwrap();
        seen.update(&mut Ctx { game: &mut back.game, assets: &assets });
    }
    let back = seats.last().unwrap();
    assert!(back.game.prefs.tip_screens, "tips off proves nothing");
    assert_eq!(back.game.tips.shows(), 0, "the rejoined seat was shown {:?}", seen.ids());
    let at = frozen[0];
    kit::until(&mut seats, &assets, "all three run on", |s| s.iter().all(|x| x.tick() > at + 200));
    let t = kit::common_tick(&seats);
    let hashes: Vec<Option<u64>> = seats.iter().map(|s| s.hash_at(t)).collect();
    assert!(hashes[0].is_some() && hashes.iter().all(|h| *h == hashes[0]), "{t:?}: {hashes:?}");
    assert!(seats.iter().all(|s| s.game.net.waiting.is_empty() && s.game.net.halted.is_none()));
    assert_eq!(seats.iter().map(|s| s.live().session.divergences()).sum::<u32>(), 0);
    let mut ctx = Ctx { game: &mut seats[1].game, assets: &assets };
    m.update(&mut ctx);
    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "the waiting box stayed up");
}

/// Ablations: the `ComputerPlays` arm of `NetWaitScreen::press`; `self.session.retire`
/// in `Live::to_computer` (the host still waits).
#[test]
fn the_host_lets_the_computer_play_from_the_waiting_box_and_play_resumes() {
    kit::big_stack(computer_plays);
}

fn computer_plays() {
    let Some(assets) = assets() else { l2_testkit::skip!("no install") };
    let _scope = kit::scope("computer");
    let (mut seats, _) = kit::game(&assets, 2);
    drop_seat(&mut seats, &assets, 1);
    let frozen = seats[0].tick();
    let mut m = Machine::new(ScreenId::Campaign);
    {
        let mut ctx = Ctx { game: &mut seats[0].game, assets: &assets };
        m.update(&mut ctx);
        assert_eq!(m.top_id(), Some(ScreenId::NetWait));
        kit::shot(&mut m, &ctx, "wait-host");
        let stack = m.ids();
        for e in [Event::Click { x: 30, y: 8 }, Event::KeyDown(Key::Escape)] {
            m.handle(e, &mut ctx);
            m.update(&mut ctx);
            assert_eq!(m.ids(), stack, "{e:?} reached under the box");
        }
        let computer = NetWaitScreen::buttons(&ctx)[0];
        m.handle(Event::Click { x: computer.x + 4, y: computer.y + 4 }, &mut ctx);
    }
    kit::until(&mut seats, &assets, "the host plays on", |s| s[0].tick() > frozen + 150);
    let g = &mut seats[0].game;
    assert!(g.net.waiting.is_empty());
    assert!(!g.kingdom.realms[2].is_human, "the computer plays realm 2");
    let mut ctx = Ctx { game: g, assets: &assets };
    m.update(&mut ctx);
    assert!(!m.ids().contains(&ScreenId::NetWait), "the box stayed");
}

/// Ablations: the `NoSeat::Retired` arm of `Live::knock` (told only that the game
/// started); `no_seat`'s words in `Driver::pump`.
#[test]
fn a_rejoin_after_the_computer_took_the_realm_is_refused_in_plain_words() {
    kit::big_stack(late);
}

fn late() {
    let Some(assets) = assets() else { l2_testkit::skip!("no install") };
    let _scope = kit::scope("late");
    let (mut seats, port) = kit::game(&assets, 2);
    drop_seat(&mut seats, &assets, 1);
    seats[0].game.net.computer_plays = true;
    let frozen = seats[0].tick();
    kit::until(&mut seats, &assets, "the host plays on", |s| s[0].tick() > frozen + 50);
    let why = knock(&mut seats, &assets, Seat::joiner("Matilda", 1, port)).unwrap_err();
    assert_eq!(why, "That game went on without you: the computer plays your realm now.");
    seats.pop();
    let why = knock(&mut seats, &assets, Seat::joiner("Geoffrey", 7, port)).unwrap_err();
    assert_eq!(why, "That game has already started without you.");
    seats.pop();
    let at = seats[0].tick();
    kit::until(&mut seats, &assets, "the host is undisturbed", |s| s[0].tick() > at + 50);
    assert!(seats[0].game.net.halted.is_none());
}
