#![allow(unused_imports)]
use super::*;
use super::harvest::*;
use super::ai::*;
use super::validation::*;
use super::*;
use super::combat::*;
use common::TempDir;
use l2_mods::Platform;
use l2_sim::{Battle, Troop, TroopTable, SIDE_A, SIDE_B};
use l2_kingdom::tables::{health_band, Tables};
use l2_kingdom::{Kingdom, Options};
use l2_kingdom::tables::{Commodity, JOB_COUNT};
use l2_kingdom::tables::{health_band, Tables};
use l2_kingdom::{Kingdom, Options};
use l2_kingdom::tables::{Commodity, JOB_COUNT};

use l2_kingdom::tables::{Commodity, JOB_COUNT};

/// Build a platform over one mod whose single rule file is `rules`, and take
/// the economy table out of it.
fn modded(name: &'static str, rules: &str) -> Tables {
    let base = TempDir::new(&format!("{name}-base"));
    let mods = TempDir::new(&format!("{name}-mods"));
    empty_base(&base);
    mods.write(&format!("{name}/mod.toml"), &format!("[mod]\nid = \"{name}\"\n"));
    mods.write(&format!("{name}/rules/{name}.toml"), rules);
    Platform::builder()
        .base(base.path())
        .mods_dir(mods.path())
        .enable([name])
        .build()
        .expect("the mod loads")
        .kingdom_tables()
        .expect("and its kingdom rules validate")
}

/// One human-owned county of five hundred people, already through its first
/// season, on whatever rules it is handed.
fn one_county(tables: Tables) -> Kingdom {
    let mut k = Kingdom::with_tables(0xBEEF_0001, tables);
    k.options = Options { difficulty: 0, advanced_farming: false, ..Options::default() };
    assert!(k.set_county_count(1));
    k.realms[1].in_play = true;
    k.realms[1].is_human = true;
    k.realms[1].county_count = 1;

    let c = &mut k.counties[1];
    c.owner = 1;
    c.population = 500;
    c.happiness = 60;
    c.health_meter = 65;
    c.health_band = health_band(65);
    c.herd = 500;
    c.grain = 400;
    c.fields_grain = 5;
    c.ration_wanted = 3;
    c.ration_split = 100;
    c.labour = [100_000; JOB_COUNT];
    k.start_new_game();
    k
}

/// A hundred crowns of ale, then a full year through `Season_Advance`.
/// Returns the happiness the ale itself bought
/// year later.
fn ale_then_a_year(tables: Tables) -> (i32, i32) {
    let mut k = one_county(tables);
    // The setup season moved the population; pin it back so the ladder's step
    // is a round tenth
    k.counties[1].population = 500;
    k.counties[1].happiness = 55;
    let gained = l2_kingdom::happiness::buy_ale(&k.tables, &mut k.counties[1], 100, k.options.quirks);
    for _ in 0..4 {
        k.advance_season();
    }
    (gained, k.counties[1].population)
}

#[test]
fn what_a_barrel_of_ale_buys_is_a_rule_a_mod_sets() {
    let cheap = modded("cheap-ale", "[kingdom.happiness]\nale_step_pct = 100\nale_max = 20\n");
    assert_eq!(cheap.ale.step_pct, 100);
    assert_eq!(cheap.ale.max, 20);

    let (stock_gain, stock_pop) = ale_then_a_year(Tables::DEFAULT);
    let (mod_gain, mod_pop) = ale_then_a_year(cheap);

    // A hundred crowns in a county of 500 is two rungs of the stock ladder
    // (one per fifty crowns) and twenty of the modded one (one per five,
    // stopped by ale_max).
    assert_eq!(stock_gain, 2);
    assert_eq!(mod_gain, 20);
    // And a happier county breeds: the year ends with more people in it.
    assert!(
        mod_pop > stock_pop,
        "cheap ale should leave more people alive: {mod_pop} vs {stock_pop}"
    );
}

