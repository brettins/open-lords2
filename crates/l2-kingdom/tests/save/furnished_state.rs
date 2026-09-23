#![allow(unused_imports)]
use super::*;
use super::campaign::*;
use super::round_trips::*;
use super::coverage::*;
use super::header::*;
use super::census_part::*;
use l2_kingdom::county::{ChangeReason, MAX_COUNTIES};
use l2_kingdom::realm::{Pair, MAX_REALMS};
use l2_kingdom::save::{
    checksum, decode, encode, ruleset_fingerprint, LoadError, HEADER_LEN, MAGIC, VERSION,
};
use l2_kingdom::tables::{Tables, Weather, JOB_COUNT};
use l2_kingdom::{Kingdom, Options};

pub(crate) fn furnished(seed: u64) -> Kingdom {
    let mut k = Kingdom::new(seed);

    k.season = 2;
    k.season_next = 3;
    k.season_prev = 1;
    k.year = 1300;
    k.year_next = 1301;
    k.turn_count = 42;
    k.weather_county = 9;
    k.turn.phase = l2_kingdom::Phase::Merchants;
    k.turn.step = 5;
    k.rng.next_u32();
    k.options = Options {
        difficulty: 2,
        advanced_farming: true,
        armies_eat: true,
        fight_humans_only_byte: 1,
        exploration: true,
        time_limit: 120,
        quirks: {
            let mut q = l2_kingdom::Quirks::FAITHFUL;
            q.set_reproduced(l2_kingdom::Quirk::AnyAleFillsATinyVillage, false);
            q.set_reproduced(l2_kingdom::Quirk::MercenaryBandOvershoots, false);
            q
        },
    };
    assert!(k.set_county_count(14));

    for id in 0..MAX_REALMS {
        let n = id as i32;
        let r = &mut k.realms[id];
        r.in_play = id != 4;
        r.strength = (3 * id + 1) as u8;
        r.is_human = id == 1;
        r.lord = (id + 1) as u8;
        r.shield_index = (id + 2) as u8;
        r.tax_hap_empire = -7 + n as i8;
        r.county_count = (id + 3) as u8;
        r.rank = (id % 5 + 1) as u8;
        r.score = 700 + n;
        r.wages = 80 + n;
        r.gold = 1000 + n * 37;
        r.iron = 40 + n;
        r.stone = 50 + n;
        r.wood = 60 + n;
        r.weapons = [1, 2, 3, 4, 5, 6].map(|w| w * (n + 1));
        r.bankrupt_stage = (id % 5 + 1) as u8;
        r.trade_spent_a = 1100 + n;
        r.trade_spent_b = 1200 + n;
        r.trade_received_a = 1300 + n;
        r.trade_received_b = 1400 + n;
        r.tax_ledger = [1500 + n, 1600 + n];
        r.population_total = 4000 + n;
        r.population_last = 3900 + n;
        r.population_mean = 300 + n;
        r.mean_happiness = 40 + n;
        r.mean_health = 55 + n;
        r.share_of_map_pct = 10 + n;
        r.army_count = (id + 1) as u8;
        r.total_men = 900 + n;
        r.score_inputs = [1, 2, 3, 4, 5, 6].map(|v| v * (n + 1));
        r.ai_step = n + 1;
        r.weapon_rota = n + 1;

        r.offer_pending = id % 2 == 0;
        r.ally_candidate = (id + 2) as u8;
        r.ally = (id + 3) as u8;
        r.target_county = (id + 4) as u8;
        r.taunt_timer = (id + 5) as u8;
        r.taunt_stage = (id % 2 + 1) as u8;
        r.war_target = (id + 6) as u8;
        r.offer_timer = -(n as i8) - 1;
        r.crowned_once = id % 3 == 0;
        r.voice_rotation = (id % 4 + 1) as u8;
        // Realm `+0x2A`, `VERSION` 24 — what `County_ChangeOwner`'s letter reads.
        r.peak_counties = (id + 12) as u8;

        r.muster_county = (id + 7) as u8;
        r.raid_county = (id + 8) as u8;
        r.muster_timer = (id + 9) as u8;
        r.threat_realm = (id % 5 + 1) as u8;
        r.attack_county = (id + 10) as u8;
        r.raid_timer = (id + 11) as u8;
        r.want = [100 + n, 200 + n, 0, 300 + n];
        for other in 0..MAX_REALMS {
            let m = other as i32;
            r.pairs[other] = Pair {
                standing: (n * 7 + m) as i8 - 30,
                allied: (id + other) % 2 == 0,
                grudge: (id * 3 + other + 1) as u8,
                warnings_sent: ((id + other) % 3 + 1) as u8,
                at_war: (id + other) % 3 == 0,
                compliments_from: (other + 1) as u8,
                best_gift: 50 + n * 10 + m,
                has_mail: other % 2 == 1,
                help_price_multiple: (other + 2) as u8,
            };
        }
    }

    for id in 0..MAX_COUNTIES {
        let n = id as i32;
        let c = &mut k.counties[id];
        c.owner = match id {
            1 | 4 | 8 => 1,
            11 | 13 => 2,
            _ => 0,
        };

        c.event_fired = id % 2 == 0;
        c.event_id = id as u16 * 3 + 1;
        c.health_meter = 60 + n;
        c.health_band = l2_kingdom::tables::health_band(c.health_meter) as u8;
        c.happiness = 50 + n;
        c.happiness_last = 48 + n;
        c.d_hap_tax = -3 - n;
        c.d_hap_tax_local = -4 - n;
        c.d_hap_health = 5 + n;
        c.d_hap_ration = 6 + n;
        c.shown_tax = 7 + n;
        c.shown_ration = 8 + n;
        c.shown_health = 9 + n;
        c.shown_army = 10 + n;
        c.tax_hap_other = 11 + n;
        c.shown_events = 12 + n;
        c.happiness_avg = 13 + n;
        c.happiness_sum = 14 + n;
        c.shown_ale = 15 + n;
        c.ale_happiness_given = 16 + n;
        c.unrest = (id % 4 + 1) as u8;
        c.unrest_warned = id % 2 == 0;

        c.population = 400 + n * 11;
        c.pop_last = 390 + n * 11;
        c.pop_change_pct = 17 + n;
        c.births = 18 + n;
        c.deaths = 19 + n;
        c.army = 20 + n;
        c.emigrants = 21 + n;
        c.immigrants = 22 + n;
        c.largest_inflow = 23 + n;
        c.emigrant_destination = (id % 13 + 1) as u8;
        c.largest_inflow_source = (id % 11 + 1) as u8;
        for slot in 0..c.inflow_sources.len() {
            c.inflow_sources[slot] = ((slot + id) % 15 + 1) as u8;
        }
        for neighbour in 1..=14u8 {
            if neighbour as usize != id {
                c.add_neighbour(neighbour);
            }
        }
        c.change_reason = match id % 5 {
            0 => ChangeReason::None,
            1 => ChangeReason::Births,
            2 => ChangeReason::Deaths,
            3 => ChangeReason::Emigration,
            _ => ChangeReason::Immigration,
        };
        c.pop_band = 24 + n;
        c.anchor_x = (id + 1) as u8;
        c.anchor_y = (100 - id) as u8;

        c.tax_rate = id as i32 % 13 + 1;
        c.tax_collected = 25 + n;
        c.tax_shown = 26 + n;
        c.purse = 27 + n;
        c.merchant_count = id as i32 % 3;
        c.merchant_unit = (id % 150 + 1) as u8;
        c.merchant_visits = 28 + n;
        for job in 0..JOB_COUNT {
            c.labour[job] = 500 + n + job as i32;
            c.labour_wanted[job] = 300 + n + job as i32;
            c.labour_useful[job] = 200 + n + job as i32;
        }
        for job in 0..JOB_COUNT - 1 {
            c.labour_share[job] = 7 + n + job as i32;
        }
        c.industry_share = 61 + n;
        for slot in 0..c.field_progress.len() {
            c.field_progress[slot] = ((slot as u16 * 37) % 801) + 1;
        }

        c.ration_achieved = id as i32 % 4 + 4;
        c.ration_wanted = id as i32 % 3 + 4;
        c.ration_split = id as i32 * 7 % 101;
        c.grain_eaten = 27 + n;
        c.herd_eaten = 28 + n;
        c.grain_available = 29 + n;
        c.herd_available = 30 + n;
        // `+0x18C`/`+0x190`, `VERSION` 30 — deliberately unequal to
        // `grain_eaten`/`herd_eaten` above: the shadow is what the season's
        // ration pass priced, those two are next season's forecast (C20).
        c.grain_eaten_shadow = 33 + n;
        c.herd_eaten_shadow = 34 + n;
        c.friendly_troops = 31 + n;
        c.enemy_troops = 32 + n;

        c.mercenary_offer = (id % 12 + 1) as u8;
        c.garrison_unit = id + 3;
        c.levy_surcharge = (id as i32 % 4) * 5 + 5;

        c.castle_type = (id % 6) as u8;
        c.castle_building = ((id + 1) % 6) as u8;
        c.castle_degraded = (id % 3) as u8;
        c.castle_ruined = id % 2 == 1;
        c.castle_level_left = (id % 5) as u8;
        c.siege_scars = l2_kingdom::siege::SiegeScars {
            moat_filled: (n % 97 + 1) as u16,
            wall_damage: (n % 53 + 2) as u16,
            breach_score: n * 3 + 11,
            approach_score: n * 5 + 13,
            ramparts_breached: (id % 7 + 1) as u8,
            gate_open: id % 2 == 0,
        };
        c.castle_switch = id % 3 == 1;
        c.castle_percent = (n % 101) as u8;
        c.castle_work_left = n * 13 + 1;
        c.castle_work_total = n * 17 + 2;
        c.castle_stone_owed = n * 3 + 4;
        c.castle_stone_total = n * 5 + 6;
        c.castle_wood_owed = n * 7 + 8;
        c.castle_wood_total = n * 11 + 9;

        c.event_population_pct = 33 + n;
        c.event_population_swing = 44 + n;
        c.event_grain_pct = 34 + n;
        c.event_herd_pct = 35 + n;

        for slot in 0..c.field_tiles.len() {
            c.field_tiles[slot] = ((slot as u16 * 11 + id as u16) % 4096) + 1;
        }
        c.tax_suppressed = id % 4 == 0;
        c.unrest_warned = id % 2 == 0;
        c.event_id = id as u16 * 3;
        c.weapon_type = id % 6;
        c.labour[0] = 500 + id as i32;
        c.labour[3] = 120;
        c.inflow_sources[0] = 9;
        c.inflow_sources[15] = 4;
        for slot in 0..c.field_progress.len() {
            c.field_progress[slot] = (slot as u16 * 37) % 801;
        }
        c.fields_fallow = 3;
        c.fields_cattle = 4;
        c.fields_grain = 6;
        c.fields_waste = 7;
        c.fields_reclaiming = 2;
        c.fertility = -5 - n;
        c.weather = Weather::ALL[id % 6];
        c.dryness = 30 + n;

        c.grain = 100 + n * 5;
        c.crop = [n + 1, n * 2 + 1, n * 3 + 1];
        c.fields_grain_sown = 5;
        // `+0x206`, VERSION 25: one below the sown count, as a trampled field
        // leaves it
        c.fields_grain_standing = 4;
        c.sow_shortfall = id % 2 == 1;
        c.pasture_cursor = (3 + n) as u8;
        c.blight_cursor = (11 + n) as u8;

        c.herd = 60 + n * 3;
        c.herd_crowding = 30 + n;
        c.herd_births_expected = 36 + n;
        c.herd_deaths_expected = 37 + n;
        c.herd_change_expected = -38 - n;
        c.grain_weather_change = -44 - n;
        c.grain_event_change = 45 + n;
        c.herd_weather_change = -46 - n;
        c.herd_event_change = 47 + n;
        c.grain_sown_expected = 39 + n;
        c.grain_grown_expected = 40 + n;
        c.grain_change_expected = -41 - n;
        c.reclaim_fields_finishing = 42 + n;
        c.reclaim_seasons_to_next = 43 + n;

        for slot in 0..c.industry.len() {
            let s = slot as i32;
            c.industry[slot].output = 10 * (s + 1) + n;
            c.industry[slot].efficiency = 5 * (s + 1) + n;
            c.industry[slot].last_efficiency = 4 * (s + 1) + n;
            c.industry[slot].capacity = 25 * (s + 1);
            c.industry[slot].has_resource = slot != 3;
            c.industry[slot].enabled = slot != 2;
            c.industry[slot].disabled_seasons = s % 3 + 1;
            c.industry[slot].total = 100 * (s + 1) + n;
            // The sidebar industry row's forecast, VERSION 17 — county
            // `+0x2A8 + c*0x18`, `Industry_LabourEstimate`'s tail.
            c.industry[slot].next_season = 7 * (s + 1) + n + 3;
        }
        c.weapon_type = id % 6;
        c.farm_style = (id % 5 + 1) as u8;
        c.tax_suppressed = id % 4 == 0;
    }

    furnish_campaign(&mut k);

    let mut snapshot = k.counties.clone();
    for season in 0..(l2_kingdom::tables::HISTORY_SEASONS + 25) {
        snapshot[1].population = 1_000 + season as i32;
        snapshot[2].happiness = (season % 100) as i32 + 1;
        k.history.record(&snapshot);
    }

    k
}

