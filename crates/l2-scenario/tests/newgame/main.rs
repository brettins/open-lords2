//! **The two constructors, held against each other.**
//!
//! ```text
//! LORDS2_DIR="F:\games\Lords of the Realm II" \
//! LORDS2_FIXTURES="E:\dev\lords2-fixtures" cargo test -p l2-scenario --test newgame
//! ```
//!
//! `Scenario::from_save` reads a world the original built. `Scenario::from_map`
//! builds one. They are two independent readings of the *same* England, out of
//! two files that were authored separately — `L2_maps.dat` and a save — so
//! where they disagree, one of them is wrong; and where they agree by both
//! being silent, **both** may be wrong.
//!
//! That last case is the one this file exists for. `docs/agents.md`:
//!
//! > A field is only tested if something a test reads was written by something
//! > the game runs.
//!
//! A second constructor is the strongest available witness for the first, and
//! the trap it walks straight into is agreement-by-omission —
//! `CLAUDE.md`'s *"two of our own implementations agreeing proves only that we
//! ported our own misunderstanding faithfully"*, one level down. So the diff
//! below reports three verdicts, not two: **agree**, **differ, because…**, and
//! **both silent**. And
//! [`every_county_field_is_accounted_for`] reads the field list of
//! `CountyState` **out of the source**, so a field added tomorrow fails this
//! test until somebody says which verdict it takes.

use l2_formats::maps::{MapSet, Plane, PLANE_DIM, SLOT_LEN};
use l2_kingdom::map::MAP_TILES;
use l2_scenario::newgame::{self, MapError, NewGame};
use l2_scenario::{CountyState, Scenario};

/// `L2_maps.dat`, or skip. Its own gate
/// because a partial install can have the executable and not the maps.
macro_rules! maps {
    () => {
        match l2_testkit::read_install("L2_maps.dat") {
            Some(b) => b,
            None => l2_testkit::skip!("no L2_maps.dat in the install"),
        }
    };
}

mod build;
pub use build::*;
mod comparison;
pub use comparison::*;


/// Slot 0 is England — `L2.eng` group 101 names it, and rendering it produces
/// England and Wales (`docs/formats/maps-layers.md` §0).
const ENGLAND: usize = 0;

/// The five England start counties. `docs/decisions.md` C23: the **set** is
/// fixed by the map file — the five castle blocks carrying a plane-4 marker —
/// and only the realm→county assignment is rolled, by `FUN_00497E65`.
const ENGLAND_STARTS: [u8; 5] = [1, 4, 8, 11, 13];

/// The seed both kingdoms are built on. Nothing in either construction path
/// draws from it except the start-table deal, which is compared by its result.
const SEED: u64 = 7;

// ------------------------------------------------------- the whole map corpus

