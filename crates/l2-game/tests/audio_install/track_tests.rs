#![allow(unused_imports)]
use super::*;
use super::voice_tests::*;
use super::format_tests::*;
use std::path::{Path, PathBuf};
use l2_game::audio::{names, track, wav};

#[test]
fn every_track_the_ladder_can_choose_is_in_the_install() {
    let dir = l2_testkit::install!();
    for share in [0, 7, 8, 14, 15, 28, 29, 42, 43, 100] {
        let m = track::campaign(4, share);
        assert!(find(&dir, m.file()).is_some(), "{share}% -> {} is missing", m.file());
    }
    let mut cycle = track::BattleCycle::default();
    for _ in 0..4 {
        for kind in [track::BattleKind::Field, track::BattleKind::Siege] {
            let m = cycle.next(kind);
            assert!(find(&dir, m.file()).is_some(), "{:?} -> {} is missing", kind, m.file());
        }
    }
}

#[test]
fn england_turn_one_would_play_scroll1() {
    let save = l2_testkit::england!();
    let game = l2_game::scenario::from_save(&save, l2_kingdom::tables::Tables::DEFAULT)
        .expect("the fixture loads");
    let realm = &game.kingdom.realms[game.player as usize];
    assert_eq!(realm.county_count, 1, "the England start is one county");
    assert_eq!(game.kingdom.county_count, 14);

    // `docs/decisions.md` C116.
    let machine = l2_game::screen::Machine::new(l2_game::screen::ScreenId::Campaign);
    let scene = l2_game::audio::scene(&machine, &game);
    assert_eq!(
        scene,
        l2_game::audio::Scene::Campaign { county_count: 1, share_of_map_pct: 7 },
        "PctOf(1, 14)"
    );
    assert_eq!(track::campaign(1, 7), track::Music::Scroll(1));

    assert_eq!(realm.share_of_map_pct, 0, "the derived field is not populated on import");

    let front = l2_game::screen::Machine::new(l2_game::screen::ScreenId::Setup(
        l2_game::screens::setup::SetupPage::Title,
    ));
    assert_eq!(l2_game::audio::scene(&front, &game), l2_game::audio::Scene::FrontEnd);
}

#[test]
fn a_real_track_comes_out_of_the_mixer_audible() {
    let dir = l2_testkit::install!();
    let path = find(&dir, "scroll1.wav").expect("scroll1.wav");
    let sound = wav::decode(&std::fs::read(&path).unwrap()).expect("decodes");
    let seconds = sound.frames() as f64 / sound.rate as f64;
    assert!((100.0..300.0).contains(&seconds), "scroll1 is {seconds:.0}s");

    let mut m = l2_game::audio::mixer::Mixer::new(48_000);
    m.set_music("scroll1.wav".into(), std::sync::Arc::new(sound));
    assert_eq!(m.music_name(), Some("scroll1.wav"));

    let mut buf = vec![0f32; 48_000 / 10 * 2];
    m.fill(&mut buf);
    assert!(buf.iter().all(|s| s.is_finite() && (-1.0..=1.0).contains(s)), "out of range");
    let peak = buf.iter().fold(0f32, |a, b| a.max(b.abs()));
    assert!(peak > 0.001, "the first tenth of a second is silent (peak {peak})");

    for _ in 0..600 {
        m.fill(&mut buf);
    }
    let peak = buf.iter().fold(0f32, |a, b| a.max(b.abs()));
    assert!(peak > 0.001, "went silent partway through (peak {peak})");
}

