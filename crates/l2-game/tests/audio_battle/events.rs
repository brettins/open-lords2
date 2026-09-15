#![allow(unused_imports)]
use super::*;
use super::cries::*;
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

#[test]
fn the_sword_a_man_falls_to_is_chosen_by_the_troop_that_struck_him() {
    for (troop, sword, not) in [
        (Troop::Knights, "sword2.wav", ["sword3.wav", "sword5.wav"]),
        (Troop::Macemen, "sword5.wav", ["sword2.wav", "sword3.wav"]),
        (Troop::Peasants, "sword3.wav", ["sword2.wav", "sword5.wav"]),
    ] {
        let (asked, live) = fight(&[(troop, 3)], &[(troop, 3)], 12_000, true);
        assert!(asked.contains(sword), "{troop:?} asked for {asked:?}");
        for n in not {
            assert!(!asked.contains(n), "{troop:?} asked for {n}: {asked:?}");
        }
        let lost = |side| cells_of(&live, side).len() < 3;
        assert_eq!(asked.contains("deadguy2.wav"), lost(SIDE_A), "{troop:?}: {asked:?}");
        assert_eq!(asked.contains("deadguy3.wav"), lost(SIDE_B), "{troop:?}: {asked:?}");
        assert!(lost(SIDE_A) || lost(SIDE_B), "{troop:?}: nobody died in 12,000 ticks");
    }
}

#[test]
fn a_bow_and_a_crossbow_are_heard_as_themselves() {
    for (troop, loose, hit, other) in [
        (Troop::Archers, "bowmen1.wav", "bow_hit.wav", ["crossbow.wav", "cros_hit.wav"]),
        (Troop::Crossbowmen, "crossbow.wav", "cros_hit.wav", ["bowmen1.wav", "bow_hit.wav"]),
    ] {
        let (asked, _) = fight(&[(troop, 6)], &[(Troop::Peasants, 1)], 6_000, false);
        assert!(asked.contains(loose), "{troop:?}: {asked:?}");
        assert!(asked.contains(hit), "{troop:?}: {asked:?}");
        assert!(asked.contains("deadguy4.wav"), "{troop:?} shot nobody dead: {asked:?}");
        for n in other {
            assert!(!asked.contains(n), "{troop:?} asked for {n}: {asked:?}");
        }
        assert!(!asked.contains("catfire.wav"));
    }
}


#[test]
fn the_battlefield_is_eighty_three_more_files() {
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install, so nothing to open");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let mut sound = Audio::headless(&platform.vfs);
    for r in audio::battle_requests(&Cues::default(), &Cues::of_every_occasion()) {
        sound.play_effect(file_of(r));
    }
    let mut cries = TroopCries::default();
    for troop in 0..11u8 {
        for class in 0..4u8 {
            for _ in 0..4 {
                if let Some(n) = cries.cry(troop, class) {
                    sound.play_effect(n);
                }
            }
        }
    }
    let heard = sound.heard();
    let ladder = [
        "bathit2.wav", "bow_hit.wav", "bowmen1.wav", "cathit.wav", "catfire.wav", "catmiss.wav",
        "cros_hit.wav", "crossbow.wav", "deadguy2.wav", "deadguy3.wav", "deadguy4.wav",
        "dest_ind.wav", "pouroil.wav", "siegedoc.wav", "sword2.wav", "sword3.wav", "sword5.wav",
    ];
    for f in ladder {
        assert!(heard.contains(&f), "{f} was not opened: {heard:?}");
    }
    assert_eq!(heard.len() - ladder.len(), 66, "the cries: {heard:?}");
    assert_eq!(heard.len(), 83);
}

