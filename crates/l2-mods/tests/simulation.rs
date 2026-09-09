//! The whole path, end to end: a `.toml` in a mod directory changes what
//! happens when two figures fight.
//!
//! Everything else in this crate's tests checks a stage — the document parses,
//! the merge resolves, the table loads. This checks that the stages are
//! actually joined up, which is the claim that matters and the one easiest to
//! believe without evidence.
//!
//! `docs/decisions.md` C11 is why it is worth a file of its own. These numbers
//! live in `Lords2.exe` as instructions; there is no data file in the 1996
//! game that reaches them. A test that starts at a text file and ends at a
//! different casualty count is the demonstration that the situation has
//! changed.

mod common;

use common::TempDir;
use l2_mods::Platform;
use l2_sim::{Battle, Troop, TroopTable, SIDE_A, SIDE_B};

fn empty_base(dir: &TempDir) {
    // A layer must be a directory that exists; it need contain nothing. The
    // engine's own rules are compiled in, so a game with no install at all
    // still has combat constants.
    dir.write("placeholder.txt", "not a rule and not an asset the engine asks for");
}

fn duel(table: TroopTable, a: Troop, b: Troop, men: u16, ticks: u32) -> (u32, u32) {
    let mut battle = Battle::with_troops(table);
    battle.add(a, SIDE_A, men).unwrap();
    battle.add(b, SIDE_B, men).unwrap();
    battle.engage(0, 1);
    battle.run(ticks);
    (battle.men(SIDE_A), battle.men(SIDE_B))
}

/// With no mods at all, the loaded table is the engine's table. This is the
/// half that is easy to forget to check: a loader that quietly returned
/// defaults would pass every "the mod changed it" test ever written.
#[test]
fn a_load_with_no_mods_reproduces_the_engines_own_combat_constants() {
    let base = TempDir::new("sim-plain");
    empty_base(&base);
    let p = Platform::builder().base(base.path()).build().unwrap();
    assert_eq!(p.troop_table().unwrap(), TroopTable::DEFAULT);
}

/// Two lines of TOML, and swordsmen lose a fight they used to win.
#[test]
fn a_mod_file_changes_the_outcome_of_a_battle() {
    let base = TempDir::new("sim-base");
    let mods = TempDir::new("sim-mods");
    empty_base(&base);
    mods.write("angry-peasants/mod.toml", "[mod]\nid = \"angry-peasants\"\n");
    mods.write(
        "angry-peasants/rules/peasants.toml",
        "[unit.peasants]\nmelee_attack = [40, 40, 40, 40]\n",
    );

    let plain = Platform::builder().base(base.path()).build().unwrap();
    let modded = Platform::builder()
        .base(base.path())
        .mods_dir(mods.path())
        .enable(["angry-peasants"])
        .build()
        .unwrap();

    // The rule really did override rather than being added, so the mod is
    // speaking the engine's vocabulary and not inventing its own.
    let report = modded.report();
    assert_eq!(report.overrides.len(), 1, "{report}");
    assert_eq!(report.overrides[0].path, "unit.peasants.melee_attack");
    assert!(report.added_rules.is_empty(), "{report}");

    let before = duel(plain.troop_table().unwrap(), Troop::Peasants, Troop::Swordsmen, 20, 2000);
    let after = duel(modded.troop_table().unwrap(), Troop::Peasants, Troop::Swordsmen, 20, 2000);

    assert!(
        after.0 > before.0,
        "peasants swinging for 40 should fare better than the stock 5: {after:?} vs {before:?}"
    );
    assert!(after.1 < before.1, "and the swordsmen should suffer for it");
}

