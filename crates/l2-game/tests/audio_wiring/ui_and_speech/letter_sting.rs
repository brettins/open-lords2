#![allow(unused_imports)]
use super::*;
use l2_game::audio::{self, Audio};
use l2_game::game::Assets;
use l2_game::message;
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::Game;

/// Each file first heard, with its tick (1-based); `heard()` is a set.
fn listen(rec: message::Record, lord: u8, ticks: usize) -> Vec<(usize, String)> {
    let Some(dir) = l2_testkit::install_dir() else { return Vec::new() };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let assets = Assets::placeholder();
    let mut game = world();
    game.kingdom.realms[2].in_play = true;
    game.kingdom.realms[2].lord = lord;
    let mut machine = Machine::new(APP_ROOT);
    machine.push(ScreenId::Campaign);
    let mut audio = Audio::headless(&platform.vfs);
    let mut director = audio::Director::welcomed();
    assert!(game.messages.enqueue(rec, game.player), "the record was accepted");
    let mut out = Vec::new();
    for t in 1..=ticks {
        let seen: Vec<String> = audio.heard().iter().map(|n| n.to_string()).collect();
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        machine.update(&mut ctx);
        director.listen(&mut audio, &machine, &game);
        out.extend(audio.heard().into_iter().filter(|n| !seen.iter().any(|s| s == n)).map(|n| (t, n.to_ascii_lowercase())));
    }
    out
}

fn record(category: u8, group: u16) -> message::Record {
    let mut rec = message::Record::default();
    rec.group = group;
    rec.category = category;
    rec.from = 2;
    rec.to = 1;
    rec
}

/// `FUN_004B3B92(lord - 1)` at `g_messageTimer == 0x776`, 90 ticks after the
/// fanfare, and the voice 110 after that at 0x708. Table `0x004E2470`. Ablate:
/// the `lord_sting` call in `Director::listen`.
#[test]
fn a_lords_letter_stings_ninety_ticks_in() {
    if l2_testkit::install_dir().is_none() {
        l2_testkit::skip!("no game install");
    }
    let heard = listen(record(message::category::LETTER, 170), 2, 220);
    let tick = |name: &str| heard.iter().find(|(_, n)| n.eq_ignore_ascii_case(name)).map(|(t, _)| *t);
    let fanfare = tick(audio::names::fanfare::MESSAGE).expect("no fanfare");
    let sting = tick("S246_04.wav").unwrap_or_else(|| panic!("no sting for lord 2 - heard {heard:?}"));
    assert_eq!(sting - fanfare, 2000 - 0x776);
}

/// `Msg_DrawDiplomacy` (`0x00475E07`) opens on `ff_msg.wav` and speaks no
/// line and no sting. Ablate: `c::DIPLOMACY` in `open_fanfare`.
#[test]
fn a_diplomatic_letter_opens_on_the_trumpet_and_says_nothing() {
    if l2_testkit::install_dir().is_none() {
        l2_testkit::skip!("no game install");
    }
    let heard = listen(record(message::category::DIPLOMACY, 0xF6), 2, 240);
    assert!(heard.contains(&(1, audio::names::fanfare::MESSAGE.to_string())), "{heard:?}");
    assert!(heard.iter().all(|(_, n)| n == "ff_msg.wav" || n == "scroll1.wav"), "{heard:?}");
}

#[test]
fn the_sting_table_is_read_by_lord_minus_one() {
    let rec = record(message::category::PAY_PROMPT, 0xB0);
    assert_eq!(audio::lord_sting(&rec, 0x776, 1, true), Some("S246_02.wav"));
    assert_eq!(audio::lord_sting(&rec, 0x776, 4, true), Some("S246_01.wav"));
    assert_eq!(audio::lord_sting(&rec, 0x776, 0, true), None, "lord 0 is below the guard");
    assert_eq!(audio::lord_sting(&rec, 0x777, 1, true), None);
    let end = record(message::category::ENDING, 0xC2);
    assert_eq!(audio::lord_sting(&end, 2000, 3, false), Some("S246_03.wav"));
    assert_eq!(audio::lord_sting(&end, 2000, 3, true), None, "the animated arm is the film's");
}

/// `Battle_ReturnToCampaign` (`0x004AB383`), 004a0000.c:4716..4771: ff_lose.wav to
/// the winner's owner only. Ablate: `winner == player` in `audio::after_battle`.
#[test]
fn the_verdict_fanfare_goes_to_the_side_that_did_not_lose() {
    assert_eq!(audio::after_battle(true, 1, 3, 1), Some("ff_lose.wav"));
    assert_eq!(audio::after_battle(false, 3, 1, 1), Some("ff_lose.wav"));
    assert_eq!(audio::after_battle(false, 1, 3, 1), None, "the player lost");
    assert_eq!(audio::after_battle(true, 2, 3, 1), None, "not the player's battle");
}
