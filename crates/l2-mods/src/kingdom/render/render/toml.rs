#![allow(unused_imports)]
use super::*;

use super::*;
use super::*;
use super::tables::*;
use crate::ruleset::{RuleError, Ruleset};
use l2_kingdom::tables::{
    AiPersonalityRow, AiTable, AleTable, CastleTable, CommodityRow, EfficiencyTable, EventTable,
    FieldTable, FoodTable, GoodRow, GrainTable, HealthBandRow, HerdCrowdingRow, HerdTable,
    JobTable, PopulationTable, RationRow, ScoreTable, SeasonRow, Tables, TaxLadder, WageTable,
    WeaponRow, WeatherRow, AI_PERSONALITY_COUNT, AI_TAX_LADDER_COUNT, ARMY_HAPPINESS_COST_LEN,
    CASTLE_TYPE_COUNT, HERD_CROWDING_COUNT, TAX_LADDER_RUNGS,
};

pub fn render_toml(t: &Tables) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    out.push_str(
        "# The kingdom economy: tax, happiness, rations, health, births,\n\
         # deaths, yields, industry, castles, wages, trade and score.\n\
         #\n\
         # GENERATED, NOT AUTHORED. Rendered from l2_kingdom::tables::Tables\n\
         # ::DEFAULT, with a test asserting the two agree. Editing it here\n\
         # will fail the build; put your change in a mod instead.\n\
         #\n\
         # WHY THIS FILE IS THE POINT OF THE PROJECT\n\
         #\n\
         # docs/decisions.md C11: every one of these numbers lives inside\n\
         # Lords2.exe, clustered around 0x004D6300 and 0x004D8900, and in no\n\
         # data file anywhere. TROOPS*.ENG set an expectation that rules would\n\
         # be reachable as data; they are not. Modding the original's economy\n\
         # means patching a 1996 binary. This file is the first time these\n\
         # numbers have been text.\n\
         #\n\
         # Two conventions run through it:\n\
         #\n\
         #   * Named rows carry the array index they stand for, and it is\n\
         #     checked. The names are for you; the indices are what the\n\
         #     simulation uses, and they cannot be reordered.\n\
         #   * Ladders are arrays of tables, tried in order, and the last row\n\
         #     is the catch-all. Arrays replace whole on merge, so a mod that\n\
         #     changes one rung restates the ladder.\n\
         #\n\
         # HONEST SCOPE: every rule in this file is read by the simulation. A\n\
         # mod that changes one changes what a county harvests, mines, pays or\n\
         # feels, and crates/l2-mods/tests/simulation/main.rs proves that rule by\n\
         # rule by running the season pipeline twice. The one exception is\n\
         # named where it appears: kingdom.ai.personality.*.farm_style loads\n\
         # and nothing reads it. See docs/modding.md sec 11.\n",
    );

    let _ = write!(
        out,
        "\n# --- food ------------------------------------------------------------\n\
         # A standing herd feeds five per head per season without being\n\
         # slaughtered; a slaughtered animal feeds ten; a sack feeds six.\n\
         \n[kingdom.food]\n\
         dairy_per_head = {}\n\
         food_per_head = {}\n\
         food_per_sack = {}\n",
        t.food.dairy_per_head, t.food.food_per_head, t.food.food_per_sack
    );

    let _ = write!(
        out,
        "\n# --- grain and fields ------------------------------------------------\n\
         # max_sacks_per_field is 10. The printed manual says 5, twice, and the\n\
         # manual is wrong - docs/decisions.md C10.\n\
         # The labour divisors run the other way round than they look: the\n\
         # SMALLER divisor demands MORE labour, so turning Advanced Farming off\n\
         # makes sowing harder.\n\
         \n[kingdom.grain]\n\
         yield_per_sack = {}\n\
         max_sacks_per_field = {}\n\
         labour_divisor_advanced = {}\n\
         labour_divisor_basic = {}\n\
         # ...and these two are MULTIPLIERS, on the same line of the same\n\
         # branch: Grain_Grow and Grain_Harvest cap the standing crop at\n\
         # labour * this. With Advanced Farming off all three read\n\
         # labour_divisor_basic, which is one global in the original.\n\
         grow_per_worker_advanced = {}\n\
         harvest_per_worker_advanced = {}\n\
         \n[kingdom.field]\n\
         progress_max = {}\n\
         reclaim_per_season = {}\n",
        t.grain.yield_per_sack,
        t.grain.max_sacks_per_field,
        t.grain.labour_divisor_advanced,
        t.grain.labour_divisor_basic,
        t.grain.grow_per_worker_advanced,
        t.grain.harvest_per_worker_advanced,
        t.field.progress_max,
        t.field.reclaim_per_season
    );

    let _ = write!(
        out,
        "\n# --- random events ---------------------------------------------------\n\
         \n[kingdom.event]\n\
         population_cap_pct = {}\n\
         first_year = {}\n",
        t.event.population_cap_pct, t.event.first_year
    );

    out.push_str(
        "\n# --- seasons ---------------------------------------------------------\n\
         # death_rate is a percentage added to the health death rate; dryness is\n\
         # the seasonal push on the per-county dryness accumulator. Slot 0 is\n\
         # the original's \"No Season\" and is never used.\n",
    );
    for (index, id) in SEASON_IDS.iter().enumerate() {
        let row = t.season[index];
        let _ = write!(
            out,
            "\n[kingdom.season.{id}]\nindex = {index}\ndeath_rate = {}\ndryness = {}\n",
            row.death_rate, row.dryness
        );
    }

    let _ = write!(
        out,
        "\n# --- rations ---------------------------------------------------------\n\
         # Food needed is ceil(people / divisor) * multiplier.\n\
         # health_delta is added to the health meter each season, by health\n\
         # band, in the band order below. Its last column is the rule players\n\
         # feel: perfect health decays under anything less than Double.\n\
         #\n\
         # ration_slope and ration_offset give the happiness a ration level is\n\
         # worth, as slope * level + offset. In the original this is not a\n\
         # table at all - it is the expression 3L - 8 at the end of\n\
         # Ration_Apply.\n\
         #\n\
         # ale_step_pct and ale_max are the ale ladder: one happiness per that\n\
         # percentage of the county's population in crowns, up to ale_max. The\n\
         # published figure is +1 per 20%; the binary divides by 10, so it is\n\
         # +1 per 10%. ale_max is BOTH the top rung and a cumulative cap that\n\
         # nothing ever resets - five happiness from ale per county for the\n\
         # whole game, not five a season.\n\
         #\n\
         # army_cost is what raising men costs the county, indexed by the\n\
         # PERCENTAGE of its population taken, not by the number of men. 102\n\
         # rows, 0..=101, and the count is fixed: in the original the row after\n\
         # the last is the first merchant price.\n\
         \n[kingdom.happiness]\n\
         ration_slope = {}\n\
         ration_offset = {}\n\
         ale_step_pct = {}\n\
         ale_max = {}\n\
         army_cost = [\n{}\n]\n",
        t.ration_happiness_slope,
        t.ration_happiness_offset,
        t.ale.step_pct,
        t.ale.max,
        wrap_i32(&t.army_happiness_cost, 16)
    );
    for (index, id) in RATION_IDS.iter().enumerate() {
        let row = t.ration[index];
        let _ = write!(
            out,
            "\n[kingdom.ration.{id}]\nindex = {index}\ndivisor = {}\nmultiplier = {}\n\
             health_delta = [{}]\n",
            row.divisor,
            row.multiplier,
            join_i32(&row.health_delta)
        );
    }

    let _ = write!(
        out,
        "\n# --- health ----------------------------------------------------------\n\
         # band_ladder holds the INCLUSIVE upper bound of each band, one entry\n\
         # per band. A meter of 65 bands as 2, which is what pins the comparison\n\
         # as <= rather than <. The last entry is a real bound in the game's own\n\
         # table rather than an `else`: five bands, five bounds.\n\
         \n[kingdom.health]\n\
         band_ladder = [{}]\n",
        join_i32(&t.health_band_ladder.map(|(bound, _)| bound))
    );
    for (index, id) in HEALTH_BAND_IDS.iter().enumerate() {
        let row = t.health[index];
        let _ = write!(
            out,
            "\n[kingdom.health.band.{id}]\nindex = {index}\nhappiness = {}\ndeath_rate = {}\n",
            row.happiness, row.death_rate
        );
    }

    out.push_str(
        "\n# --- population ------------------------------------------------------\n\
         # birth_rate: the first row whose up_to is at least the county's\n\
         # population gives the base birth rate. This is the soft population\n\
         # cap players describe - at 2,000 people the 2% birth rate cannot keep\n\
         # up with winter's 8% death rate.\n",
    );
    for &(up_to, percent) in &t.population.birth_rate_ladder {
        let _ = write!(
            out,
            "\n[[kingdom.population.birth_rate]]\nup_to = {up_to}\npercent = {percent}\n"
        );
    }
    out.push_str(
        "\n# happiness_factor scales the birth rate. The last row is the\n\
         # catch-all and its threshold is never read; it is written as i32::MAX\n\
         # so that a reader which does compare it still gets the right answer.\n",
    );
    for &(below, percent) in &t.population.happiness_factor_ladder {
        let _ = write!(
            out,
            "\n[[kingdom.population.happiness_factor]]\nbelow = {below}\npercent = {percent}\n"
        );
    }

    out.push_str(
        "\n# --- weather ---------------------------------------------------------\n\
         # The percentage swing weather puts on the herd. The signs match\n\
         # L2.eng group 66's own descriptions one for one: only Sunny boosts\n\
         # crops, only Cloudy is neutral.\n",
    );
    for (index, id) in WEATHER_IDS.iter().enumerate() {
        let _ = write!(
            out,
            "\n[kingdom.weather.{id}]\nindex = {index}\nherd_pct = {}\n",
            t.weather[index].herd_pct
        );
    }

    let _ = write!(
        out,
        "\n# --- tax -------------------------------------------------------------\n\
         # happiness_other is county +0x16, the \"Other counties\" term: what one\n\
         # county's tax rate does to the mood of every OTHER county in the same\n\
         # realm. One row per tax rate, so the array's length IS the tax\n\
         # ceiling - 51 rows means 0 to 50, and the panel's up arrow stops\n\
         # there. Flat zero to rate 19: taxing at 19%% costs the rest of the\n\
         # realm nothing at all.\n\
         # The local half of the term, 5 - rate, is arithmetic rather than a\n\
         # table and is not here.\n\
         \n[kingdom.tax]\n\
         happiness_other = [\n{}\n]\n",
        wrap_i32(&t.tax_happiness_other, 17)
    );

    let _ = write!(
        out,
        "\n# --- the herd --------------------------------------------------------\n\
         # Cattle need tending. labour_per_head is 3: staffing is\n\
         # PctOf(labour, herd * 3), capped at staffing_max, and every point\n\
         # BELOW 100 adds a third of a point to the death rate. A herd nobody\n\
         # works loses about a third of a percent of its head a season on top\n\
         # of crowding.\n\
         # A county with no pasture at all loses half its herd - or all of it\n\
         # below no_pasture_kill_all_below head - and nothing else applies.\n\
         # calving_season multiplies births by season_bonus and culling_season\n\
         # multiplies deaths; 0 is \"No Season\" and turns the bonus off.\n\
         \n[kingdom.herd]\n\
         labour_per_head = {}\n\
         staffing_max = {}\n\
         understaffing_divisor = {}\n\
         no_pasture_density = {}\n\
         no_pasture_kill_all_below = {}\n\
         no_pasture_divisor = {}\n\
         calving_season = {}\n\
         culling_season = {}\n\
         season_bonus_numerator = {}\n\
         season_bonus_denominator = {}\n",
        t.herd.labour_per_head,
        t.herd.staffing_max,
        t.herd.understaffing_divisor,
        t.herd.no_pasture_density,
        t.herd.no_pasture_kill_all_below,
        t.herd.no_pasture_divisor,
        t.herd.calving_season,
        t.herd.culling_season,
        t.herd.season_bonus.0,
        t.herd.season_bonus.1
    );
    out.push_str(
        "\n# Crowding is head per pasture field, banded. level is what the\n\
         # county stores and what L2.eng group 77 names - \"Low herd crowding.\"\n\
         # through \"Massive overcrowding!!\" - and the two rates are per ten\n\
         # thousand head. An overcrowded herd dies seven times as fast AND\n\
         # breeds a seventh as often, which is why a county cannot simply keep\n\
         # buying cattle. The last row is the catch-all.\n",
    );
    for row in &t.herd.crowding {
        let _ = write!(
            out,
            "\n[[kingdom.herd.crowding]]\ndensity_max = {}\nlevel = {}\n\
             death_rate = {}\nbirth_rate = {}\n",
            row.density_max, row.level, row.death_rate, row.birth_rate
        );
    }
    out.push_str(
        "\n# A small, FULLY STAFFED herd breeds faster, which is what lets a\n\
         # county that has lost almost everything recover. Tried in order.\n",
    );
    for &(below, bonus) in &t.herd.small_bonus {
        let _ = write!(out, "\n[[kingdom.herd.small_bonus]]\nbelow = {below}\nbonus = {bonus}\n");
    }

    let _ = write!(
        out,
        "\n# --- castles ---------------------------------------------------------\n\
         # tax_base is what Tax_CollectAll multiplies the population by. In the\n\
         # original these six are immediates in the instruction stream, not a\n\
         # table. Type 0 is \"no castle\" and has nothing to build.\n\
         \n[kingdom.castle]\n\
         starting_type = {}\n",
        t.castle.starting_type
    );
    for (index, id) in CASTLE_IDS.iter().enumerate() {
        let _ = write!(
            out,
            "\n[kingdom.castle.type.{id}]\nindex = {index}\ntax_base = {}\n",
            t.castle.tax_base[index]
        );
        if index == 0 {
            continue;
        }
        let b = index - 1;
        let _ = write!(
            out,
            "tax_bonus_pct = {}\ncost_wood = {}\ncost_stone = {}\nworkforce = {}\n\
             garrison_cap = {}\nfree_archers = {}\n",
            t.castle.tax_bonus_pct[b],
            t.castle.cost[b].0,
            t.castle.cost[b].1,
            t.castle.workforce[b].0,
            t.castle.garrison_cap[b],
            t.castle.free_archers[b]
        );
    }

    let _ = write!(
        out,
        "\n# --- jobs and industry -----------------------------------------------\n\
         # Six of the nine job slots are established; castle_building is NOT.\n\
         # docs/kingdom.md never says which slot holds it, and the value is\n\
         # this engine's placeholder so the rule can be written and tested at\n\
         # all. cattle_farming IS established: Herd_SeasonTick passes county\n\
         # +0xD0, which is record 1, and the England turn-one fixture's\n\
         # nine labour records sum to the county's population in all\n\
         # fourteen counties.\n\
         \n[kingdom.job]\n\
         count = {}\n\
         iron_mining = {}\n\
         stone_quarrying = {}\n\
         wood_cutting = {}\n\
         blacksmith = {}\n\
         grain_farming = {}\n\
         cattle_farming = {}\n\
         castle_building = {}\n",
        t.job.count,
        t.job.iron_mining,
        t.job.stone_quarrying,
        t.job.wood_cutting,
        t.job.blacksmith,
        t.job.grain_farming,
        t.job.cattle_farming,
        t.job.castle_building
    );
    let _ = write!(
        out,
        "\n# The efficiency ramp's two bounds. Efficiency COMPOUNDS season on\n\
         # season towards max, so an industry is worth more the longer it has\n\
         # run and a county that is conquered and restarted is not. With\n\
         # Advanced Farming off none of that happens and every industry sits\n\
         # flat at without_advanced_farming - which is the England turn-one\n\
         # fixture's setting, so it is the ramp most games actually see.\n\
         \n[kingdom.efficiency]\n\
         max = {}\n\
         without_advanced_farming = {}\n",
        t.efficiency.max, t.efficiency.without_advanced_farming
    );

    out.push_str(
        "\n# The four passes Industry_Produce makes per county per season, in\n\
         # the order it makes them. divisor is applied to the worker count\n\
         # before the efficiency percentage - iron and wood harvest at twice\n\
         # the quantity of stone, which is exactly this column.\n",
    );
    for (index, id) in COMMODITY_IDS.iter().enumerate() {
        let row = t.commodity[index];
        let _ = write!(
            out,
            "\n[kingdom.commodity.{id}]\nindex = {index}\njob = {}\ndivisor = {}\n\
             base_efficiency = {}\n",
            row.job, row.divisor, row.base_efficiency
        );
    }

    out.push_str(
        "\n# --- weapons ---------------------------------------------------------\n\
         # Wood and iron to forge one. A bow needs no iron; armour costs the\n\
         # most of anything.\n",
    );
    for (index, id) in WEAPON_IDS.iter().enumerate() {
        let row = t.weapon[index];
        let _ = write!(
            out,
            "\n[kingdom.weapon.{id}]\nindex = {index}\nwood = {}\niron = {}\n",
            row.wood, row.iron
        );
    }

    out.push_str(
        "\n# --- trade -----------------------------------------------------------\n\
         # The merchant's base SELL price. The buy price the scroll shows is\n\
         # twice this. Sheep and wool are priced zero because they are the two\n\
         # goods a county cannot produce or trade in the base game. Slot 0 is\n\
         # not a good.\n",
    );
    for (index, id) in GOOD_IDS.iter().enumerate() {
        let _ = write!(
            out,
            "\n[kingdom.good.{id}]\nindex = {index}\nsell_price = {}\n",
            t.good[index].sell_price
        );
    }

    let _ = write!(
        out,
        "\n# --- wages -----------------------------------------------------------\n\
         # A human pays men / 4. Troop type does not enter it: a knight and a\n\
         # peasant cost the same. divisor_ai is by AI difficulty.\n\
         \n[kingdom.wages]\n\
         divisor_human = {}\n\
         divisor_ai = [{}]\n\
         bankrupt_stage_max = {}\n",
        t.wages.divisor_human,
        join_i32(&t.wages.divisor_ai),
        t.wages.bankrupt_stage_max
    );

    let _ = write!(
        out,
        "\n# --- the AI's advantages ---------------------------------------------\n\
         # Row 0 being all zeros is the load-bearing part: the human's lord\n\
         # byte is 0, so the human gets nothing. Rows 1..3 are NOT documented\n\
         # and are zeroed here rather than invented - a known gap, not a rule.\n\
         \n[kingdom.ai]\n\
         grant_population_per_difficulty = {}\n\
         grant_herd_per_difficulty = {}\n\
         grant_grain_per_difficulty = {}\n\
         grant_min_population = {}\n\
         grant_min_herd = {}\n\
         grant_min_grain = {}\n",
        t.ai.grant_population_per_difficulty,
        t.ai.grant_herd_per_difficulty,
        t.ai.grant_grain_per_difficulty,
        t.ai.grant_min_population,
        t.ai.grant_min_herd,
        t.ai.grant_min_grain
    );
    for (lord, row) in t.ai.gold_grant.iter().enumerate() {
        let _ = write!(
            out,
            "\n[[kingdom.ai.gold_grant]]\nlord = {lord}\nby_difficulty = [{}]\n",
            join_i32(row)
        );
    }

    out.push_str(
        "\n# The four tax ladders. In the original these are not tables at all -\n\
         # they are four if/else-if chains inside AI_SetTaxRates, which is why\n\
         # docs/kingdom.md sec 8.2 says the ladders exist and does not give\n\
         # them. Each row is `happiness below this -> tax this rate`, walked in\n\
         # order; the last row is the catch-all and its threshold is never\n\
         # read. A ladder may state 1 to 8 rows and the last is repeated to\n\
         # fill, so only the neutral ladder writes all eight.\n\
         #\n\
         # tax_ladder_neutral is what unowned counties pay, set once a turn in\n\
         # phase 1. Note it charges 1% at 20 happiness where every lord's own\n\
         # ladder charges nothing below 30 - nobody collects it, though:\n\
         # Tax_CollectAll banks an unowned county's take into the county.\n",
    );
    for &(below, rate) in tax_ladder_rows(&t.ai.tax_ladder_neutral) {
        let _ = write!(out, "\n[[kingdom.ai.tax_ladder_neutral]]\nbelow = {below}\nrate = {rate}\n");
    }
    out.push_str(
        "\n# The three an AI realm picks between, by its lord's personality.\n\
         # 0 is the greediest - 15% on a happy county. 2 is the gentlest, the\n\
         # only one charging nothing below 60, and the one three lords use.\n",
    );
    for (i, ladder) in t.ai.tax_ladders.iter().enumerate() {
        for &(below, rate) in tax_ladder_rows(ladder) {
            let _ =
                write!(out, "\n[[kingdom.ai.tax_ladder.{i}]]\nbelow = {below}\nrate = {rate}\n");
        }
    }

    out.push_str(
        "\n# One personality record per AI lord, in lord order.\n\
         #\n\
         # farm_style NOW TAKES EFFECT. It used to load and do nothing: the\n\
         # note here said AI_ManageFields dispatched into \"one of three labour\n\
         # allocators that were never traced\". There are FIVE allocators, two\n\
         # outer passes and two different functions with those two names -\n\
         # Ai_ManageCountyFarms at 0x0049DD01 for an AI realm and\n\
         # AI_ManageFields at 0x0049DFC6 for the unowned counties. All five are\n\
         # implemented; see l2_kingdom::ai_farm. 0 is an arable lord, 1 a\n\
         # grazier, 9 a mixer; anything else farms nothing.\n\
         #\n\
         # weapon_rota is the lord's weapon programme, six weapon types stepped\n\
         # through by AI turn step 12. Values index the weapon table: 0\n\
         # crossbow, 1 mace, 2 sword, 3 pike, 4 bow, 5 armour. The ten-step\n\
         # cursor visits slots 0,1,2,3,0,1,2,3,4,5, so the first four are made\n\
         # twice as often as the last two.\n\
         #\n\
         # There are four records and not five: docs/kingdom.md sec 2 says the\n\
         # lord byte runs 1..5, but a fifth record's bytes read a farm style of\n\
         # 17 where every real one reads 0, 1 or 9. A realm whose lord names no\n\
         # record sets no tax rates at all.\n\
         #\n\
         # siege_doctrine is record +0xA0 and it DOES take effect: 8 orders four\n\
         # siege towers, 9 a battering ram, 7 three catapults and a late ram,\n\
         # and anything else leaves the default of two towers. The orders are\n\
         # cumulative, so 7 and 9 also get the two towers.\n\
         #\n\
         # The five war fields are records +0x28, +0x68, +0x70, +0x74 and\n\
         # +0x9C, and l2_kingdom::ai_army reads all five:\n\
         #   muster_patience         turns between musters when there is no war\n\
         #   muster_arms             the weapon stock wanted before mustering\n\
         #   garrison_min_population the county size a castle garrison needs\n\
         #   raid_interval           turns between raids\n\
         #   abandon_tax_rate        the tax put on a county being written off\n\
         # docs/diplomacy.md sec 8.4 listed the last three as never traced.\n",
    );
    for (i, row) in t.ai.personality.iter().enumerate() {
        let _ = write!(
            out,
            "\n[[kingdom.ai.personality]]\nlord = {}\nfarm_style = {}\ntax_ladder = {}\n\
             gift_increment = {}\nhelp_price = {}\ngrudge_tolerance = {}\n\
             offer_interval = {}\nhelp_population_floor = {}\nmuster_pct = {}\n\
             muster_patience = {}\nmuster_arms = {}\ngarrison_min_population = {}\n\
             raid_interval = {}\nabandon_tax_rate = {}\n\
             castle_concurrent = {}\ncastle_min_population = {}\ncastle_gold = [{}]\n\
             weapon_rota = [{}]\nsiege_doctrine = {}\n\
             trade_gold_floor = {}\nweapon_buy_qty = {}\nreserve_wood = {}\n\
             reserve_stone = {}\nreserve_iron = {}\n",
            i + 1,
            row.farm_style,
            row.tax_ladder,
            row.gift_increment,
            row.help_price,
            row.grudge_tolerance,
            row.offer_interval,
            row.help_population_floor,
            row.muster_pct,
            row.muster_patience,
            row.muster_arms,
            row.garrison_min_population,
            row.raid_interval,
            row.abandon_tax_rate,
            row.castle_concurrent,
            row.castle_min_population,
            join_i32(&row.castle_gold),
            row.weapon_rota.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(", "),
            row.siege_doctrine,
            row.trade_gold_floor,
            row.weapon_buy_qty,
            row.reserve_wood,
            row.reserve_stone,
            row.reserve_iron
        );
    }

    out.push_str(
        "\n# --- score -----------------------------------------------------------\n\
         # The gold brackets are the one term of the score whose meaning is\n\
         # unambiguous. The six weights apply to six realm fields NONE of which\n\
         # docs/kingdom.md could identify, so they are named by their raw\n\
         # offset rather than given invented names.\n",
    );
    for &(at_least, points) in &t.score.gold_brackets {
        let _ = write!(
            out,
            "\n[[kingdom.score.gold_bracket]]\nat_least = {at_least}\npoints = {points}\n"
        );
    }
    for i in 0..t.score.weights.len() {
        let (numerator, denominator) = t.score.weights[i];
        let _ = write!(
            out,
            "\n[[kingdom.score.weight]]\noffset = 0x{:02X}\nnumerator = {numerator}\n\
             denominator = {denominator}\n",
            t.score.input_offsets[i]
        );
    }

    out
}

