//! **The economy tables, against the bytes in `Lords2.exe`.**
//!
//! ```text
//! LORDS2_DIR="F:\games\Lords of the Realm II" cargo test -p l2-kingdom --test oracle
//! ```
//!
//! # Why this file exists
//!
//! `crates/l2-kingdom/src/tables.rs` is 1,200 lines of hand-transcribed
//! literals, each with an address in its doc comment and none with a check. The
//! addresses were read once, by hand, out of a decompiler; the numbers were
//! typed once, by hand, into Rust. `tools/oracle/kingdom.ps1` reads the same
//! tables out of the executable and **prints them to a console**, where nothing
//! compares them to anything.
//!
//! `docs/decisions.md` C10 (the manual is wrong twice), C12 (a test that cannot
//! fail), C17 (the answer was in the code and we looked at the data) and C20 (a
//! test named after a file it never opened) are four instances of one
//! mechanism: **nothing re-checks.** A transcription slip in this file would be
//! invisible until a save failed to reproduce, and most of these tables are not
//! exercised by the England turn-one position at all — every county on it is on
//! Normal rations at health band 3, so 25 of the 30 entries in
//! `g_healthDeltaTable` have never been read by any test.
//!
//! # What makes this different from the reproduction test
//!
//! The reproduction checks that our *rules* land on a real save's numbers, which
//! is strong evidence about the rules that the save exercises. This checks that
//! our *tables* are the executable's, including every row the save never
//! touches. They fail in different ways and neither replaces the other.
//!
//! Two addresses are asserted by arithmetic as well as by content, because
//! `tools/oracle/kingdom.ps1`'s comments pin several table *lengths* that way:
//! `0x004D6520 + 40` is exactly where `g_healthHappiness` begins, so the health
//! band ladder is five pairs and not six.

// The loops below index by `[rationLevel][healthBand]` on purpose: those are the
// table's own coordinates and a failure message that names them is the point.
#![allow(clippy::needless_range_loop)]

use l2_kingdom::field::{FIELD_BRUSHES, WASTE_BRUSHES};
use l2_kingdom::labour::SHARE_TABLE;
use l2_kingdom::tables::{
    Tables, BIRTH_RATE_LADDER, CASTLE_FREE_ARCHERS, CASTLE_GARRISON_CAP, DEATH_RATE_BY_HEALTH,
    GOOD_SELL_PRICE, HEALTH_BAND_LADDER, HEALTH_DELTA, HEALTH_HAPPINESS, HERD_WEATHER_PCT,
    RATION_TABLE, TAX_HAPPINESS_OTHER, WEAPON_COST,
};
use l2_testkit::pe::Table;

const HEALTH_BAND_LADDER_VA: u32 = 0x004D_6520;
const HEALTH_HAPPINESS_VA: u32 = 0x004D_6548;
const HEALTH_DELTA_VA: u32 = 0x004D_64A8;
const RATION_TABLE_VA: u32 = 0x004D_6738;
const BIRTH_RATE_LADDER_VA: u32 = 0x004D_6308;
const HERD_WEATHER_PCT_VA: u32 = 0x004D_6560;
const TAX_HAPPINESS_OTHER_VA: u32 = 0x004D_63D8;
const CASTLE_GARRISON_CAP_VA: u32 = 0x004D_8A10;
const CASTLE_FREE_ARCHERS_VA: u32 = 0x004D_8A40;
const WEAPON_COST_VA: u32 = 0x004D_8990;
const GOOD_SELL_PRICE_VA: u32 = 0x004D_8910;

/// `g_healthBandLadder` — **five** `{inclusive upper bound, band}` pairs, not
/// four. The top band is an explicit `(100, 4)` entry rather than an `else`.
///
/// The count is fixed by arithmetic and not by belief: five pairs is forty
/// bytes, and `0x004D6520 + 40` is exactly `0x004D6548`, where
/// `g_healthHappiness` begins — asserted below, and again by that table's own
/// content.
#[test]
fn the_health_ladder_and_its_length_are_the_bytes_in_the_executable() {
    let exe = l2_testkit::executable!();
    let t = Table::at(&exe, HEALTH_BAND_LADDER_VA);
    let read: Vec<(i32, i32)> = (0..5).map(|i| (t.i32_at(i * 2), t.i32_at(i * 2 + 1))).collect();
    assert_eq!(read, HEALTH_BAND_LADDER.to_vec(), "g_healthBandLadder");
    assert_eq!(
        HEALTH_BAND_LADDER_VA + 5 * 2 * 4,
        HEALTH_HAPPINESS_VA,
        "five pairs is forty bytes, and forty bytes is where the next table starts"
    );

    let h = Table::at(&exe, HEALTH_HAPPINESS_VA);
    assert_eq!(h.i32s(5), HEALTH_HAPPINESS.to_vec(), "g_healthHappiness");
    assert_eq!(h.i32_at(5), 0, "the sixth slot is the pad that pins the length");
}

