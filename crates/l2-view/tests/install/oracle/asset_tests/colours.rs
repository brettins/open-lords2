#![allow(unused_imports)]
use super::*;
use super::icons::*;
use super::super::*;
use super::super::campaign_tests::*;
use super::super::*;
use super::super::corpus::*;
use super::super::render::*;
use std::{fs, path::{Path, PathBuf}};
use l2_sim::runner::{self as battle, BattleRunner};
use l2_sim::terrain;
use l2_sim::{Troop, SIDE_A, SIDE_B};
use l2_view::campaign;
use l2_view::canvas::Canvas;
use l2_view::chrome;
use l2_view::figures::{self, Anim, Colour};
use l2_view::scene::{self, BattleAssets, Camera};
use l2_view::sheet::Sheet;

/// `l2_view::chrome::realm_pen` *derives* the pen from the shield
/// reading realm `+0x08` out of the save, because both writers in the binary
/// derive it the same way and nothing else touches the field
/// (`Realms_AssignLords` at new game, `FUN_0042BA40` for a custom battle). This
/// is that claim tested against data: for every realm of every fixture, the
/// byte the game stored at `+0x08` must equal our table indexed by `+0x0A`.
#[test]
fn every_saved_realms_stored_pen_is_its_shields_pen() {
    let Some(dir) = l2_testkit::fixtures_dir() else {
        l2_testkit::skip!("LORDS2_FIXTURES not set - skipping");
    };
    let Some(install) = asset_dir() else {
        l2_testkit::skip!("LORDS2_DIR not set - the save schema comes from the executable");
    };
    let Some(exe) = read(&install, "Lords2.exe") else {
        l2_testkit::skip!("Lords2.exe not present - skipping");
    };

    let mut realms_checked = 0;
    let mut files_checked = 0;
    let mut separating = 0;
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .expect("the fixture directory")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.to_ascii_lowercase().ends_with(".sav"))
        .collect();
    names.sort();
    for name in &names {
        let Ok(bytes) = std::fs::read(dir.join(name)) else { continue };
        let save = l2_formats::save::Save::open(&exe, &bytes)
            .unwrap_or_else(|e| panic!("{name}: {e:?}"));
        files_checked += 1;
        for index in 1..l2_formats::save::REALM_RECORDS {
            let Ok(realm) = save.realm(index) else { continue };
            if realm.shield_index == 0 {
                continue;
            }
            let stored = save
                .u8_at(
                    l2_formats::save::REALM_BASE
                        + (index * l2_formats::save::REALM_STRIDE) as u32
                        + 0x08,
                )
                .expect("realm +0x08 is inside the realm block");
            assert_eq!(
                Some(stored),
                chrome::realm_pen(realm.shield_index),
                "{name}: realm {index} flies shield {} and stored pen {stored:#04X}",
                realm.shield_index
            );
            if index as u8 != realm.shield_index {
                separating += 1;
            }
            realms_checked += 1;
        }
    }
    assert!(files_checked >= 1, "no fixture saves were readable");
    assert!(
        separating >= 2,
        "no fixture has a realm whose id differs from its shield, so this cannot tell the \
         two keys apart - it passed for the wrong reason"
    );
    eprintln!(
        "realm pens: {realms_checked} realms across {files_checked} saves, {separating} of them \
         with id != shield"
    );
}