/// The whole stack, in order: two mods, the later one wins, and the winning
/// number is the one the simulation fights with. Load order is the
/// conflict-resolution policy right through to the casualty count.
#[test]
fn the_last_mod_in_the_load_order_is_the_one_the_simulation_runs_on() {
    let base = TempDir::new("sim-order-base");
    let mods = TempDir::new("sim-order-mods");
    empty_base(&base);
    for (id, armour) in [("aaa", 10), ("zzz", 55)] {
        mods.write(&format!("{id}/mod.toml"), &format!("[mod]\nid = \"{id}\"\n"));
        mods.write(
            &format!("{id}/rules/{id}.toml"),
            &format!("[unit.archers]\narmour = {armour}\n"),
        );
    }

    let forward = Platform::builder()
        .base(base.path())
        .mods_dir(mods.path())
        .enable(["aaa", "zzz"])
        .build()
        .unwrap();
    let backward = Platform::builder()
        .base(base.path())
        .mods_dir(mods.path())
        .enable(["zzz", "aaa"])
        .build()
        .unwrap();

    let armour_of = |p: &Platform| p.troop_table().unwrap().stats(Troop::Archers).armour;
    assert_eq!(armour_of(&forward), 55);
    assert_eq!(armour_of(&backward), 10);

    // And a figure built from that table carries the number, which is the seam
    // that makes the simulation data-driven rather than merely configurable.
    let mut battle = Battle::with_troops(forward.troop_table().unwrap());
    let i = battle.add(Troop::Archers, SIDE_A, 4).unwrap();
    assert_eq!(battle.figures[i].armour(), 55);
}

/// A rule outside the range the simulation can use is refused at load, naming
/// the mod, the file and the line — not clamped, and not discovered later as a
/// figure that cannot be hit.
#[test]
fn an_impossible_combat_constant_is_refused_with_the_line_that_wrote_it() {
    let base = TempDir::new("sim-bad-base");
    let mods = TempDir::new("sim-bad-mods");
    empty_base(&base);
    mods.write("bad/mod.toml", "[mod]\nid = \"bad\"\n");
    mods.write("bad/rules/bad.toml", "[unit.archers]\nrecovery = 0\n");

    let p = Platform::builder()
        .base(base.path())
        .mods_dir(mods.path())
        .enable(["bad"])
        .build()
        .expect("the document is well formed, so the load itself succeeds");

    let err = p.troop_table().unwrap_err().to_string();
    assert!(err.contains("bad:rules/bad.toml:2:12"), "{err}");
    assert!(err.contains("unit.archers.recovery"), "{err}");
    assert!(err.contains("0 is outside 1..=65535"), "{err}");
}

/// Strength bands run best first. A rising row would mean a weakened figure
/// hitting harder, which is not a rebalance.
#[test]
fn a_rising_strength_band_is_refused_rather_than_sorted() {
    let base = TempDir::new("sim-band-base");
    let mods = TempDir::new("sim-band-mods");
    empty_base(&base);
    mods.write("rising/mod.toml", "[mod]\nid = \"rising\"\n");
    mods.write("rising/rules/r.toml", "[unit.knights]\nmelee_attack = [5, 9, 3, 1]\n");

    let p = Platform::builder()
        .base(base.path())
        .mods_dir(mods.path())
        .enable(["rising"])
        .build()
        .unwrap();

    let err = p.troop_table().unwrap_err().to_string();
    assert!(err.contains("the bands run best first"), "{err}");
    assert!(err.contains("rising:rules/r.toml"), "{err}");
}

