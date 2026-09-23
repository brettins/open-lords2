#![allow(unused_imports)]
use super::*;
use super::ring::*;
use super::taxes::*;
use super::super::*;
use crate::tables::Weather;

#[test]
fn a_new_kingdom_holds_seventeen_counties_and_six_realms_with_slot_zero_unused() {
    let k = Kingdom::new(1);
    assert_eq!(k.counties.len(), 17);
    assert_eq!(k.realms.len(), 6);
    assert_eq!(k.county_count, 0);
}

#[test]
fn the_county_count_is_bounded_by_the_array() {
    let mut k = Kingdom::new(1);
    assert!(k.set_county_count(14), "the England map");
    assert!(k.set_county_count(16), "the last usable id");
    assert!(!k.set_county_count(17), "index 0 is not a county");
    assert_eq!(k.county_count, 16, "and a refused count changes nothing");
}

#[test]
fn a_new_game_starts_in_winter_1268_on_turn_one() {
    let mut k = Kingdom::new(1);
    k.set_county_count(14);
    k.start_new_game();
    assert_eq!(k.season, 4);
    assert_eq!(k.season(), Some(Season::Winter));
    assert_eq!(k.year, 1268);
    assert_eq!(k.turn_count, 1);
    assert_eq!(k.season_next, 1, "Spring is next");
    assert_eq!(k.season_prev, 3);
}

#[test]
fn the_year_rolls_in_the_call_that_begins_winter() {
    let mut k = Kingdom::new(1);
    k.start_new_game();
    let mut seen = vec![(k.season, k.year)];
    for _ in 0..8 {
        k.advance_season();
        seen.push((k.season, k.year));
    }
    assert_eq!(
        seen,
        vec![
            (4, 1268),
            (1, 1268),
            (2, 1268),
            (3, 1268),
            (4, 1269),
            (1, 1269),
            (2, 1269),
            (3, 1269),
            (4, 1270),
        ]
    );
}

#[test]
fn the_season_cycles_one_two_three_four_forever() {
    let mut k = Kingdom::new(1);
    k.start_new_game();
    let mut last = k.season;
    for _ in 0..40 {
        k.advance_season();
        let expected = if last == 4 { 1 } else { last + 1 };
        assert_eq!(k.season, expected);
        last = k.season;
    }
}

#[test]
fn a_season_runs_every_pass_in_the_documented_order() {
    let mut k = Kingdom::new(1);
    k.set_county_count(3);
    let report = k.advance_season();
    assert_eq!(report.passes, SEASON_PIPELINE.to_vec());
}

/// **A new game runs no phase-7 pass.** `Game_NewGame` (`0x00497CED`)
/// calls `Season_Advance` and `Score_RankRealms`; `Mercenary_AdvanceAll`,
/// `Units_ResetMoves` (`0x004651B9`) and `Diplo_ReconcileAlliances`
/// (`0x004A1847`) are `Turn_Tick`'s phase-7 arm, which a game that has not
/// started has not reached. End Turn still runs all three, and that is the
/// second half here.
#[test]
fn a_new_game_runs_no_phase_seven_pass_and_a_season_end_runs_all_three() {
    let phase7 =
        [Pass::MercenaryAdvance, Pass::UnitsResetMoves, Pass::ReconcileAlliances];
    let mut k = Kingdom::new(1);
    k.set_county_count(3);
    let new = k.start_new_game();
    for pass in phase7 {
        assert!(!new.passes.contains(&pass), "a new game ran a phase-7 pass: {pass:?}");
    }
    assert_eq!(
        new.passes.len(),
        SEASON_PIPELINE.len() - phase7.len(),
        "and it ran every other pass"
    );
    let ended = k.advance_season();
    for pass in phase7 {
        assert!(ended.passes.contains(&pass), "End Turn stopped running {pass:?}");
    }
}

#[test]
fn only_the_seventh_phase_advances_the_season() {
    let mut k = Kingdom::new(1);
    k.set_county_count(2);
    let mut seasons = 0;
    for _ in 0..64 {
        let (tick, report) = k.tick(true);
        if report.is_some() {
            seasons += 1;
            assert_eq!(tick.phase, Phase::SeasonEnd);
        }
    }
    assert!(seasons >= 3, "several full cycles should have run");
    assert_eq!(k.turn_count as usize, seasons);
}

