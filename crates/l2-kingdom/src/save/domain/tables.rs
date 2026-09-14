#![allow(unused_imports)]
use super::*;
use super::unit::*;
use super::county::*;
use super::industry::*;
use super::realm::*;
use super::history::*;
use super::*;
use super::codec::*;
use crate::county::{ChangeReason, County, Industry, MAX_COUNTIES, MAX_INFLOW_SOURCES, MAX_NEIGHBOURS};
use crate::kingdom::{History, HistoryEntry, Kingdom, Options};
use crate::phase::{Phase, TurnMachine};
use crate::realm::{Realm, MAX_REALMS};
use crate::tables::{
    Tables, Weather, HISTORY_COUNTIES, HISTORY_SEASONS, JOB_COUNT, WEAPON_TYPE_COUNT,
};
use l2_net::canonical::{Canonical, CodecError, Decode, Encode, Reader};

/// The whole of [`Tables`], written out so it can be hashed.
///
/// Encode only: the save does not carry a ruleset, it carries this hash. Every
/// field is here, and `tests/save.rs` mutates each sub-table in turn to check
/// that the hash notices — which is the guard against a constant being added to
/// `Tables` and quietly left out of the fingerprint.
impl Encode for Tables {
    fn encode(&self, out: &mut Canonical) {
        out.section("food");
        out.i32(self.food.dairy_per_head);
        out.i32(self.food.food_per_head);
        out.i32(self.food.food_per_sack);

        out.section("grain");
        out.i32(self.grain.yield_per_sack);
        out.i32(self.grain.max_sacks_per_field);
        out.i32(self.grain.labour_divisor_advanced);
        out.i32(self.grain.labour_divisor_basic);

        out.section("field");
        out.i32(self.field.progress_max);
        out.i32(self.field.reclaim_per_season);

        out.section("event");
        out.i32(self.event.population_cap_pct);
        out.i32(self.event.first_year);

        out.section("season");
        for row in &self.season {
            out.i32(row.death_rate);
            out.i32(row.dryness);
        }

        out.section("ration");
        for row in &self.ration {
            out.i32(row.divisor);
            out.i32(row.multiplier);
            for delta in &row.health_delta {
                out.i32(*delta);
            }
        }
        out.i32(self.ration_happiness_slope);
        out.i32(self.ration_happiness_offset);

        out.section("health");
        for row in &self.health {
            out.i32(row.happiness);
            out.i32(row.death_rate);
        }
        for (threshold, band) in &self.health_band_ladder {
            out.i32(*threshold);
            out.i32(*band);
        }

        out.section("population");
        for (up_to, percent) in &self.population.birth_rate_ladder {
            out.i32(*up_to);
            out.i32(*percent);
        }
        for (below, percent) in &self.population.happiness_factor_ladder {
            out.i32(*below);
            out.i32(*percent);
        }

        out.section("tax");
        for v in &self.tax_happiness_other {
            out.i32(*v);
        }

        out.section("weather");
        for row in &self.weather {
            out.i32(row.herd_pct);
        }

        out.section("herd");
        out.i32(self.herd.labour_per_head);
        out.i32(self.herd.staffing_max);
        out.i32(self.herd.understaffing_divisor);
        for row in &self.herd.crowding {
            out.i32(row.density_max);
            out.i32(row.level);
            out.i32(row.death_rate);
            out.i32(row.birth_rate);
        }
        for (below, bonus) in &self.herd.small_bonus {
            out.i32(*below);
            out.i32(*bonus);
        }
        out.i32(self.herd.no_pasture_density);
        out.i32(self.herd.no_pasture_kill_all_below);
        out.i32(self.herd.no_pasture_divisor);
        out.u8(self.herd.calving_season);
        out.u8(self.herd.culling_season);
        out.i32(self.herd.season_bonus.0);
        out.i32(self.herd.season_bonus.1);

        out.section("castle");
        out.u8(self.castle.starting_type);
        for v in &self.castle.tax_base {
            out.i32(*v);
        }
        for v in &self.castle.tax_bonus_pct {
            out.i32(*v);
        }
        for (wood, stone) in &self.castle.cost {
            out.i32(*wood);
            out.i32(*stone);
        }
        for (a, b) in &self.castle.workforce {
            out.i32(*a);
            out.i32(*b);
        }
        for v in &self.castle.garrison_cap {
            out.i32(*v);
        }
        for v in &self.castle.free_archers {
            out.i32(*v);
        }

        out.section("commodity");
        for row in &self.commodity {
            out.u32(row.job as u32);
            out.i32(row.divisor);
            out.i32(row.base_efficiency);
        }

        out.section("job");
        out.u32(self.job.count as u32);
        out.u32(self.job.iron_mining as u32);
        out.u32(self.job.stone_quarrying as u32);
        out.u32(self.job.wood_cutting as u32);
        out.u32(self.job.blacksmith as u32);
        out.u32(self.job.grain_farming as u32);
        out.u32(self.job.cattle_farming as u32);
        out.u32(self.job.castle_building as u32);

        out.section("weapon");
        for row in &self.weapon {
            out.i32(row.wood);
            out.i32(row.iron);
        }

        out.section("good");
        for row in &self.good {
            out.i32(row.sell_price);
        }

        out.section("wages");
        out.i32(self.wages.divisor_human);
        for v in &self.wages.divisor_ai {
            out.i32(*v);
        }
        out.u8(self.wages.bankrupt_stage_max);

        out.section("efficiency");
        out.i32(self.efficiency.max);
        out.i32(self.efficiency.without_advanced_farming);

        out.section("ale");
        out.i32(self.ale.step_pct);
        out.i32(self.ale.max);

        out.section("army_happiness");
        for v in &self.army_happiness_cost {
            out.i32(*v);
        }

        out.section("ai");
        for row in &self.ai.gold_grant {
            for v in row {
                out.i32(*v);
            }
        }
        out.i32(self.ai.grant_population_per_difficulty);
        out.i32(self.ai.grant_herd_per_difficulty);
        out.i32(self.ai.grant_grain_per_difficulty);
        out.i32(self.ai.grant_min_population);
        out.i32(self.ai.grant_min_herd);
        out.i32(self.ai.grant_min_grain);
        for (threshold, rate) in &self.ai.tax_ladder_neutral {
            out.i32(*threshold);
            out.i32(*rate);
        }
        for ladder in &self.ai.tax_ladders {
            for (threshold, rate) in ladder {
                out.i32(*threshold);
                out.i32(*rate);
            }
        }
        for row in &self.ai.personality {
            out.u8(row.farm_style);
            out.u32(row.tax_ladder as u32);
        }

        out.section("score");
        for (at_least, points) in &self.score.gold_brackets {
            out.i32(*at_least);
            out.i32(*points);
        }
        for (numerator, denominator) in &self.score.weights {
            out.i32(*numerator);
            out.i32(*denominator);
        }
        for offset in &self.score.input_offsets {
            out.u16(*offset);
        }
    }
}


