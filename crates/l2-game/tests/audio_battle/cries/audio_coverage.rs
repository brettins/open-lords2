#![allow(unused_imports)]
use super::*;
use super::troop_cries::*;
use super::*;
use super::events::*;
use super::determinism::*;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use l2_game::audio::{self, names, Audio, Request, TroopCries};
use l2_game::battlefield::{self as bf, cry, Cry, LiveBattle};
use l2_game::game::Assets;
use l2_game::input::{Event, Key};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::Game;
use l2_sim::runner::{Army, BattleRunner};
use l2_sim::{Cues, Troop, SIDE_A, SIDE_B};

/// **Every call the battlefield's ladder can make, pinned.** Twenty-two sites,
/// seventeen files, and the verb each one uses: every bank slot is
/// drop-if-busy on its own buffer, and the wall coming down and the bridge
/// catching are the one-shot buffer.
///
/// A new arm makes this red with the request it added — which is how the six
/// that fire, oil, the tower and the high rampart added arrived here.
#[test]
fn the_battle_ladder_is_twenty_two_calls_and_seventeen_files() {
    let every = Cues::of_every_occasion();
    let asked = audio::battle_requests(&Cues::default(), &every);
    assert_eq!(
        asked,
        [
            Request::Slot(4),
            Request::Slot(5),
            Request::Slot(5),
            Request::Slot(6),
            Request::Slot(0xb),
            Request::Slot(0xc),
            Request::Slot(0xb),
            Request::Slot(0xc),
            Request::Slot(0xf),
            Request::Slot(0x10),
            Request::Slot(10),
            Request::Slot(8),
            Request::Slot(10),
            Request::Slot(8),
            Request::Slot(0xd),
            Request::Slot(9),
            Request::Slot(7),
            Request::Slot(0xe),
            Request::File("bathit2.wav"),
            Request::Slot(3),
            Request::Slot(0x11),
            Request::File("dest_ind.wav"),
        ]
    );
    let files: BTreeSet<&str> = asked.into_iter().map(file_of).collect();
    assert_eq!(
        files.into_iter().collect::<Vec<_>>(),
        [
            "bathit2.wav",
            "bow_hit.wav",
            "bowmen1.wav",
            "catfire.wav",
            "cathit.wav",
            "catmiss.wav",
            "cros_hit.wav",
            "crossbow.wav",
            "deadguy2.wav",
            "deadguy3.wav",
            "deadguy4.wav",
            "dest_ind.wav",
            "pouroil.wav",
            "siegedoc.wav",
            "sword2.wav",
            "sword3.wav",
            "sword5.wav",
        ]
    );
    // And a tick in which nothing happened asks for nothing.
    assert!(audio::battle_requests(&every, &every).is_empty());
}

/// **Every cry a player can hear ships, and the ones D34 says cannot be asked
/// for are the ones that do not** — including two the bug list said were real.
#[test]
fn every_reachable_cry_ships_and_nine_unreachable_names_do_not() {
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install");
    };
    let mut cries = TroopCries::default();
    let mut reachable = BTreeSet::new();
    for troop in 0..11u8 {
        for class in 0..4u8 {
            for _ in 0..4 {
                if let Some(n) = cries.cry(troop, class) {
                    reachable.insert(n);
                }
            }
        }
    }
    assert_eq!(reachable.len(), 66, "the reachable cries moved");
    for n in &reachable {
        assert!(find(&dir, n).is_some(), "{n} is reachable and does not ship");
    }
    // The three cells of every `_M` row that class 3 can never select. Seven
    // are `_F1` and two are cell 14, which `docs/bugs.md` D34 used to say always
    // named a real file.
    for n in [
        "Peas_F1.wav", "Cros_F1.wav", "Mace_F1.wav", "Swor_F1.wav", "Pike_F1.wav", "Arch_F1.wav",
        "Knig_F1.wav", "Swor_U3.wav", "Arch_U3.wav",
    ] {
        assert!(!reachable.contains(n), "{n} should be unreachable");
        assert!(find(&dir, n).is_none(), "{n} ships after all");
    }
}

/// **`TROOP_CRIES` is the executable's table**, cell for cell.
#[test]
fn the_troop_cry_table_is_the_one_at_0x004db0d0() {
    let exe = l2_testkit::executable!();
    let base = l2_testkit::pe::va_to_offset(&exe, 0x004D_B0D0).expect("the table is in the image");
    for (t, troop) in names::TROOP_CRIES.iter().enumerate() {
        for (c, class) in troop.iter().enumerate() {
            for (k, name) in class.iter().enumerate() {
                let at = base + t * 0x100 + c * 0x40 + k * 0x10;
                let cell = &exe[at..at + 16];
                let end = cell.iter().position(|&b| b == 0).unwrap_or(16);
                assert_eq!(
                    std::str::from_utf8(&cell[..end]).unwrap(),
                    *name,
                    "g_troopSounds[{t}][{c}][{k}]"
                );
            }
        }
    }
}

/// **The throttle is the original's and nothing else.** `Sound_PlayFile` has
/// one buffer: a cry over a cry is dropped, a cry over the narrator is dropped,
/// and **the take a dropped cry would have played is spent** — because the
/// counter steps before the drop.
///
/// Played through the director, from orders given as events. Ablation: move
/// the busy test in `Audio::play_file` after the load and `peas_p3.wav` is heard.
#[test]
fn a_cry_over_a_cry_is_dropped_and_its_take_is_spent() {
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install, so nothing to drop");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let mut sound = Audio::headless(&platform.vfs);
    let mut director = audio::Director::new();
    let a = Assets::placeholder();
    let (mut g, mut m) = staged(&[(Troop::Peasants, 4)], &[(Troop::Knights, 1)]);
    let drain = |sound: &mut Audio, name: &str| {
        let mut buf = vec![0f32; 4096];
        for _ in 0..2_000 {
            if !sound.is_playing(name) {
                return;
            }
            sound.mix(&mut buf);
        }
        panic!("{name} never finished");
    };

    box_the_army(&mut m, &mut g, &a);
    director.listen(&mut sound, &m, &g);
    assert!(sound.heard().contains(&"peas_u2.wav"), "the selection's cry: {:?}", sound.heard());
    drain(&mut sound, "peas_u2.wav");

    // Two orders inside one tick. The first is take 1; the second is take 2,
    // asked for while take 1 is sounding, and dropped.
    let ground = pixel(live(&g), empty_cell(live(&g)));
    click_at(&mut m, &mut g, &a, ground);
    click_at(&mut m, &mut g, &a, ground);
    director.listen(&mut sound, &m, &g);
    assert!(sound.heard().contains(&"peas_p2.wav"), "{:?}", sound.heard());
    assert!(!sound.heard().contains(&"peas_p3.wav"), "a cry over a cry was played");
    drain(&mut sound, "peas_p2.wav");

    // The next order is take 3, not take 2: the dropped cry spent its take.
    click_at(&mut m, &mut g, &a, ground);
    director.listen(&mut sound, &m, &g);
    assert!(sound.heard().contains(&"peas_p4.wav"), "{:?}", sound.heard());
    assert!(!sound.heard().contains(&"peas_p3.wav"), "the dropped take came back");

    // And the narrator holds the same buffer.
    drain(&mut sound, "peas_p4.wav");
    assert!(sound.play_file("S021_01.wav", true), "the narrator, into an idle buffer");
    assert!(!sound.play_file("Knig_E2.wav", true), "a cry played over the narrator");
}


