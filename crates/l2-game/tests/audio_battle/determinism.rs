#![allow(unused_imports)]
use super::*;
use super::cries::*;
use super::events::*;
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

/// One battle, played twice from the same events: once with a
/// [`audio::Director`] listening after every tick, once with nothing listening.
/// Returns the two games.
fn play_it_twice(sound: &mut Audio) -> (Game, Game) {
    let a = Assets::placeholder();
    let armies = (
        [(Troop::Swordsmen, 3), (Troop::Archers, 3)],
        [(Troop::Crossbowmen, 3), (Troop::Macemen, 3)],
    );
    let (mut heard, mut hm) = staged(&armies.0, &armies.1);
    let (mut silent, mut sm) = staged(&armies.0, &armies.1);
    let mut director = audio::Director::new();

    for (g, m) in [(&mut heard, &mut hm), (&mut silent, &mut sm)] {
        box_the_army(m, g, &a);
        let enemy = *cells_of(live(g), SIDE_B).first().unwrap();
        let at = pixel(live(g), enemy);
        click_at(m, g, &a, at);
        // Park the pointer mid-field so the edge scroll never moves either
        // camera.
        send(m, g, &a, Event::Pointer { x: 240, y: 240 });
    }
    assert_eq!(heard.battle, silent.battle, "the same events left the two games different");

    for t in 0..4_000 {
        tick(&mut hm, &mut heard, &a);
        director.listen(sound, &hm, &heard);
        tick(&mut sm, &mut silent, &a);
        assert!(heard.battle == silent.battle, "sound changed the battle at tick {t}");
        if t % 500 == 250 {
            // An order mid-battle, to both, so a cry lands while men are dying.
            for (g, m) in [(&mut heard, &mut hm), (&mut silent, &mut sm)] {
                send(m, g, &a, Event::KeyDown(Key::letter('h')));
            }
        }
    }
    (heard, silent)
}

/// **Sound does not change the battle, tick for tick** — with the silent
/// layer, so it runs everywhere.
///
/// Compared by the whole `LiveBattle`'s derived equality at every tick — every
/// field of the runner, the figures, the missiles, the cues and the cries — and
/// by the saved game's bytes and trailing hash at the end. That is strictly more
/// than the lockstep digest, which is a hand-written projection of the same
/// runner (`crates/l2-sim/tests/lockstep.rs`).
///
/// **A green ablation, and it is the finding.**
/// `Director` whose deletion turns this red, because `Director::listen` holds
/// `&Game` and cannot write to it: the guarantee is the type, and this test is
/// the tripwire for the day somebody changes the signature. Ablated the other
/// way instead — an extra `runner.step()` on one copy at tick 100 — it goes red
/// at tick 100.
#[test]
fn sound_does_not_change_the_battle() {
    let mut silent_layer = Audio::silent();
    let (heard, silent) = play_it_twice(&mut silent_layer);
    assert_eq!(l2_game::save::encode(&heard), l2_game::save::encode(&silent));
    // The listener had something to listen to, or the comparison is empty.
    let b = live(&heard);
    assert!(b.cries.len() >= 2, "cries: {:?}", b.cries);
    assert!(b.runner.sim.cues.loosed(l2_sim::WeaponClass::Bow) > 0, "{:?}", b.runner.sim.cues);
    // Missile units halt at range (Order_StopShortOfTarget 0x00497437), so the
    // proving siege may end its 4,000 ticks without a melee: any casualty will do.
    let melee: u32 = l2_sim::ALL_TROOPS.iter().map(|&t| b.runner.sim.cues.melee_casualties(t)).sum();
    let missile = b.runner.sim.cues.missile_deaths();
    assert!(melee + missile > 0, "{:?}", b.runner.sim.cues);
}

/// **The proving ground as a live battle**: `l2_sim::proving`'s siege, unpaused,
/// in a game on the battlefield screen.
fn staged_siege() -> (Game, Machine) {
    let runner = l2_sim::proving::deploy();
    let mut live = LiveBattle::new(runner, 0, 0, 0, Some(l2_sim::proving::LEVEL), 1, 1);
    live.paused = false;
    live.cam = (20, 25);
    let mut g = Game::new(5);
    g.prefs.tip_screens = false;
    g.player = 1;
    g.battle = Some(Box::new(live));
    (g, Machine::new(ScreenId::Battlefield))
}

