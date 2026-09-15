
use l2_formats::maps::{MapSet, Plane, PLANE_DIM, SLOT_LEN};
use l2_kingdom::map::MAP_TILES;
use l2_scenario::newgame::{self, MapError, NewGame};
use l2_scenario::{CountyState, Scenario};

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

const SEED: u64 = 7;


#[derive(Debug, PartialEq, Eq)]
enum Verdict {
    Agree,
    Differ(&'static str),
    BothSilent(&'static str),
}

const UNEXPLAINED: &str = "UNEXPLAINED";

fn england_pair(save: &l2_formats::save::Save, maps: &[u8]) -> (Scenario, Scenario) {
    let from_save = Scenario::from_save(save).expect("the fixture imports");
    let set = MapSet::parse(maps).expect("L2_maps.dat parses");
    let slot = set.slot(ENGLAND).expect("slot 0");
    let setup = NewGame {
        slot: ENGLAND,
        lords: 5,
        local_player: from_save.local_player,
        shield: from_save.realms[from_save.local_player as usize].shield_index,
        seed: SEED,
        options: from_save.options,
    };
    let from_map = Scenario::from_map(&slot, &setup).expect("England builds");
    (from_save, from_map)
}

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