/// * **`g_lordChoice`** (`0x004DC17C`) is per **colour**, four candidate lords
///   per shield. `Realms_AssignLords` hands out the lowest free shield by realm
///   order and then picks the lord *from the shield*, so no lord has a
///   preference on the campaign path — the appearance of one is this table's
///   first column.
///
/// * **`g_battleLordShield`** (`0x004D4CA8`) is per **lord**, `{preferred,
///   alternate}`, and really is preference-then-fallback. It belongs to the
///   custom battle (`FUN_0042BA40`) and nothing else.
#[test]
fn the_lord_colour_tables_are_the_bytes_in_the_binary() {
    let Some(dir) = asset_dir() else {
        l2_testkit::skip!("LORDS2_DIR not set - skipping");
    };
    let Some(exe) = read(&dir, "Lords2.exe") else {
        l2_testkit::skip!("Lords2.exe not present - skipping");
    };
    let u32_at = |va: u32| -> u32 {
        let o = va_to_offset(&exe, va).unwrap_or_else(|| panic!("{va:#010X} unmapped"));
        u32::from_le_bytes(exe[o..o + 4].try_into().unwrap())
    };
    let u8_at = |va: u32| -> u8 {
        exe[va_to_offset(&exe, va).unwrap_or_else(|| panic!("{va:#010X} unmapped"))]
    };

    // Knight, Baron, Countess, Bishop are lord ids 1 … 4 (`L2.eng` group 7).
    const BATTLE_LORD_SHIELD_VA: u32 = 0x004D_4CA8;
    let pair = |lord: u32| {
        (u32_at(BATTLE_LORD_SHIELD_VA + lord * 8), u32_at(BATTLE_LORD_SHIELD_VA + lord * 8 + 4))
    };
    assert_eq!(pair(1), (2, 4), "the Knight prefers yellow, falling back to magenta");
    assert_eq!(pair(2), (1, 5), "the Baron prefers red, falling back to blue");
    assert_eq!(pair(3), (5, 1), "the Countess prefers blue, falling back to red");
    assert_eq!(pair(4), (4, 2), "the Bishop prefers magenta, falling back to yellow");
    // Three of the four first columns are the player's own words. The Baron is
    // the one he could not remember, and in *this* table it is red — while the
    // campaign gives him black, which is the whole point of §7a.
    assert_ne!(pair(2).0, 3, "the Baron's battle preference is not the campaign's black");

    const LORD_CHOICE_VA: u32 = 0x004D_C17C;
    let candidates = |group: u32, shield: u32| -> [u8; 4] {
        let base = LORD_CHOICE_VA + group * 0x14 + shield * 4;
        [u8_at(base), u8_at(base + 1), u8_at(base + 2), u8_at(base + 3)]
    };
    // England is slot 0, so group 0 — the arrangement every default game shows.
    assert_eq!(candidates(0, 2), [1, 3, 4, 2], "yellow leads with the Knight");
    assert_eq!(candidates(0, 3), [2, 1, 3, 4], "black leads with the Baron");
    assert_eq!(candidates(0, 4), [4, 3, 2, 1], "magenta leads with the Bishop");
    assert_eq!(candidates(0, 5), [3, 2, 4, 1], "blue leads with the Countess");
    assert_eq!(candidates(0, 1), [2, 1, 3, 4], "red leads with the Baron");

    for group in 0..3 {
        assert_eq!(
            candidates(group, 5),
            candidates(group + 1, 0),
            "group {group}'s slot 5 should be group {}'s slot 0",
            group + 1
        );
    }
    // …and group 3's overrun lands on `g_realmColour`'s own base, overlapping
    // it by two bytes. That is what the two bytes below the pen table are, and
    // `the_realm_pen_table_matches_the_bytes_in_the_binary` asserts their
    // values from the other side.
    let overrun = LORD_CHOICE_VA + 3 * 0x14 + 5 * 4;
    assert_eq!(overrun, 0x004D_C1CC);
    assert_eq!(overrun + 2, chrome::REALM_PEN_VA - 2, "the two tables overlap by two bytes");
    assert_eq!(candidates(3, 5)[2..], [0x04, 0x02], "and those two bytes are shared");

    eprintln!(
        "lord colours: g_lordChoice is per-colour, g_battleLordShield per-lord; \
         the two tables overlap by two bytes at {:#010X}",
        overrun + 2
    );
}

#[test]
fn taking_a_middle_colour_moves_the_lords_and_not_only_their_colours() {
    let Some(dir) = asset_dir() else {
        l2_testkit::skip!("LORDS2_DIR not set - skipping");
    };
    let Some(exe) = read(&dir, "Lords2.exe") else {
        l2_testkit::skip!("Lords2.exe not present - skipping");
    };
    let u8_at = |va: u32| -> u8 {
        exe[va_to_offset(&exe, va).unwrap_or_else(|| panic!("{va:#010X} unmapped"))]
    };

    let assign = |human_shield: u8| -> Vec<(u8, u8)> {
        let mut shield_taken = [false; 6];
        shield_taken[human_shield as usize] = true;
        let mut lord_taken = [false; 8];
        let mut out = Vec::new();
        for _realm in 2..=5u8 {
            let shield = (1..=5u8).find(|s| !shield_taken[*s as usize]).expect("a free shield");
            shield_taken[shield as usize] = true;
            // Then the lord, from that shield. England is slot 0 -> group 0.
            let base = 0x004D_C17C + u32::from(shield) * 4;
            let lord = (0..4)
                .map(|n| u8_at(base + n))
                .find(|&c| c != 0 && !lord_taken[c as usize & 7])
                .unwrap_or(0);
            lord_taken[lord as usize & 7] = true;
            out.push((shield, lord));
        }
        out
    };

    const KNIGHT: u8 = 1;
    const BARON: u8 = 2;
    const COUNTESS: u8 = 3;
    const BISHOP: u8 = 4;

    assert_eq!(
        assign(1),
        vec![(2, KNIGHT), (3, BARON), (4, BISHOP), (5, COUNTESS)],
        "taking red gives the arrangement every default game shows"
    );

    assert_eq!(
        assign(2),
        vec![(1, BARON), (3, KNIGHT), (4, BISHOP), (5, COUNTESS)],
        "taking yellow makes the Baron red and the Knight black"
    );

    assert_eq!(assign(3), vec![(1, BARON), (2, KNIGHT), (4, BISHOP), (5, COUNTESS)]);
    assert_eq!(
        assign(4),
        vec![(1, BARON), (2, KNIGHT), (3, COUNTESS), (5, BISHOP)],
        "taking magenta moves the Countess to black and the Bishop to blue"
    );
    assert_eq!(assign(5), vec![(1, BARON), (2, KNIGHT), (3, COUNTESS), (4, BISHOP)]);

    let knight_yellow = (1..=5u8)
        .filter(|&h| assign(h).iter().any(|&(s, l)| l == KNIGHT && s == 2))
        .count();
    assert_eq!(knight_yellow, 4, "true four times in five, which is why it reads as a rule");
    assert!(
        !assign(2).iter().any(|&(s, l)| l == KNIGHT && s == 2),
        "and false exactly where the person took it"
    );
    eprintln!("lord colours: the Knight has yellow in {knight_yellow} of 5 openings");

}



