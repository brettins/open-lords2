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
    k.options = Options { difficulty: 0, advanced_farming: false, armies_eat: false };
    assert!(k.set_county_count(1));
    k.realms[1].in_play = true;
    k.realms[1].is_human = true;
    k.realms[1].county_count = 1;

    let c = &mut k.counties[1];
    c.owner = 1;
    c.population = 400;
    c.happiness = 65;
    c.health_meter = 65;
    c.health_band = health_band(65);
    c.herd = 400;
    c.grain = 100;
    c.fields_grain = 5;
    c.ration_wanted = 3; // Normal
    c.ration_split = 100; // fed entirely from the herd
    c.labour = [100_000; l2_kingdom::tables::JOB_COUNT];

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
    assert_eq!(
        a_years_harvest(forward),
        a_years_harvest(backward) * 4,
        "24 against 6 is four times the harvest"
    );
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