/// Raising fifty men out of five hundred, then a year.
fn army_then_a_year(tables: Tables) -> (i32, i32, i32) {
    let mut k = one_county(tables);
    k.counties[1].population = 500;
    k.counties[1].happiness = 95;
    let cost = l2_kingdom::happiness::raise_army(&k.tables, &mut k.counties[1], 50);
    let after = k.counties[1].happiness;
    for _ in 0..4 {
        k.advance_season();
    }
    (cost, after, k.counties[1].population)
}

#[test]
fn what_raising_an_army_costs_a_county_is_a_rule_a_mod_sets() {
    // The table is indexed by the percentage of the county taken
    // restates all 102 rows. This one is flat: any army at all costs 60.
    let mut rows = String::from("[kingdom.happiness]\narmy_cost = [0");
    for _ in 1..102 {
        rows.push_str(", 60");
    }
    rows.push_str("]\n");
    let brutal = modded("brutal-levy", &rows);
    assert_eq!(brutal.army_happiness_cost[10], 60);

    let (stock_cost, stock_happy, stock_pop) = army_then_a_year(Tables::DEFAULT);
    let (mod_cost, mod_happy, mod_pop) = army_then_a_year(brutal);

    // Fifty men is a tenth of five hundred, which the stock table prices at 5.
    assert_eq!(stock_cost, 5);
    assert_eq!(mod_cost, 60);
    assert_eq!((stock_happy, mod_happy), (90, 35));
    assert!(
        mod_pop < stock_pop,
        "a levy that costs sixty happiness should cost people too: {mod_pop} vs {stock_pop}"
    );
}

/// Three years of wood-cutting with *Advanced Farming* on, so the efficiency
/// ramp runs. Returns the realm's timber
/// efficiency.
fn three_years_of_timber(tables: Tables) -> (i32, i32) {
    let mut k = one_county(tables);
    k.options.advanced_farming = true;
    let wood = Commodity::Wood.index();
    let job = k.tables.commodity[wood].job;
    let c = &mut k.counties[1];
    c.industry[wood].enabled = true;
    c.industry[wood].has_resource = true;
    c.industry[wood].capacity = 100_000;
    // The ramp compounds from wherever it left off
    // on the flat Advanced-Farming-off figure. Start it at zero so the twelve
    // seasons below are the whole ramp and nothing else.
    c.industry[wood].efficiency = 0;
    // `FUN_0044F248` ramps from county `+0x29C`, which is this field.
    c.industry[wood].last_efficiency = 0;
    c.labour = [0; JOB_COUNT];
    c.labour[job] = 100;
    k.realms[1].wood = 0;
    // **Twelve wood-cutting passes, not twelve whole seasons.** The season
    // pipeline runs `Labour_AllocateAll` twice now
    // straight into the record does not survive a full `advance_season` — the
    // allocator rebuilds all nine records from the population
    // ceilings. Running the one pass keeps the hundred cutters fixed, which is
// what makes the totals below exact arithmetic on the ramp
    // measurement of the allocator.
    let mut report = l2_kingdom::SeasonReport::new();
    for _ in 0..12 {
        k.run_pass(l2_kingdom::Pass::Industry(Commodity::Wood), &mut report);
    }
    (k.realms[1].wood, k.counties[1].industry[wood].efficiency)
}

#[test]
fn the_efficiency_ramps_ceiling_is_a_rule_a_mod_sets() {
    let capped = modded("low-ceiling", "[kingdom.efficiency]\nmax = 30\n");
    assert_eq!(capped.efficiency.max, 30);
    assert_eq!(
        capped.efficiency.without_advanced_farming,
        Tables::DEFAULT.efficiency.without_advanced_farming,
        "and nothing else in the ramp moved"
    );

    let (stock_wood, stock_eff) = three_years_of_timber(Tables::DEFAULT);
    let (mod_wood, mod_eff) = three_years_of_timber(capped);

    // Wood's base is 20, so the stock ramp reaches its ceiling in five seasons
    //
    assert_eq!(stock_eff, 100);
    assert_eq!(mod_eff, 30);
    // A hundred wood-cutters at e percent efficiency fell e loads a season, so
    // the totals are the ramps summed: 20+40+60+80 then eight seasons at 100,
    // against 20 then eleven at 30.
    assert_eq!((stock_wood, mod_wood), (1_000, 350));
}

