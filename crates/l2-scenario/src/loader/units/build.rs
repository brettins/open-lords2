#![allow(unused_imports)]
use super::*;
use super::records::*;
use super::restore::*;
use super::blank::*;
use super::super::*;
use super::super::helpers::*;
use super::super::*;
use l2_formats::save::{Save, SaveError, COUNTY_BASE, COUNTY_STRIDE, REALM_BASE, REALM_STRIDE};
use l2_kingdom::county::{County, MAX_COUNTY_ID, MAX_FIELDS};
use l2_kingdom::explore::Explored;
use l2_kingdom::map::MAP_TILES;
use l2_kingdom::merchant::{MerchantRoutes, ROUTES, ROUTE_SLOTS};
use l2_kingdom::mercenary::{Band, MercenaryBands, MERCENARY_BANDS, ROSTER};
use l2_kingdom::realm::MAX_REALMS;
use l2_kingdom::tables::{health_band, Tables, Weather, JOB_COUNT};
use l2_kingdom::unit::{Mercenaries, TroopType, Unit, UnitKind, Units, MAX_UNITS, TROOP_TYPES};
use l2_kingdom::{field, land, CampaignMap, Kingdom, Options};

impl Scenario {
    pub fn county_ids(&self) -> core::ops::RangeInclusive<usize> {
        1..=self.county_count
    }

    pub fn kingdom(&self, seed: u64) -> Kingdom {
        self.kingdom_with_tables(seed, Tables::DEFAULT)
    }

