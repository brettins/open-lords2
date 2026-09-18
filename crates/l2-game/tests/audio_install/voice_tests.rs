#![allow(unused_imports)]
use super::*;
use super::track_tests::*;
use super::format_tests::*;
use std::path::{Path, PathBuf};
use l2_game::audio::{names, track, wav};

#[test]
fn the_two_sample_banks_ship_apart_from_the_hole_in_both() {
    let dir = l2_testkit::install!();
    for (label, bank) in
        [("kingdom", &names::KINGDOM_BANK[..]), ("battle", &names::BATTLE_BANK[..])]
    {
        for (i, name) in bank.iter().enumerate() {
            if *name == "null.wav" {
                assert!(find(&dir, name).is_none(), "null.wav actually exists?");
                continue;
            }
            assert!(find(&dir, name).is_some(), "{label} bank slot {i} ({name}) is missing");
        }
    }
}

#[test]
fn all_448_lord_voices_exist_and_the_convention_generates_them() {
    let dir = l2_testkit::install!();
    let mut found = 0;
    for group in 170..=197u16 {
        for variant in 0..16u8 {
            let name = names::lord_voice(group, variant).expect("in range");
            assert!(find(&dir, &name).is_some(), "{name} is missing");
            found += 1;
        }
    }
    assert_eq!(found, 448);
}

#[test]
fn the_message_fanfares_ship() {
    let dir = l2_testkit::install!();
    for name in [
        names::fanfare::MESSAGE,
        names::fanfare::CAPTURED,
        names::fanfare::BATTLE,
        names::fanfare::AFTER_BATTLE,
    ] {
        assert!(find(&dir, name).is_some(), "{name} is missing");
    }
    assert!(find(&dir, "ff_win.wav").is_some(), "ff_win.wav ships even so");
}

///: only twelve mercenary nationalities exist, so
#[test]
fn the_screen_voices_ship_over_the_range_a_running_game_can_reach() {
    let dir = l2_testkit::install!();
    assert_eq!(l2_kingdom::mercenary::ROSTER[1].nationality, "Scottish");
    assert_eq!(names::speech::MERCENARY_OFFER[0], "S016_01.wav");
    for name in &names::speech::MERCENARY_OFFER[..l2_kingdom::mercenary::ROSTER.len() - 1] {
        assert!(find(&dir, name).is_some(), "{name} is missing");
    }
    assert!(
        find(&dir, names::speech::MERCENARY_OFFER[12]).is_none(),
        "the table's last four entries name files the install does not have",
    );
    for name in &names::speech::POPULATION_HEALTH[..5] {
        assert!(find(&dir, name).is_some(), "{name} is missing");
    }
    assert_eq!(
        names::speech::POPULATION_HEALTH[3], names::speech::POPULATION_HEALTH[4],
        "the two healthiest bands share one clip at 0x004E2058, and generating the name \
         instead of reading the bytes is what would silently swap it for S020_05",
    );
    for name in names::speech::PICKED_UNIT.iter().chain(names::speech::PICKED_CASTLE.iter()) {
        assert!(find(&dir, name).is_some(), "{name} is missing");
    }
    assert!(find(&dir, "S071_01.wav").is_none(), "S071_01.wav does not ship");
}

