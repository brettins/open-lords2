#![allow(unused_imports)]
use super::*;
use super::screen_tips::*;
use super::input_and_layout::*;
use super::game_events::*;
use super::*;
use std::collections::BTreeMap;
use l2_game::audio::{self, Audio};
use l2_game::game::Assets;
use l2_game::input::Event;
use l2_game::message::{self, Frame};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::options::{self, Page, Setting};
use l2_game::tip::{self, Tips, View};
use l2_game::Game;
use l2_kingdom::units_tick::Incursion;

/// **The words are the player's own `L2.eng`**, and our transcription agrees
/// with it string for string — which is what makes it a fallback
/// rewrite. `CLAUDE.md` rule 6.
#[test]
fn every_tip_draws_the_players_own_words_and_our_transcription_is_them() {
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install, so no L2.eng");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let assets = Assets::load(&platform.vfs).expect("assets load");
    let mut strings = 0;
    for (group, words) in tip::TEXT {
        for (i, want) in words.iter().enumerate() {
            assert_eq!(assets.shell.text(*group as usize, i), *want, "L2.eng {group}/{i}");
            assert_eq!(tip::words(&assets.shell, *group, i), *want);
            strings += 1;
        }
    }
    assert_eq!(strings, 53, "fourteen groups: fourteen headings and thirty-nine paragraphs");
}

#[test]
fn the_kingdom_overview_tip_puts_its_ok_button_where_one_line_puts_it() {
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install, so no body font to measure with");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let assets = Assets::load(&platform.vfs).expect("assets load");
    let mut g = world();
    let mut m = Machine::new(ScreenId::Campaign);
    g.tips = armed();
    assert!(tip::show(&mut g, 206));
    tick(&mut m, &mut g, &assets);
    let r = *g.messages.open().expect("pulled on 0x27");
    let ctx = Ctx { game: &mut g, assets: &assets };
    let frame = l2_game::screens::message::window_frame(&ctx, &r).expect("a tip window");
    assert_eq!(frame, Frame { x: 0x10, y: 0x80, w: 0x1C0, h: 0xC0 });
    assert_eq!(frame.ok_button(), (416, 272));
}

/// `Msg_DrawWindow#24` and `FUN_004B3ACD`, through the real machine, the real
/// director and the real mixer, with no device. The expected ticks are
/// computed from what the mixer reports — when a clip was last sounding — and
/// the rule's own numbers, typed: 81 ticks for `timer < 0x780`, and 64 ticks
/// for *"more than 999 ms"* at 16 ms a tick, counted from the tick the director
/// last saw him busy.
#[test]
fn a_tip_reads_its_first_line_and_then_its_takes_a_second_apart() {
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install, so no voice clips");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let (mut g, a, mut m) = campaign();
    let mut sound = Audio::headless(&platform.vfs);
    let mut director = audio::Director::new();
    let mut buf = vec![0f32; 706 * 2];

    let names = ["s200_01.wav", "s200_02.wav", "s200_03.wav"];
    let mut first: BTreeMap<&str, usize> = BTreeMap::new();
    let mut last_sounding: BTreeMap<&str, usize> = BTreeMap::new();
    let mut opened_at = None;
    let mut first_line_timer = None;
    for t in 1..=8000 {
        tick(&mut m, &mut g, &a);
        if opened_at.is_none() && g.messages.timer() == 2000 && open_group(&g) == Some(200) {
            opened_at = Some(t);
        }
        director.listen(&mut sound, &m, &g);
        for n in names {
            if !first.contains_key(n) && sound.heard().contains(&n) {
                first.insert(n, t);
                if n == names[0] {
                    first_line_timer = Some(g.messages.timer());
                }
            }
        }
        sound.mix(&mut buf);
        for n in names {
            if sound.is_playing(n) {
                last_sounding.insert(n, t);
            }
        }
        if first.contains_key(names[2]) && last_sounding[names[2]] < t {
            break;
        }
    }
    let opened = opened_at.expect("tip 200 opened");
    assert_eq!(first_line_timer, Some(0x7C6), "the first line on Msg_DrawWindow's tick");

    let t2 = *first.get(names[1]).expect("S200_02.wav was never played");
    let t3 = *first.get(names[2]).expect("S200_03.wav was never played: the cursor did not advance");
    let end1 = last_sounding[names[0]];
    let end2 = last_sounding[names[1]];
    let expect2 = if end1 + 1 >= opened + 81 { end1 + 1 + 63 } else { opened + 81 };
    assert_eq!(t2, expect2, "S200_02: opened {opened}, first line last sounding {end1}");
    assert_eq!(t3, end2 + 1 + 63, "S200_03: S200_02 last sounding {end2}");

    let pool: Vec<String> =
        l2_game::audio::names::TAKE_POOL.iter().map(|n| n.to_ascii_lowercase()).collect();
    let takes: Vec<&str> = sound.heard().into_iter().filter(|h| pool.iter().any(|p| p == h)).collect();
    assert_eq!(takes, vec!["s200_02.wav", "s200_03.wav"]);
}

/// **A troop cry holds a tip's next take back**
/// because there is one buffer. `FUN_004B3ACD` asks `Sound_OneShotBusy()`, and
/// `Sound_PlayTroopCry` is `Sound_PlayFile` into the same `DAT_00522AEC` the
/// first line went into — so a cry keeps the next take waiting until a full
/// second after the cry ends. That is what folding the tips' and the battle's
/// two records of the buffer into one field was for (`docs/decisions.md`
/// C166), and until this test nothing had put a cry in front of a take.
#[test]
fn a_troop_cry_holds_back_a_tips_next_take_as_the_narrator_does() {
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install, so no voice clips");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    const TAKE: &str = "s200_02.wav";
    const CRY: &str = "knig_e2.wav";

    let run = |cry_on: Option<usize>| -> (Option<usize>, Option<usize>) {
        let (mut g, a, mut m) = campaign();
        let mut sound = Audio::headless(&platform.vfs);
        let mut director = audio::Director::new();
        let mut buf = vec![0f32; 706 * 2];
        let mut cry_last = None;
        for t in 1..=8000 {
            tick(&mut m, &mut g, &a);
            if cry_on == Some(t) {
                assert!(sound.play_file(CRY, true), "the buffer was busy on the take's own tick");
            }
            director.listen(&mut sound, &m, &g);
            if sound.heard().contains(&TAKE) {
                return (Some(t), cry_last);
            }
            sound.mix(&mut buf);
            if sound.is_playing(CRY) {
                cry_last = Some(t);
            }
        }
        (None, cry_last)
    };

    let on = run(None).0.expect("S200_02.wav was never played");
    let (take, cry_last) = run(Some(on));
    assert_ne!(take, Some(on), "the take talked over the cry on tick {on}");
    let take = take.expect("S200_02.wav was never played once the cry had finished");
    let cry_last = cry_last.expect("the cry never sounded");
    assert_eq!(take, cry_last + 1 + 63, "a second of quiet from the cry's last tick, {cry_last}");
}