    pub fn kingdom_with_tables(&self, seed: u64, tables: Tables) -> Kingdom {
        let mut k = self.skeleton(seed, tables);
        k.season = self.clock.season;
        k.season_next = self.clock.season_next;
        k.season_prev = if self.clock.season == 1 { 4 } else { self.clock.season - 1 };
        k.year = self.clock.year;
        k.year_next = self.clock.year + 1;
        k.turn_count = self.clock.turn_count;

        for id in self.county_ids() {
            let Some(s) = &self.counties[id] else { continue };
            let c = &mut k.counties[id];

            // **Destructured with no `..`, and that is the whole point of the
            // shape.** `County::farm_style` was read by the rules and written by
            // nothing on this path for months (`docs/decisions.md` C62): the
            // importer assigned field by field onto a default,
// remembered stayed at zero, and *no test anywhere could see
            // it* — a diff of two `CountyState`s cannot notice a step after
            // `CountyState`, and a round trip compares one fixture.
            let CountyState {
                population,
                population_last,
                happiness,
                happiness_last,
                shown_tax,
                shown_ration,
                shown_health,
                shown_events,
                d_hap_ration,
                d_hap_health,
                d_hap_tax_local,
                tax_shown,
                health_meter,
                health_band,
                unrest,
                births,
                deaths,
                emigrants,
                immigrants,
                pop_band,
                tax_collected,
                ration_achieved,
                grain_eaten,
                herd_eaten,
                grain,
                herd,
                owner: _,
                anchor: _,
                neighbours: _,
                tax_rate: _,
                ration_wanted: _,
                ration_split: _,
                castle_type: _,
                castle_building: _,
                castle_switch: _,
                industry,
                fertility: _,
                weather: _,
                dryness: _,
                labour: _,
                labour_wanted: _,
                labour_useful: _,
                labour_share: _,
                industry_share: _,
                field_tiles: _,
                farm_style: _,
                purse,
                merchant_count,
                merchant_unit,
                merchant_visits,
                // --- derived, not imported --------------------------------
                // `County_RecountFields` makes these from the twenty field
                // tiles, so the file's copies are a cache we recompute rather
                // than trust. `docs/decisions.md` C62 records the check that
                // the derived values match the stored bytes.
                fields_fallow: _,
                fields_cattle: _,
                fields_grain: _,
                // --- C161: carried here ---------------------
                herd_change_expected,
                herd_births_expected,
                herd_deaths_expected,
                grain_weather_change,
                grain_event_change,
                herd_weather_change,
                herd_event_change,
                grain_change_expected,
                grain_sown_expected,
                grain_grown_expected,
                reclaim_fields_finishing,
                reclaim_seasons_to_next,
                happiness_avg,
                happiness_sum,
                d_hap_tax,
                shown_army,
                tax_hap_other,
                shown_ale,
                ale_happiness_given,
                unrest_warned,
                pop_change_pct,
                army,
                largest_inflow,
                inflow_sources,
                emigrant_destination,
                largest_inflow_source,
                change_reason,
                event_fired,
                event_id,
                event_population_pct,
                event_population_swing,
                event_grain_pct,
                event_herd_pct,
                tax_suppressed,
                field_progress,
                friendly_troops,
                enemy_troops,
                levy_surcharge,
                castle_degraded,
                castle_ruined,
                castle_level_left,
                castle_percent,
                castle_work_left,
                castle_work_total,
                castle_stone_owed,
                castle_stone_total,
                castle_wood_owed,
                castle_wood_total,
                siege_scars,
                crop,
                pasture_cursor,
                blight_cursor,
                fields_grain_sown,
                fields_grain_standing,
                sow_shortfall,
                weapon_type,
                mercenary_offer,
            } = s;

            c.population = *population;
            c.pop_last = *population_last;
            c.happiness = *happiness;
            c.happiness_last = *happiness_last;
            c.happiness_sum = *happiness_sum;
            c.happiness_avg = *happiness_avg;
            // **The three farm rows' forecasts, and the rest of what the county
            // panels draw.** Each was `County::new()`'s zero on a loaded game,
            // which `Ui_DrawDelta` draws as nothing — C142's defect, a fourth
            // and fifth time, found this time by `docs/stored-fields.json`
// Not recomputed on load, for C142's reason:
            c.herd_change_expected = *herd_change_expected;
            c.herd_births_expected = *herd_births_expected;
            c.herd_deaths_expected = *herd_deaths_expected;
            c.grain_weather_change = *grain_weather_change;
            c.grain_event_change = *grain_event_change;
            c.herd_weather_change = *herd_weather_change;
            c.herd_event_change = *herd_event_change;
            c.grain_change_expected = *grain_change_expected;
            c.grain_sown_expected = *grain_sown_expected;
            c.grain_grown_expected = *grain_grown_expected;
            c.reclaim_fields_finishing = *reclaim_fields_finishing;
            c.reclaim_seasons_to_next = *reclaim_seasons_to_next;
            c.d_hap_tax = *d_hap_tax;
            c.shown_army = *shown_army;
            c.tax_hap_other = *tax_hap_other;
            c.shown_ale = *shown_ale;
            c.ale_happiness_given = *ale_happiness_given;
            c.unrest_warned = *unrest_warned;
            c.pop_change_pct = *pop_change_pct;
            c.army = *army;
            c.largest_inflow = *largest_inflow;
            c.inflow_sources = *inflow_sources;
            c.emigrant_destination = *emigrant_destination;
            c.largest_inflow_source = *largest_inflow_source;
            c.change_reason = match *change_reason {
                1 => l2_kingdom::county::ChangeReason::Births,
                2 => l2_kingdom::county::ChangeReason::Deaths,
                3 => l2_kingdom::county::ChangeReason::Emigration,
                4 => l2_kingdom::county::ChangeReason::Immigration,
                _ => l2_kingdom::county::ChangeReason::None,
            };
            c.event_fired = *event_fired;
            c.event_id = *event_id;
            c.event_population_pct = *event_population_pct;
            c.event_population_swing = *event_population_swing;
            c.event_grain_pct = *event_grain_pct;
            c.event_herd_pct = *event_herd_pct;
            c.tax_suppressed = *tax_suppressed;
            c.field_progress = *field_progress;
            c.friendly_troops = *friendly_troops;
            c.enemy_troops = *enemy_troops;
            c.levy_surcharge = *levy_surcharge;
            c.castle_degraded = *castle_degraded;
            c.castle_ruined = *castle_ruined;
            c.castle_level_left = *castle_level_left;
            c.castle_percent = *castle_percent;
            c.castle_work_left = *castle_work_left;
            c.castle_work_total = *castle_work_total;
            c.castle_stone_owed = *castle_stone_owed;
            c.castle_stone_total = *castle_stone_total;
            c.castle_wood_owed = *castle_wood_owed;
            c.castle_wood_total = *castle_wood_total;
            c.siege_scars = *siege_scars;
            c.crop = *crop;
            c.pasture_cursor = *pasture_cursor;
            c.blight_cursor = *blight_cursor;
            c.fields_grain_sown = *fields_grain_sown;
            c.fields_grain_standing = *fields_grain_standing;
            c.sow_shortfall = *sow_shortfall;
            c.weapon_type = *weapon_type;
            c.mercenary_offer = *mercenary_offer;
            for (slot, s) in industry.iter().enumerate() {
                c.industry[slot].efficiency = s.efficiency;
                // The ramp's input is record `+0x08`, not this byte
                // (`Industry_EfficiencyRamp` `0x0044F248`), and `+0x08` is not
                // in `docs/stored-fields.json`. `Industry_Produce`
                // (`0x0044EA92`) copies `+0x00` into it every season, so the
                // two part only between a refresh and the next season's pass
                // and only with *Advanced Farming* on — off, both are the flat
                // 80 every save on this machine stores. `[I]`.
                c.industry[slot].last_efficiency = s.efficiency;
                c.industry[slot].capacity = s.capacity;
                c.industry[slot].total = s.total;
                c.industry[slot].output = s.total - s.total_snapshot;
            }
            c.shown_tax = *shown_tax;
            c.shown_ration = *shown_ration;
            c.shown_health = *shown_health;
            c.shown_events = *shown_events;
            c.d_hap_ration = *d_hap_ration;
            // **The three the county panels draw and nothing carried.** Not
            // recomputed on load: the original restores a memory image, and
            // `incombat.sav` proves the difference — its stored `+0xC0` is the
            // answer for the population the county had before the battle ate
            // it, which no recompute could reproduce. See [`D_HAP_TAX_LOCAL`]
            // and `docs/decisions.md` C142.
            c.d_hap_health = *d_hap_health;
            c.d_hap_tax_local = *d_hap_tax_local;
            c.tax_shown = *tax_shown;
            c.health_meter = *health_meter;
            c.health_band = *health_band;
            c.unrest = *unrest;
            c.births = *births;
            c.deaths = *deaths;
            c.emigrants = *emigrants;
            c.immigrants = *immigrants;
            c.pop_band = *pop_band;
            c.tax_collected = *tax_collected;
            c.purse = *purse;
            c.merchant_count = *merchant_count;
            c.merchant_unit = *merchant_unit;
            c.merchant_visits = *merchant_visits;
            c.ration_achieved = *ration_achieved;
            c.grain_eaten = *grain_eaten;
            c.herd_eaten = *herd_eaten;
            c.grain_available = *grain;
            c.herd_available = *herd;
        }

        for (id, r) in self.realms.iter().enumerate().take(MAX_REALMS).skip(1) {
            let RealmState {
                ai_step,
                tax_hap_empire,
                population_total,
                population_mean,
                population_last,
                mean_happiness,
                mean_health,
                share_of_map_pct,
                army_count,
                total_men,
                castle_count,
                offer_pending,
                ally_candidate,
                ally,
                target_county,
                taunt_timer,
                taunt_stage,
                war_target,
                offer_timer,
                crowned_once,
                weapon_rota,
                voice_rotation,
                muster_county,
                raid_county,
                muster_timer,
                threat_realm,
                attack_county,
                raid_timer,
                want,
                bankrupt_stage,
                trade_spent_a,
                trade_spent_b,
                trade_received_a,
                trade_received_b,
                tax_ledger,
                ..
            } = r;
            let realm = &mut k.realms[id];
            realm.ai_step = *ai_step;
            realm.tax_hap_empire = *tax_hap_empire;
            realm.population_total = *population_total;
            realm.population_mean = *population_mean;
            realm.population_last = *population_last;
            realm.mean_happiness = *mean_happiness;
            realm.mean_health = *mean_health;
            realm.share_of_map_pct = *share_of_map_pct;
            realm.army_count = *army_count;
            realm.total_men = *total_men;
            // The five named score inputs are copies; `+0x4C` is the sixth and
            // the only one with no named field of its own.
            realm.sync_score_inputs();
            realm.score_inputs[l2_kingdom::tables::SCORE_INPUT_CASTLES] = *castle_count;
            realm.offer_pending = *offer_pending;
            realm.ally_candidate = *ally_candidate;
            realm.ally = *ally;
            realm.target_county = *target_county;
            realm.taunt_timer = *taunt_timer;
            realm.taunt_stage = *taunt_stage;
            realm.war_target = *war_target;
            realm.offer_timer = *offer_timer;
            realm.crowned_once = *crowned_once;
            realm.weapon_rota = *weapon_rota;
            realm.voice_rotation = *voice_rotation;
            realm.muster_county = *muster_county;
            realm.raid_county = *raid_county;
            realm.muster_timer = *muster_timer;
            realm.threat_realm = *threat_realm;
            realm.attack_county = *attack_county;
            realm.raid_timer = *raid_timer;
            realm.want = *want;
            realm.bankrupt_stage = *bankrupt_stage;
            realm.trade_spent_a = *trade_spent_a;
            realm.trade_spent_b = *trade_spent_b;
            realm.trade_received_a = *trade_received_a;
            realm.trade_received_b = *trade_received_b;
            realm.tax_ledger = *tax_ledger;
        }
        k
    }