/// The economy loads and validates the same way, and a mod changes the values
/// the table carries — two of them at once, with nothing else moved.
///
/// This stops at the table on purpose; that the table is then *consumed* is
/// what `a_mod_file_changes_what_a_county_harvests` asserts, and the two are
/// worth keeping apart. An earlier revision of this test was named
/// `..._even_though_the_economy_does_not_read_it_yet`, which was true when it
/// was written and is not now.
#[test]
fn a_kingdom_mod_overrides_exactly_the_two_rules_it_names() {
    let base = TempDir::new("sim-king-base");
    let mods = TempDir::new("sim-king-mods");
    empty_base(&base);
    mods.write("famine/mod.toml", "[mod]\nid = \"famine\"\n");
    mods.write(
        "famine/rules/famine.toml",
        "[kingdom.grain]\nyield_per_sack = 6\n\n[kingdom.food]\nfood_per_sack = 3\n",
    );

    let plain = Platform::builder().base(base.path()).build().unwrap();
    assert_eq!(plain.kingdom_tables().unwrap(), l2_kingdom::tables::Tables::DEFAULT);

    let modded = Platform::builder()
        .base(base.path())
        .mods_dir(mods.path())
        .enable(["famine"])
        .build()
        .unwrap();
    let t = modded.kingdom_tables().unwrap();
    assert_eq!(t.grain.yield_per_sack, 6);
    assert_eq!(t.food.food_per_sack, 3);
    // Nothing else moved.
    let stock = l2_kingdom::tables::Tables::DEFAULT;
    assert_eq!(t.grain.max_sacks_per_field, stock.grain.max_sacks_per_field);
    assert_eq!(t.food.food_per_head, stock.food.food_per_head);
    assert_eq!(modded.report().overrides.len(), 2, "{}", modded.report());
}

/// The example mod in `example-mods/` is the one printed in the
/// documentation, and its `unit.*` half needs no game install to check.
#[test]
fn the_documented_example_mod_reaches_the_simulation() {
    let base = TempDir::new("sim-example-base");
    empty_base(&base);
    let p = Platform::builder()
        .base(base.path())
        .mods_dir(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("example-mods"))
        .enable(["longbows"])
        .build()
        .expect("the documented example loads");

    let archers = p.troop_table().unwrap().stats(Troop::Archers);
    assert_eq!(archers.melee_attack, [7, 5, 4, 2]);
    assert_eq!(archers.armour, 4);
    // Its battle and difficulty rules land on nothing here, because no game
    // install has been seeded - which the report says plainly rather than
    // leaving the author to wonder.
    assert!(!p.report().added_rules.is_empty());
}

// ---------------------------------------------------------------------------
// The economy, end to end
// ---------------------------------------------------------------------------
//
// The same claim as `a_mod_file_changes_the_outcome_of_a_battle`, on the other
// half of the engine: a `.toml` in a mod directory changes what a county
// harvests. Until the rules were threaded through `l2-kingdom` this could only
// be asserted as far as the *table*, which is a much weaker statement — a
// loader that stored a number nothing read would have passed it.

use l2_kingdom::tables::{health_band, Tables};
use l2_kingdom::{Kingdom, Options};

/// A one-county kingdom with five grain fields, a hundred sacks of seed and
/// more labour than sowing can use, run through a full year: Spring sows,
/// Summer and Autumn grow, Winter harvests.
///
/// Rations come entirely from the herd (`ration_split = 100`) and the herd is
/// large, so nothing eats the grain and the store at the end of the year is the
/// harvest.
fn a_years_harvest(tables: Tables) -> i32 {
    let mut k = Kingdom::with_tables(0xF00D_1234, tables);
    k.options = Options { difficulty: 0, advanced_farming: false, ..Options::default() };
    assert!(k.set_county_count(1));
    k.realms[1].in_play = true;
    k.realms[1].is_human = true;
    k.realms[1].county_count = 1;

    let c = &mut k.counties[1];
    c.owner = 1;
    c.population = 400;
    c.pop_band = c.compute_pop_band();
    c.happiness = 65;
    c.health_meter = 65;
    c.health_band = health_band(65);
    c.herd = 400;
    c.grain = 100;
    c.ration_wanted = 3; // Normal
    c.ration_split = 100; // fed entirely from the herd
    c.labour = [100_000; l2_kingdom::tables::JOB_COUNT];

    // **Five grain fields, on the map.** Writing `fields_grain = 5` into the
    // record is not enough any more and never should have been:
    // `County_RecountFieldsAll` is a pass of the season now, and it rebuilds
    // all five counts from the terrain byte of the twenty tiles the county
    // names. A county with no field tiles has no fields, whatever its record
    // says. `docs/kingdom.md` §7.2.
    for slot in 0..5usize {
        let tile = slot + 1;
        k.counties[1].field_tiles[slot] = tile as u16;
        k.campaign.map.terrain[tile] = l2_kingdom::field::terrain::GRAIN;
    }

    k.start_new_game(); // Winter 1268
    k.advance_season(); // Spring: sow
    // Whatever is left in the barn after sowing is seed the county did not
    // plant, and it is still there in Winter. Subtracting it leaves the
    // harvest itself, which is the number the yield rule decides.
    let unsown = k.counties[1].grain;
    for _ in 0..3 {
        k.advance_season(); // Summer, Autumn, Winter
    }
    k.counties[1].grain - unsown
}