/// `g_healthDeltaTable` — `int[6][5]`, indexed `[rationLevel][healthBand]`.
///
/// **Twenty-five of these thirty entries are never read by any other test**,
/// because every county in the England turn-one position sits on Normal
/// rations at band 3. This is the only thing in the suite that would notice a
/// transposed row.
#[test]
fn the_health_delta_table_is_the_bytes_in_the_executable() {
    let exe = l2_testkit::executable!();
    let t = Table::at(&exe, HEALTH_DELTA_VA);
    for level in 0..6 {
        for band in 0..5 {
            assert_eq!(
                HEALTH_DELTA[level][band],
                t.i32_at(level * 5 + band),
                "g_healthDeltaTable[{level}][{band}]"
            );
        }
    }
    // Thirty ints is 120 bytes and `0x004D64A8 + 120` is `0x004D6520`, where the
    // band ladder starts. That is what fixes the table at six rows rather than
    // five or seven, and it is asserted rather than remembered.
    assert_eq!(HEALTH_DELTA_VA + 6 * 5 * 4, HEALTH_BAND_LADDER_VA);
    eprintln!("g_healthDeltaTable: 30 values match the executable");
}

/// `g_rationTable` — six `{divisor, multiplier}` pairs, and
/// `g_birthRateLadder` — twenty `{population, percent}` pairs.
#[test]
fn the_ration_and_birth_ladders_are_the_bytes_in_the_executable() {
    let exe = l2_testkit::executable!();

    let r = Table::at(&exe, RATION_TABLE_VA);
    let read: Vec<(i32, i32)> = (0..6).map(|i| (r.i32_at(i * 2), r.i32_at(i * 2 + 1))).collect();
    assert_eq!(read, RATION_TABLE.to_vec(), "g_rationTable");

    let b = Table::at(&exe, BIRTH_RATE_LADDER_VA);
    let read: Vec<(i32, i32)> = (0..20).map(|i| (b.i32_at(i * 2), b.i32_at(i * 2 + 1))).collect();
    assert_eq!(read, BIRTH_RATE_LADDER.to_vec(), "g_birthRateLadder");

    // A ladder that is not monotone would be read wrong by any search that
    // stops at the first match, which is what the rule does.
    for w in BIRTH_RATE_LADDER.windows(2) {
        assert!(w[0].0 <= w[1].0, "the birth ladder is not sorted by population");
    }
    eprintln!("g_rationTable and g_birthRateLadder match the executable");
}

/// The single-array tables: the weather's swing on the herd, the tax happiness
/// curve, the two castle columns, the weapon costs and the merchant prices.
#[test]
fn the_flat_economy_tables_are_the_bytes_in_the_executable() {
    let exe = l2_testkit::executable!();

    assert_eq!(
        Table::at(&exe, HERD_WEATHER_PCT_VA).i32s(6),
        HERD_WEATHER_PCT.to_vec(),
        "g_herdWeatherPct"
    );
    assert_eq!(
        Table::at(&exe, TAX_HAPPINESS_OTHER_VA).i32s(TAX_HAPPINESS_OTHER.len()),
        TAX_HAPPINESS_OTHER.to_vec(),
        "g_taxHappinessOther"
    );
    assert_eq!(
        Table::at(&exe, CASTLE_GARRISON_CAP_VA).i32s(6),
        CASTLE_GARRISON_CAP.to_vec(),
        "g_castleGarrisonCap"
    );
    assert_eq!(
        Table::at(&exe, CASTLE_FREE_ARCHERS_VA).i32s(6),
        CASTLE_FREE_ARCHERS.to_vec(),
        "g_castleFreeArchers"
    );
    assert_eq!(
        Table::at(&exe, GOOD_SELL_PRICE_VA).i32s(GOOD_SELL_PRICE.len()),
        GOOD_SELL_PRICE.to_vec(),
        "g_goodSellPrice"
    );

    let w = Table::at(&exe, WEAPON_COST_VA);
    let read: Vec<(i32, i32)> =
        (0..WEAPON_COST.len()).map(|i| (w.i32_at(i * 2), w.i32_at(i * 2 + 1))).collect();
    assert_eq!(read, WEAPON_COST.to_vec(), "g_weaponCost");

    // `g_deathRateByHealth` is not a table in the image - it is written by
    // Rules_InitConstants and lives in the instruction stream (C16), which
    // tools/oracle/initconsts.ps1 recovers. Named here so its absence from this
    // file reads as deliberate rather than as an oversight.
    assert_eq!(DEATH_RATE_BY_HEALTH.len(), 5);
    eprintln!("six flat economy tables match the executable");
}