    /// **The food stores are the ones the save holds** at `+0x224` / `+0x250`,
    /// the stores the season *ended* on.
    ///
    /// > This used to continue *"because the save holds no earlier ones"*, and
    /// > it does: county `+0x228` and `+0x254` are the grain and herd as
    /// > `Grain_SeasonTick` (`0x0044C8AE`) and `Herd_SeasonTick` (`0x0044D60D`)
    /// > found them, copied before the season's food is taken out, and
    /// > `Game_SetupRealmsAndCounties` (`0x0049BD99`) writes the new-game stores
    /// > into the same two fields. Starting from them reproduces the whole
    /// > England turn-one map — `crates/l2-kingdom/tests/reproduction/main.rs`,
    /// > `every_county_reproduces_from_the_stores_the_season_found`. This
    /// > function still does not read them; that is a change to what a rewound
    /// > position *is*, and it is left for whoever owns the importer.
    pub fn starting_kingdom(&self, seed: u64) -> Kingdom {
        self.starting_kingdom_with_tables(seed, Tables::DEFAULT)
    }

    pub fn starting_kingdom_with_tables(&self, seed: u64, tables: Tables) -> Kingdom {
        let mut k = self.skeleton(seed, tables);
        for id in self.county_ids() {
            let Some(s) = &self.counties[id] else { continue };
            let c = &mut k.counties[id];
            c.population = s.population_last;
            c.happiness = s.happiness_last;
            c.health_meter = STARTING_HEALTH_METER;
            c.health_band = health_band(STARTING_HEALTH_METER) as u8;
            c.ration_achieved = s.ration_wanted;
        }
        k
    }

}
