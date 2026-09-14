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
                // Slot 1 of both banks. `null.wav` is not in the install and
                //
                // this is how it spells an unused slot.
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
    // 28 groups x 4 lords x 4 takes. If the convention were wrong this would
    // fail on the first file, but the count is the claim.
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
    // The finding this test exists to keep: `Ff_win.wav` ships and
    // `Battle_ReturnToCampaign` plays `ff_lose.wav` at both of its two sites.
    // The file is here; nothing in `Lords2.exe` names it.
    assert!(find(&dir, "ff_win.wav").is_some(), "ff_win.wav ships even so");
}

/// **The four screen-voice tables, and the one entry that is
/// convention.**
///
/// `S016_*`, `S020_*`, `S031_*` and `S071_*` are read out of `.data` as
/// `char[n][16]`, and this is why: **entries 3 and 4 of
/// the health table are the same file.** A `format!("S020_{:02}", band + 1)`
/// would have produced `S020_05.wav` for band 4 — a file that ships, and the
/// wrong line — so the defect would have been a health readout that speaks
/// somebody else's sentence and nothing would have gone red.
///
/// The tables over-allocate in the original's own way and that is asserted
///: only twelve mercenary nationalities exist, so
/// `S016_13` … `S016_16` name files that do not ship, and `S020_06` /
/// `S020_07` sit past the five bands `health_band` produces.
#[test]
fn the_screen_voices_ship_over_the_range_a_running_game_can_reach() {
    let dir = l2_testkit::install!();
    // `l2_kingdom::mercenary::ROSTER` has a dead slot 0 and twelve bands, so
    // `mercenaryOffer` is 1..=12 and the table is indexed by `offer - 1`.
    // Band 1 is the Scottish pikemen, which is the line that was reported
    // missing and is therefore `MERCENARY_OFFER[0]`.
    assert_eq!(l2_kingdom::mercenary::ROSTER[1].nationality, "Scottish");
    assert_eq!(names::speech::MERCENARY_OFFER[0], "S016_01.wav");
    for name in &names::speech::MERCENARY_OFFER[..l2_kingdom::mercenary::ROSTER.len() - 1] {
        assert!(find(&dir, name).is_some(), "{name} is missing");
    }
    assert!(
        find(&dir, names::speech::MERCENARY_OFFER[12]).is_none(),
        "the table's last four entries name files the install does not have",
    );
    // `healthBand` is 0..=4.
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
    // The table `PICKED_CASTLE` starts one past: `S071_01.wav` is named
    // nowhere in the binary and is not in the install either.
    assert!(find(&dir, "S071_01.wav").is_none(), "S071_01.wav does not ship");
}