/// One siege that pours oil, docks a tower, burns a bridge and the men on it
/// and bounces catapult shots off a wall four high, played twice from the same
/// timetable: once with a [`audio::Director`] listening after every tick, once
/// with nothing listening.
fn play_a_siege_twice(sound: &mut Audio) -> (Game, Game) {
    let a = Assets::placeholder();
    let (mut heard, mut hm) = staged_siege();
    let (mut silent, mut sm) = staged_siege();
    let mut director = audio::Director::new();
    for (g, m) in [(&mut heard, &mut hm), (&mut silent, &mut sm)] {
        send(m, g, &a, Event::Pointer { x: 240, y: 240 });
    }
    for t in 0..2_000 {
        for g in [&mut heard, &mut silent] {
            l2_sim::proving::orders(&mut g.battle.as_deref_mut().unwrap().runner);
        }
        tick(&mut hm, &mut heard, &a);
        director.listen(sound, &hm, &heard);
        tick(&mut sm, &mut silent, &a);
        assert!(heard.battle == silent.battle, "sound changed the siege at tick {t}");
    }
    (heard, silent)
}

/// **Sound does not change a siege that burns, tick for tick** — C166's proof,
/// extended to the six sites fire, oil, the tower and the high rampart added.
///
/// The same comparison as [`sound_does_not_change_the_battle`]: the whole
/// `LiveBattle` at every tick — every field of the runner, the missile array
/// with its fires and its stream, the battlefield with its burning cells and
/// its ramp — and the saved game's bytes at the end. The second half of the
/// assertion is what keeps the first from being about a quiet siege: every one
/// of the six occasions happened, so the listener was asked for all six.
#[test]
fn sound_does_not_change_a_siege_that_burns() {
    let mut silent_layer = Audio::silent();
    let (heard, silent) = play_a_siege_twice(&mut silent_layer);
    assert_eq!(l2_game::save::encode(&heard), l2_game::save::encode(&silent));
    let c = live(&heard).runner.sim.cues;
    assert_eq!(c.oil_poured(), 1, "{c:?}");
    assert_eq!(c.towers_docked(), 1, "{c:?}");
    assert!(c.bridges_fired() >= 1, "{c:?}");
    assert!(c.walls_missed() >= 1, "{c:?}");
    assert!(c.burn_deaths(SIDE_B) >= 1, "{c:?}");
    let asked = audio::battle_requests(&Cues::default(), &c);
    for want in [Request::Slot(3), Request::Slot(0x11), Request::File("dest_ind.wav"), Request::Slot(0x10), Request::Slot(0xc)] {
        assert!(asked.contains(&want), "{want:?} was never asked for: {asked:?}");
    }
    assert_eq!(live(&heard).conclusion, None, "the siege was still being fought");
}

// ------------------------------------------------------ against the install

/// The install's case-insensitive lookup, as `tests/audio_install.rs` does it.
pub(crate) fn find(dir: &Path, name: &str) -> Option<PathBuf> {
    let want = name.to_ascii_lowercase();
    std::fs::read_dir(dir).ok()?.filter_map(|e| e.ok()).find_map(|e| {
        let p = e.path();
        let n = p.file_name()?.to_str()?.to_ascii_lowercase();
        (n == want).then_some(p)
    })
}

/// **The same, with sound** — decoded and mixed from the
/// install, and the assertion that sound was made is what keeps the equality
/// from being about a silence.
#[test]
fn sound_that_plays_does_not_change_the_battle() {
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install, so nothing to play");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let mut sound = Audio::headless(&platform.vfs);
    let (heard, silent) = play_it_twice(&mut sound);
    assert_eq!(l2_game::save::encode(&heard), l2_game::save::encode(&silent));
    let played = sound.heard();
    for want in ["swor_u2.wav", "bowmen1.wav", "crossbow.wav"] {
        assert!(played.contains(&want), "{want} was never played: {played:?}");
    }
}

/// **The same siege with sound** — and the four files the
/// siege added are among what was opened: the pour, the dock, the bridge and
/// the shot off the high wall, with a burning man's death cry beside them.
#[test]
fn sound_that_plays_does_not_change_a_siege_that_burns() {
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install, so nothing to play");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let mut sound = Audio::headless(&platform.vfs);
    let (heard, silent) = play_a_siege_twice(&mut sound);
    assert_eq!(l2_game::save::encode(&heard), l2_game::save::encode(&silent));
    let played = sound.heard();
    for want in ["pouroil.wav", "siegedoc.wav", "dest_ind.wav", "catmiss.wav", "deadguy3.wav"] {
        assert!(played.contains(&want), "{want} was never played: {played:?}");
    }
}