/// With no mods at all the kingdom runs on the engine's own numbers. The half
/// that is easy to forget: a `Kingdom` that quietly ignored its table would
/// pass every "the mod changed it" test ever written.
#[test]
fn a_load_with_no_mods_reproduces_the_engines_own_economy() {
    let base = TempDir::new("king-plain");
    empty_base(&base);
    let p = Platform::builder().base(base.path()).build().unwrap();
    let loaded = p.kingdom_tables().unwrap();
    assert_eq!(loaded, Tables::DEFAULT);
    assert_eq!(
        a_years_harvest(loaded),
        a_years_harvest(Tables::DEFAULT),
        "the loaded table and the engine's own must farm identically"
    );
    assert_eq!(Kingdom::new(1).tables, Tables::DEFAULT);
}

/// **Two lines of TOML, and the county starves.**
///
/// This is the assertion `docs/modding.md` §11 used to say could not be made.
/// It starts at a text file in a mod directory and ends at a different number
/// of sacks in a barn, through the real `Season_Advance` pipeline.
#[test]
fn a_mod_file_changes_what_a_county_harvests() {
    let base = TempDir::new("king-base");
    let mods = TempDir::new("king-mods");
    empty_base(&base);
    mods.write("famine/mod.toml", "[mod]\nid = \"famine\"\n");
    mods.write(
        "famine/rules/famine.toml",
        "[kingdom.grain]\nyield_per_sack = 6\n",
    );

    let plain = Platform::builder().base(base.path()).build().unwrap();
    let modded = Platform::builder()
        .base(base.path())
        .mods_dir(mods.path())
        .enable(["famine"])
        .build()
        .unwrap();

    // It overrode a rule the engine already had, rather than inventing one.
    let report = modded.report();
    assert_eq!(report.overrides.len(), 1, "{report}");
    assert_eq!(report.overrides[0].path, "kingdom.grain.yield_per_sack");
    assert!(report.added_rules.is_empty(), "{report}");

    let before = a_years_harvest(plain.kingdom_tables().unwrap());
    let after = a_years_harvest(modded.kingdom_tables().unwrap());

    assert!(before > 0, "the stock year should actually grow something: {before}");
    assert!(
        after < before,
        "halving the yield per sack should halve the harvest: {after} vs {before}"
    );
    // Exactly half, and that is worth pinning rather than leaving as an
    // inequality: the yield multiplies the crop once at sowing and nothing
    // downstream of it is a threshold at these quantities.
    assert_eq!(after * 2, before, "{after} should be half of {before}");
}