/// **The ruleset a mod overrides is the same data these tables hold.**
///
/// `Tables::DEFAULT` is what the simulation actually runs on;
/// `tables::HEALTH_DELTA` and friends are what this file compares against the
/// binary. If the two ever came apart the oracle above would be checking
/// something the engine does not use — which is the same failure as checking a
/// constant against a second copy of itself.
#[test]
fn the_default_ruleset_is_built_from_the_tables_the_oracle_checked() {
    let t = &Tables::DEFAULT;
    for level in 0..6 {
        assert_eq!(t.ration[level].health_delta.to_vec(), HEALTH_DELTA[level].to_vec());
    }
    for band in 0..5 {
        assert_eq!(t.health[band].happiness, HEALTH_HAPPINESS[band]);
    }
    assert_eq!(t.health_band_ladder.to_vec(), HEALTH_BAND_LADDER.to_vec());
    assert_eq!(t.tax_happiness_other.to_vec(), TAX_HAPPINESS_OTHER.to_vec());
    for (i, &pct) in HERD_WEATHER_PCT.iter().enumerate() {
        assert_eq!(t.weather[i].herd_pct, pct, "weather {i}");
    }
}

// ---------------------------------------------------------------------------
// The field brush
// ---------------------------------------------------------------------------

/// The two hotspot tables `Map_Click` puts up when a field is clicked, and the
/// share table `Labour_ToggleShare` indexes.
const FIELD_BRUSH_MENU_VA: u32 = 0x004D_C4D0;
const WASTE_BRUSH_MENU_VA: u32 = 0x004D_C530;
const SHARE_TABLE_VA: u32 = 0x004D_6768;

/// `Hotspot_Test` walks 24-byte records: `{x0, y0, x1, y1}` as `i16`, a
/// callback at `+8`, an id at `+0x10` and an argument at `+0x14`.
const HOTSPOT_RECORD: usize = 24;
/// Every field brush button calls `FUN_00438B02`, which is the only caller of
/// `Field_SetType` a player can reach.
const FIELD_BRUSH_HANDLER: i32 = 0x0043_8B02;

/// **The five field brushes are the executable's, not ours.**
///
/// `docs/screens-county.md` §9 guessed that county `+0x130`/`+0x134`/`+0x138`
/// were the field brush; they are the first three of the eight job percentages,
/// and the brush is these two hotspot tables. Reading them settles it with
/// bytes rather than with a story (`docs/decisions.md` C3), and it fixes the
/// terrain value of each button, which `l2_kingdom::field::FieldType::brush`
/// has to agree with.
#[test]
fn the_field_brush_is_five_buttons_in_two_menus_and_they_are_the_ones_we_paint() {
    let exe = l2_testkit::executable!();

    let read = |va: u32, count: usize| -> Vec<i32> {
        let t = Table::at(&exe, va);
        for i in 0..count {
            assert_eq!(
                t.i32_at(i * HOTSPOT_RECORD / 4 + 2),
                FIELD_BRUSH_HANDLER,
                "{va:#010x} record {i} calls something other than FUN_00438B02"
            );
        }
        (0..count).map(|i| t.i32_at(i * HOTSPOT_RECORD / 4 + 4)).collect()
    };

    assert_eq!(
        read(FIELD_BRUSH_MENU_VA, FIELD_BRUSHES.len()),
        FIELD_BRUSHES.iter().map(|b| b.brush() as i32).collect::<Vec<_>>(),
        "the three buttons offered on a field: fallow, grain, pasture"
    );
    assert_eq!(
        read(WASTE_BRUSH_MENU_VA, WASTE_BRUSHES.len()),
        WASTE_BRUSHES.iter().map(|b| b.brush() as i32).collect::<Vec<_>>(),
        "the two offered on waste: begin reclaiming, or abandon"
    );

    // All five are 48 x 48 on one row, which is what makes them a popup rather
    // than five unrelated hotspots — and is why `FUN_00438990` can choose
    // between the tables on the tile's terrain alone.
    for (va, count) in [(FIELD_BRUSH_MENU_VA, 3usize), (WASTE_BRUSH_MENU_VA, 2)] {
        let t = Table::at(&exe, va);
        for i in 0..count {
            let half = i * HOTSPOT_RECORD / 2;
            let (x0, y0) = (t.u16_at(half) as i32, t.u16_at(half + 1) as i32);
            let (x1, y1) = (t.u16_at(half + 2) as i32, t.u16_at(half + 3) as i32);
            assert_eq!((x1 - x0, y1 - y0), (48, 48), "{va:#010x} record {i}");
            assert_eq!((y0, y1), (184, 232), "{va:#010x} record {i} is off the row");
        }
    }
}

/// `g_shareTable` (`0x004D6768`) — `100 / (n + 1)` for the first five entries,
/// which is how a job joining the farm split gets an even share.
#[test]
fn the_share_table_is_the_executables_and_its_head_is_a_hundred_over_n_plus_one() {
    let exe = l2_testkit::executable!();
    assert_eq!(Table::at(&exe, SHARE_TABLE_VA).i32s(SHARE_TABLE.len()), SHARE_TABLE.to_vec());
    for n in 0..5 {
        assert_eq!(SHARE_TABLE[n], 100 / (n as i32 + 1), "entry {n}");
    }
    // Entries 5..8 are `{0, 5, 0}` and are **not** that sequence. Nothing can
    // index them: `Labour_ToggleShare` counts non-zero shares among three jobs,
    // so `n` is at most 2 when it is called. Asserted so that a reader who
    // notices the break knows it was looked at.
    assert_eq!(&SHARE_TABLE[5..], &[0, 5, 0]);
}
