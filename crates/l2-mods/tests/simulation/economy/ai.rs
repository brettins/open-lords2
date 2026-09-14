#![allow(unused_imports)]
use super::*;
use super::harvest::*;
use super::rules::*;
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

/// An AI realm taxing one county for a season. Returns the rate its ladder
/// chose
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
    // cascade is refused
    k.run_ai_farms(1, &mut l2_kingdom::ai_farm::NoMarket);
    l2_kingdom::field::recount(&mut k.counties[1], &k.campaign.map);
    (k.counties[1].fields_grain, k.counties[1].fields_cattle, k.counties[1].industry_share)
}

/// **`farm_style` is a rule a mod sets, and it changes the map.**
#[test]
fn which_way_an_ai_lord_farms_is_a_rule_a_mod_sets() {
    // Lord 1 ships as style 1, a grazier: no grain at all
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
         help_population_floor = 750\nmuster_pct = 30\nmuster_patience = 3\nmuster_arms = 100\n\
         garrison_min_population = 300\nraid_interval = 6\nabandon_tax_rate = 32\n\
         castle_concurrent = 4\n\
         castle_min_population = 700\ncastle_gold = [200, 0, 1000, 0, 10000]\n\
         weapon_rota = [0, 1, 4, 2, 5, 2]\nsiege_doctrine = 8\n\
         trade_gold_floor = 1000\nweapon_buy_qty = 100\n\
         reserve_wood = 250\nreserve_stone = 250\nreserve_iron = 250\n\n\
         [[kingdom.ai.personality]]\nlord = 2\nfarm_style = 1\ntax_ladder = 2\n\
         gift_increment = 100\nhelp_price = 1000\ngrudge_tolerance = 10\noffer_interval = 10\n\
         help_population_floor = 800\nmuster_pct = 30\nmuster_patience = 4\nmuster_arms = 120\n\
         garrison_min_population = 300\nraid_interval = 10\nabandon_tax_rate = 28\n\
         castle_concurrent = 3\n\
         castle_min_population = 650\ncastle_gold = [0, 500, 0, 4000, 0]\n\
         weapon_rota = [3, 5, 4, 4, 4, 5]\nsiege_doctrine = 9\n\
         trade_gold_floor = 1500\nweapon_buy_qty = 80\n\
         reserve_wood = 300\nreserve_stone = 300\nreserve_iron = 300\n\n\
         [[kingdom.ai.personality]]\nlord = 3\nfarm_style = 0\ntax_ladder = 2\n\
         gift_increment = 200\nhelp_price = 1600\ngrudge_tolerance = 15\noffer_interval = 8\n\
         help_population_floor = 900\nmuster_pct = 40\nmuster_patience = 4\nmuster_arms = 200\n\
         garrison_min_population = 250\nraid_interval = 5\nabandon_tax_rate = 23\n\
         castle_concurrent = 2\n\
         castle_min_population = 600\ncastle_gold = [0, 300, 0, 2000, 0]\n\
         weapon_rota = [0, 1, 1, 2, 4, 0]\nsiege_doctrine = 7\n\
         trade_gold_floor = 2500\nweapon_buy_qty = 70\n\
         reserve_wood = 500\nreserve_stone = 500\nreserve_iron = 500\n\n\
         [[kingdom.ai.personality]]\nlord = 4\nfarm_style = 9\ntax_ladder = 1\n\
         gift_increment = 50\nhelp_price = 1500\ngrudge_tolerance = 20\noffer_interval = 4\n\
         help_population_floor = 1000\nmuster_pct = 50\nmuster_patience = 2\nmuster_arms = 250\n\
         garrison_min_population = 150\nraid_interval = 10\nabandon_tax_rate = 35\n\
         castle_concurrent = 1\n\
         castle_min_population = 600\ncastle_gold = [0, 0, 100, 0, 2000]\n\
         weapon_rota = [4, 4, 3, 4, 4, 3]\nsiege_doctrine = 7\n\
         trade_gold_floor = 4000\nweapon_buy_qty = 150\n\
         reserve_wood = 1000\nreserve_stone = 1000\nreserve_iron = 1000\n",
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
    // 10,000,
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
             muster_patience = 3\nmuster_arms = 100\ngarrison_min_population = 300\n\
             raid_interval = 6\nabandon_tax_rate = 32\n\
             castle_concurrent = {concurrent}\ncastle_min_population = {min_pop}\n\
             castle_gold = [{gold}]\nweapon_rota = [0, 1, 2, 3, 4, 5]\n\
             siege_doctrine = 8\ntrade_gold_floor = 1000\nweapon_buy_qty = 100\n\
             reserve_wood = 250\nreserve_stone = 250\nreserve_iron = 250\n\n"
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

    // At 85 happiness the gentle ladder charges 3%
    let (stock_rate, stock_gold) = an_ai_seasons_tax(Tables::DEFAULT, 85);
    let (mod_rate, mod_gold) = an_ai_seasons_tax(ruthless, 85);
    assert_eq!((stock_rate, mod_rate), (3, 15));
    assert_eq!(mod_gold, stock_gold * 5, "five times the rate, five times the take");
}