/// Load order decides the economy too, right through to the barn — the same
/// claim `the_last_mod_in_the_load_order_is_the_one_the_simulation_runs_on`
/// makes about a casualty count.
#[test]
fn the_last_kingdom_mod_in_the_load_order_is_the_one_the_economy_runs_on() {
    let base = TempDir::new("king-order-base");
    let mods = TempDir::new("king-order-mods");
    empty_base(&base);
    for (id, yield_per_sack) in [("aaa", 6), ("zzz", 24)] {
        mods.write(&format!("{id}/mod.toml"), &format!("[mod]\nid = \"{id}\"\n"));
        mods.write(
            &format!("{id}/rules/{id}.toml"),
            &format!("[kingdom.grain]\nyield_per_sack = {yield_per_sack}\n"),
        );
    }

    let build = |order: [&str; 2]| {
        Platform::builder()
            .base(base.path())
            .mods_dir(mods.path())
            .enable(order)
            .build()
            .unwrap()
            .kingdom_tables()
            .unwrap()
    };
    let forward = build(["aaa", "zzz"]);
    let backward = build(["zzz", "aaa"]);
    assert_eq!(forward.grain.yield_per_sack, 24);
    assert_eq!(backward.grain.yield_per_sack, 6);
    let high = a_years_harvest(forward);
    let low = a_years_harvest(backward);
    assert!(high > low, "the last mod loaded is the one that farms: {high} against {low}");

    // **And it is not four times, which is the labour cap talking.** The yield
    // per sack is in `Grain_Sow`'s own labour test — `yield * seed / divisor`
    // hands are needed to tend the seed — so a fourfold yield makes each sack
    // four times as hungry for farmhands, and the county sows correspondingly
    // less of it. `Grain_Grow` and `Grain_Harvest` then cap the standing crop
    // at `labour * multiplier` twice more. Quadrupling the number in the file
    // doubles what reaches the barn.
    assert_eq!((high, low), (600, 300));
}

/// A rule with no arithmetic in it at all: the year random events start.
/// Pushing it forward means a modded game draws no events where the stock one
/// does, which is a behavioural difference rather than a numeric one.
#[test]
fn a_mod_can_postpone_the_first_year_random_events_are_drawn() {
    let base = TempDir::new("king-event-base");
    let mods = TempDir::new("king-event-mods");
    empty_base(&base);
    mods.write("quiet/mod.toml", "[mod]\nid = \"quiet\"\n");
    mods.write("quiet/rules/quiet.toml", "[kingdom.event]\nfirst_year = 1400\n");

    let modded = Platform::builder()
        .base(base.path())
        .mods_dir(mods.path())
        .enable(["quiet"])
        .build()
        .unwrap()
        .kingdom_tables()
        .unwrap();
    assert_eq!(modded.event.first_year, 1400);

    let mut county = l2_kingdom::county::County::new();
    county.owner = 1;
    assert!(
        l2_kingdom::event::eligible(&Tables::DEFAULT, &county, true, 1300),
        "the stock game is drawing events by 1300"
    );
    assert!(
        !l2_kingdom::event::eligible(&modded, &county, true, 1300),
        "and the modded one is not"
    );
}

// ---------------------------------------------------------------------------
// The five rules that had no field until now
// ---------------------------------------------------------------------------
//
// `docs/modding.md` §11 used to end with a list of rules a document could not
// reach at all: the ale ladder, the army-raising cost, the efficiency ramp's
// ceiling, and the AI's tax ladders and personality table. Each has a field
// now, and each test below is the same shape as
// `a_mod_file_changes_what_a_county_harvests` — a `.toml` in a mod directory
// in, a different number out of the real simulation.
//
// The one that is *not* here is `kingdom.ai.personality.*.farm_style`. It
// loads and validates, and nothing in `l2-kingdom` reads it, because the three
// labour allocators `AI_ManageFields` dispatches into were never traced. There
// is no observable effect to assert, so there is no test claiming one —
// `docs/decisions.md` C12.

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
/// Returns the happiness the ale itself bought and the county's population a
/// year later.
fn ale_then_a_year(tables: Tables) -> (i32, i32) {
    let mut k = one_county(tables);
    // The setup season moved the population; pin it back so the ladder's step
    // is a round tenth and the two runs differ only in the rule.
    k.counties[1].population = 500;
    k.counties[1].happiness = 55;
    let gained = l2_kingdom::happiness::buy_ale(&k.tables, &mut k.counties[1], 100);
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
    // The table is indexed by the percentage of the county taken, so a mod
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
/// ramp actually runs. Returns the realm's timber and the county's final
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
    // The ramp compounds from wherever it left off, and the setup season ran
    // on the flat Advanced-Farming-off figure. Start it at zero so the twelve
    // seasons below are the whole ramp and nothing else.
    c.industry[wood].efficiency = 0;
    c.labour = [0; JOB_COUNT];
    c.labour[job] = 100;
    k.realms[1].wood = 0;
    // **Twelve wood-cutting passes, not twelve whole seasons.** The season
    // pipeline runs `Labour_AllocateAll` twice now, so a workforce written
    // straight into the record does not survive a full `advance_season` — the
    // allocator rebuilds all nine records from the population and the
    // ceilings. Running the one pass keeps the hundred cutters fixed, which is
    // what makes the totals below exact arithmetic on the ramp rather than a
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
    // and the modded one is stopped at its own after two.
    assert_eq!(stock_eff, 100);
    assert_eq!(mod_eff, 30);
    // A hundred wood-cutters at e percent efficiency fell e loads a season, so
    // the totals are the ramps summed: 20+40+60+80 then eight seasons at 100,
    // against 20 then eleven at 30.
    assert_eq!((stock_wood, mod_wood), (1_000, 350));
}