/// **`Ai_ManageFarmsAll` (`0x0049A990`) farms every realm with strength
/// and no person behind it, at the stall, and nobody else.**
///
/// **The treasury is 4,000 because `Ai_TradeForCounty` (`0x0049E39B`) now
/// shops first**: the Knight's floor of 1,000 is cleared, the county makes
/// crossbows (weapon type 0, good 12) at 24 + `Pct(24, 100)` = 48, and
/// `Ai_BuyGoodDownTo` halves 100 to 50 for 2,400. That leaves exactly the
/// 1,600 the grain lot costs, so both orders land and the test still says
/// what it said.
///
/// *Ablation*: empty `ai_manage_farms_all`'s loop and the first assertion
/// goes red; drop its `!realm.is_human` and the realm-3 one does; drop the
/// `strength != 0` and the realm-4 one does.
#[test]
fn the_season_head_farms_ai_realms_with_strength_and_leaves_the_rest() {
    let mut k = Kingdom::new(1);
    assert!(k.set_county_count(3));
    for (id, realm, human, strength) in [(1usize, 2usize, false, 1u8), (2, 3, true, 1), (3, 4, false, 0)] {
        let r = &mut k.realms[realm];
        r.in_play = true;
        r.strength = strength;
        r.is_human = human;
        r.lord = 1;
        r.gold = 4_000;
        let c = &mut k.counties[id];
        c.owner = realm as u8;
        c.population = 500;
        c.grain = 0;
        c.herd = 50;
        c.merchant_count = 1;
        c.merchant_unit = id as u8;
        let mut m = crate::unit::Unit::new(crate::unit::UnitKind::Merchant, 6, 8, 8);
        m.morale = 100;
        m.county = id as u8;
        k.campaign.units.put(id, m);
    }
    assert_eq!(k.tables.ai_farm_style(1), Some(1), "lord 1 grazes");

    k.ai_manage_farms_all();

    assert_eq!((k.counties[1].grain, k.realms[2].gold), (400, 0), "the AI lord buys 50 crossbows for 2,400 and 400 sacks for 1,600");
    assert_eq!(k.realms[2].weapons[0], 50);
    assert_eq!(k.realms[2].trade_spent_a, 4_000);
    assert_eq!((k.counties[2].grain, k.realms[3].gold), (0, 4_000), "a persons realm is not farmed");
    assert_eq!((k.counties[3].grain, k.realms[4].gold), (0, 4_000), "a realm at strength 0 is not farmed");
}

#[test]
fn two_identical_kingdoms_stay_identical_for_forty_seasons() {
    let build = || {
        let mut k = Kingdom::new(0xA11CE);
        k.options = Options { difficulty: 2, advanced_farming: true, armies_eat: true, ..Options::default() };
        k.set_county_count(14);
        for id in 1..=5 {
            k.realms[id].in_play = true;
            k.realms[id].lord = (id - 1) as u8;
        }
        k.realms[1].is_human = true;
        for id in 1..=14 {
            let owner = if id <= 4 { 1 } else if id <= 8 { 2 } else { 0 };
            let c = &mut k.counties[id];
            c.owner = owner;
            c.population = 400 + id as i32 * 7;
            c.happiness = 60;
            c.health_meter = 65;
            c.health_band = crate::tables::health_band(65);
            c.herd = 60;
            c.grain = 300;
            c.fields_grain = 6;
            c.fields_fallow = 3;
            c.labour[crate::tables::JOB_GRAIN_FARMING] = 500;
            c.labour[crate::tables::JOB_WOOD_CUTTING] = 100;
            c.castle_type = crate::tables::CASTLE_STARTING_TYPE;
            c.tax_rate = 7;
            c.dryness = 40;
            for n in 1..=14u8 {
                if n as usize != id {
                    c.add_neighbour(n);
                }
            }
        }
        k.start_new_game();
        k
    };

    let (mut a, mut b) = (build(), build());
    for season in 0..40 {
        let ra = a.advance_season();
        let rb = b.advance_season();
        assert_eq!(ra, rb, "reports diverged at season {season}");
        assert_eq!(a, b, "state diverged at season {season}");
    }
}

#[test]
fn an_empty_kingdom_advances_its_clock_and_nothing_else() {
    let mut k = Kingdom::new(1);
    k.start_new_game();
    for _ in 0..20 {
        let report = k.advance_season();
        assert!(report.messages.is_empty());
    }
    assert_eq!(k.turn_count, 21);
}

#[test]
fn basic_farming_forces_cloudy_and_zero_fertility_across_the_map() {
    let mut k = Kingdom::new(1);
    k.set_county_count(14);
    for id in 1..=14 {
        k.counties[id].fields_fallow = 5;
        k.counties[id].fields_grain = 1;
        k.counties[id].dryness = 100;
    }
    k.start_new_game();
    for id in 1..=14 {
        assert_eq!(k.counties[id].weather, Weather::Cloudy, "county {id}");
        assert_eq!(k.counties[id].fertility, 0, "county {id}");
    }
}

#[test]
fn tax_is_collected_into_the_owning_realm_and_nowhere_else() {
    let mut k = Kingdom::new(1);
    k.set_county_count(3);
    k.realms[1].in_play = true;
    k.counties[1].owner = 1;
    k.counties[1].population = 1000;
    k.counties[1].tax_rate = 10;
    k.counties[2].owner = 0; // unowned
    k.counties[2].population = 1000;
    k.counties[2].tax_rate = 10;

    let mut report = SeasonReport::new();
    k.run_pass(Pass::TaxCollect, &mut report);
    assert_eq!(k.realms[1].gold, 320);
    assert_eq!(k.counties[2].tax_collected, 320, "computed but banked nowhere");
    assert_eq!(k.realms[0].gold, 0, "realm 0 is not a realm");
}