/// One field's verdict when the two constructors are asked the same question.
#[derive(Debug, PartialEq, Eq)]
enum Verdict {
    /// The two paths produce the same value, and it is not the type's default —
    /// so something wrote it on both sides.
    Agree,
    /// They differ, and the reason is named.
    Differ(&'static str),
    /// **They agree on the type's default**, which is not evidence of anything.
    BothSilent(&'static str),
}

/// A difference nobody has explained. Written as a reason so that an
/// unlabelled call is impossible to miss in the report.
const UNEXPLAINED: &str = "UNEXPLAINED";

fn england_pair(save: &l2_formats::save::Save, maps: &[u8]) -> (Scenario, Scenario) {
    let from_save = Scenario::from_save(save).expect("the fixture imports");
    let set = MapSet::parse(maps).expect("L2_maps.dat parses");
    let slot = set.slot(ENGLAND).expect("slot 0");
    // The map path is told the **same** rule options the save recorded — the
    // difficulty in particular, which is the whole of the setting's effect on
    // the land — so the only thing left to disagree about is the world.
    let setup = NewGame {
        slot: ENGLAND,
        lords: 5,
        local_player: from_save.local_player,
        // **The colour off the save, not a literal 1.** The map path now takes
        // the person's chosen shield and it moves every AI's colour and lord
        // with it, so handing it a guess would make the comparison below a
        // comparison with a different game. The fixture happens to hold 1;
        // reading it means a regenerated fixture that holds something else
        // fails honestly instead of silently.
        shield: from_save.realms[from_save.local_player as usize].shield_index,
        seed: SEED,
        options: from_save.options,
    };
    let from_map = Scenario::from_map(&slot, &setup).expect("England builds");
    (from_save, from_map)
}

/// Every field of [`CountyState`] the diff above judges, by name.
const JUDGED: &[&str] = &[
    "owner",
    "population",
    "population_last",
    "happiness",
    "happiness_last",
    "shown_tax",
    "shown_ration",
    "shown_health",
    "shown_events",
    "d_hap_ration",
    "d_hap_health",
    "d_hap_tax_local",
    "tax_shown",
    "health_meter",
    "health_band",
    "unrest",
    "births",
    "deaths",
    "emigrants",
    "immigrants",
    "pop_band",
    "anchor",
    "neighbours",
    "tax_rate",
    "tax_collected",
    "ration_wanted",
    "ration_achieved",
    "ration_split",
    "grain_eaten",
    "herd_eaten",
    "castle_type",
    "castle_building",
    "castle_switch",
    "industry",
    "fields_fallow",
    "fields_cattle",
    "fields_grain",
    "fertility",
    "weather",
    "dryness",
    "grain",
    "herd",
    "labour",
    "labour_wanted",
    "labour_useful",
    "labour_share",
    "industry_share",
    "field_tiles",
    "farm_style",
    "purse",
    "merchant_count",
    "merchant_unit",
    "merchant_visits",
    // C161.
    "herd_change_expected",
    "herd_births_expected",
    "herd_deaths_expected",
    "grain_change_expected",
    "grain_sown_expected",
    "grain_grown_expected",
    "reclaim_fields_finishing",
    "reclaim_seasons_to_next",
    "happiness_avg",
    "happiness_sum",
    "d_hap_tax",
    "shown_army",
    "tax_hap_other",
    "shown_ale",
    "ale_happiness_given",
    "unrest_warned",
    "pop_change_pct",
    "army",
    "largest_inflow",
    "inflow_sources",
    "emigrant_destination",
    "largest_inflow_source",
    "change_reason",
    "event_fired",
    "event_id",
    "event_population_pct",
    "event_population_swing",
    "event_grain_pct",
    "event_herd_pct",
    "tax_suppressed",
    "field_progress",
    "friendly_troops",
    "enemy_troops",
    "levy_surcharge",
    "castle_degraded",
    "castle_ruined",
    "castle_level_left",
    "castle_percent",
    "castle_work_left",
    "castle_work_total",
    "castle_stone_owed",
    "castle_stone_total",
    "castle_wood_owed",
    "castle_wood_total",
    "siege_scars",
    "crop",
    "fields_grain_sown",
    "fields_grain_standing",
    "pasture_cursor",
    "blight_cursor",
    "sow_shortfall",
    "weapon_type",
    "mercenary_offer",
    "grain_weather_change",
    "grain_event_change",
    "herd_weather_change",
    "herd_event_change",
];

/// **Enumerate the fields; do not spot-check them.**
///
/// Reads `pub struct CountyState`'s field list out of
/// `crates/l2-scenario/src/mod.rs` and fails if a field is not in [`JUDGED`].
/// A hand-written list goes stale the day somebody adds a field; a list checked
/// against the definition cannot.
///
/// It needs no game, so it runs on CI — which is the point, because the diff
/// itself is fixture-gated and this is the half of it that is not.
#[test]
fn every_county_field_is_accounted_for() {
    let src = include_str!("../../src/lib.rs");
    let start = src.find("pub struct CountyState {").expect("the struct is still called that");
    let body = &src[start..];
    let end = body.find("\n}").expect("the struct closes");
    let mut fields = Vec::new();
    for line in body[..end].lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("pub ") else { continue };
        let Some(name) = rest.split(':').next() else { continue };
        if name.is_empty() || name.contains(' ') {
            continue;
        }
        fields.push(name.to_string());
    }
    assert!(fields.len() > 20, "the parser found only {} fields", fields.len());
    let missing: Vec<&String> = fields.iter().filter(|f| !JUDGED.contains(&f.as_str())).collect();
    assert!(
        missing.is_empty(),
        "CountyState fields the two-constructor diff does not judge: {missing:?}.\n\
         Add each to JUDGED with a verdict, or say in the test why it cannot be compared."
    );
    let stale: Vec<&&str> = JUDGED.iter().filter(|c| !fields.iter().any(|f| f == *c)).collect();
    assert!(stale.is_empty(), "JUDGED names fields CountyState no longer has: {stale:?}");
}

// ------------------------------------------------------------------ ablation