/// An AI realm taxing one county for a season. Returns the rate its ladder
/// chose and the gold that rate collected.
fn an_ai_seasons_tax(tables: Tables, happiness: i32) -> (i32, i32) {
    let mut k = one_county(tables);
    k.realms[1].is_human = false;
    k.realms[1].lord = 1;
    k.realms[1].gold = 0;
    k.counties[1].happiness = happiness;
    k.run_ai_tax_rates(1);
    let rate = k.counties[1].tax_rate;
    k.advance_season();
    (rate, k.realms[1].gold)
}

/// An AI lord laying out one county's eight fields, in Winter. Returns
/// `(grain, pasture, industry share)`.
///
/// This is the rule `docs/modding.md` used to list as *"loads and does
/// nothing"*. It does something now, and this is where it has to earn that:
/// the same county, the same season, the same eight tiles, and a different
/// answer because one byte of the ruleset changed.
fn an_ai_lays_out_a_county(tables: Tables, lord: u8) -> (i32, i32, i32) {
    let mut k = one_county(tables);
    k.realms[1].is_human = false;
    k.realms[1].lord = lord;
    k.options.advanced_farming = true;
    k.season = 4; // Winter: the only season the arable styles re-sow in
    k.season_next = 1;
    for i in 0..8u8 {
        let tile = l2_kingdom::map::index(i, 8);
        k.counties[1].set_field_tile(i as usize, Some(tile));
        k.campaign.map.terrain[tile] = l2_kingdom::field::terrain::FALLOW;
    }
    k.counties[1].herd = 400;
    k.counties[1].fertility = 0;
    l2_kingdom::field::recount(&mut k.counties[1], &k.campaign.map);
    k.counties[1].herd_crowding =
        l2_kingdom::land::herd_crowding(&k.tables, k.counties[1].herd, k.counties[1].fields_cattle);
    // No merchant stall in this scenario, so every style's opening shopping
    // cascade is refused and the layout is the only thing that differs.
    k.run_ai_farms(1, &mut l2_kingdom::ai_farm::NoMarket);
    l2_kingdom::field::recount(&mut k.counties[1], &k.campaign.map);
    (k.counties[1].fields_grain, k.counties[1].fields_cattle, k.counties[1].industry_share)
}

/// **`farm_style` is a rule a mod sets, and it changes the map.**
#[test]
fn which_way_an_ai_lord_farms_is_a_rule_a_mod_sets() {
    // Lord 1 ships as style 1, a grazier: no grain at all, and the pasture
    // grows one field a pass towards all but one of the county.
    let (grain, pasture, share) = an_ai_lays_out_a_county(Tables::DEFAULT, 1);
    assert_eq!(grain, 0, "a grazier plants nothing, even in Winter");
    assert_eq!(pasture, 1, "and takes one more field for the herd");
    assert_eq!(share, 20);

    // Turn him into an arable lord with one byte.
    let arable = modded(
        "arable-lord",
        "[[kingdom.ai.personality]]\nlord = 1\nfarm_style = 0\ntax_ladder = 2\n\
         gift_increment = 100\nhelp_price = 500\ngrudge_tolerance = 5\noffer_interval = 12\n\
         help_population_floor = 750\nmuster_pct = 30\ncastle_concurrent = 4\n\
         castle_min_population = 700\ncastle_gold = [200, 0, 1000, 0, 10000]\n\
         weapon_rota = [0, 1, 4, 2, 5, 2]\nsiege_doctrine = 8\n\n\
         [[kingdom.ai.personality]]\nlord = 2\nfarm_style = 1\ntax_ladder = 2\n\
         gift_increment = 100\nhelp_price = 1000\ngrudge_tolerance = 10\noffer_interval = 10\n\
         help_population_floor = 800\nmuster_pct = 30\ncastle_concurrent = 3\n\
         castle_min_population = 650\ncastle_gold = [0, 500, 0, 4000, 0]\n\
         weapon_rota = [3, 5, 4, 4, 4, 5]\nsiege_doctrine = 9\n\n\
         [[kingdom.ai.personality]]\nlord = 3\nfarm_style = 0\ntax_ladder = 2\n\
         gift_increment = 200\nhelp_price = 1600\ngrudge_tolerance = 15\noffer_interval = 8\n\
         help_population_floor = 900\nmuster_pct = 40\ncastle_concurrent = 2\n\
         castle_min_population = 600\ncastle_gold = [0, 300, 0, 2000, 0]\n\
         weapon_rota = [0, 1, 1, 2, 4, 0]\nsiege_doctrine = 7\n\n\
         [[kingdom.ai.personality]]\nlord = 4\nfarm_style = 9\ntax_ladder = 1\n\
         gift_increment = 50\nhelp_price = 1500\ngrudge_tolerance = 20\noffer_interval = 4\n\
         help_population_floor = 1000\nmuster_pct = 50\ncastle_concurrent = 1\n\
         castle_min_population = 600\ncastle_gold = [0, 0, 100, 0, 2000]\n\
         weapon_rota = [4, 4, 3, 4, 4, 3]\nsiege_doctrine = 7\n",
    );
    assert_eq!(arable.ai.personality[0].farm_style, 0);

    let (grain, pasture, share) = an_ai_lays_out_a_county(arable, 1);
    assert_eq!(grain, 4, "an arable lord plants half the county");
    assert_eq!(pasture, 1, "and keeps exactly one pasture for a herd over ten");
    assert_eq!(share, 50, "and puts half his people into industry rather than the farm");
}

#[test]
fn the_ai_tax_ladders_are_rules_a_mod_sets() {
    // Ladder 2 is the one three of the four lords use, and it charges nothing
    // below 60 happiness. Arrays replace whole on merge, so restating it means
    // restating every rung; this one has a single rung and a flat 40%.
    let greedy =
        modded("tax-farmers", "[[kingdom.ai.tax_ladder.2]]\nbelow = 2147483647\nrate = 40\n");
    assert_eq!(greedy.ai.tax_ladders[2][0], (i32::MAX, 40));
    assert_eq!(greedy.ai.tax_ladders[0], Tables::DEFAULT.ai.tax_ladders[0], "0 untouched");
    assert_eq!(greedy.ai.tax_ladder_neutral, Tables::DEFAULT.ai.tax_ladder_neutral);

    let (stock_rate, stock_gold) = an_ai_seasons_tax(Tables::DEFAULT, 50);
    let (mod_rate, mod_gold) = an_ai_seasons_tax(greedy, 50);

    assert_eq!(stock_rate, 0, "the stock ladder taxes a county of 50 happiness nothing");
    assert_eq!(mod_rate, 40);
    assert_eq!(stock_gold, 0, "and so banks nothing");
    assert!(mod_gold > 0, "where the modded one banks {mod_gold}");
}

#[test]
fn which_ladder_an_ai_lord_taxes_on_is_a_rule_a_mod_sets() {
    // The personality table replaces whole too, so all four lords are restated
    // in full. Only lord 1 moves, from the gentlest ladder to the greediest;
    // every other field is the stock value, so the one thing that changes is
    // the one thing under test.
    //
    // The castle columns are the four lords as the binary has them, which is
    // where the Bishop's royal castle at 2,000 gold sits — against the Knight's
    // 10,000, and the Baron and Countess who are never offered one at any
    // treasury. `docs/diplomacy.md` §8.1.
    #[allow(clippy::too_many_arguments)]
    fn lord(
        n: u8, farm: u8, ladder: u8, gift: i32, help: i32, grudge: i32, offer: i32, floor: i32,
        muster: i32, concurrent: i32, min_pop: i32, gold: &str,
    ) -> String {
        format!(
            "[[kingdom.ai.personality]]\nlord = {n}\nfarm_style = {farm}\ntax_ladder = {ladder}\n\
             gift_increment = {gift}\nhelp_price = {help}\ngrudge_tolerance = {grudge}\n\
             offer_interval = {offer}\nhelp_population_floor = {floor}\nmuster_pct = {muster}\n\
             castle_concurrent = {concurrent}\ncastle_min_population = {min_pop}\n\
             castle_gold = [{gold}]\nweapon_rota = [0, 1, 2, 3, 4, 5]\n\
             siege_doctrine = 8\n\n"
        )
    }
    let rows = lord(1, 1, 0, 100, 500, 5, 12, 1000, 30, 4, 700, "200, 0, 1000, 0, 10000")
        + &lord(2, 1, 2, 100, 1000, 10, 10, 1000, 30, 3, 650, "0, 500, 0, 4000, 0")
        + &lord(3, 0, 2, 200, 1600, 15, 8, 1000, 40, 2, 600, "0, 300, 0, 2000, 0")
        + &lord(4, 9, 1, 50, 1500, 20, 4, 1000, 50, 1, 600, "0, 0, 100, 0, 2000");
    let ruthless = modded("ruthless-lord", &rows);
    assert_eq!(ruthless.ai.personality[0].tax_ladder, 0);
    assert_eq!(
        ruthless.ai.tax_ladders,
        Tables::DEFAULT.ai.tax_ladders,
        "the ladders themselves are untouched: only which one lord 1 walks changed"
    );

    // At 85 happiness the gentle ladder charges 3% and the greedy one 15%.
    let (stock_rate, stock_gold) = an_ai_seasons_tax(Tables::DEFAULT, 85);
    let (mod_rate, mod_gold) = an_ai_seasons_tax(ruthless, 85);
    assert_eq!((stock_rate, mod_rate), (3, 15));
    assert_eq!(mod_gold, stock_gold * 5, "five times the rate, five times the take");
}

/// The two ways one of these new rules can be impossible rather than merely
/// unbalanced, both refused at load with the file and line that wrote them.
///
/// `ale_step_pct` divides the population, so zero is a division by zero in
/// `buy_ale` rather than "ale is free"; a ninth rung is a ladder the
/// simulation's array cannot hold. Neither is clamped, because a clamped rule
/// is one a mod author cannot see failed.
#[test]
fn an_impossible_new_kingdom_rule_is_refused_with_the_line_that_wrote_it() {
    let refusal = |name: &'static str, rules: &str| -> String {
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
            .expect("the document is well formed, so the load itself succeeds")
            .kingdom_tables()
            .expect_err("but the rule cannot be run")
            .to_string()
    };

    let err = refusal("free-ale", "[kingdom.happiness]\nale_step_pct = 0\n");
    assert!(err.contains("free-ale:rules/free-ale.toml:2:16"), "{err}");
    assert!(err.contains("kingdom.happiness.ale_step_pct"), "{err}");
    assert!(err.contains("0 is outside 1..=10000"), "{err}");

    let mut ninth = String::new();
    for rung in 0..9 {
        ninth.push_str(&format!("[[kingdom.ai.tax_ladder.0]]\nbelow = {}\nrate = 1\n\n", rung * 10));
    }
    let err = refusal("long-ladder", &ninth);
    assert!(err.contains("kingdom.ai.tax_ladder.0"), "{err}");
    assert!(err.contains("1 to 8 rungs, found 9"), "{err}");
}
