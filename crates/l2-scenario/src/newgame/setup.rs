#![allow(unused_imports)]
use super::*;
use super::build_part::*;
use super::sites::*;
use super::fields::*;
use super::scenario::*;
use super::tests_part::*;
use l2_formats::maps::{MapSlot, Plane, PLANE_DIM};
use l2_kingdom::county::{MAX_COUNTIES, MAX_COUNTY_ID, MAX_FIELDS, MAX_NEIGHBOURS};
use l2_kingdom::map::{CampaignMap, MAP_TILES};
use l2_kingdom::merchant::{self, MerchantRoutes, ROUTES, ROUTE_SLOTS};
use l2_kingdom::mercenary::MercenaryBands;
use l2_kingdom::realm::MAX_REALMS;
use l2_kingdom::tables::{Weather, JOB_COUNT};
use l2_kingdom::unit::{Unit, UnitKind, Units};
use l2_kingdom::Options;
use crate::{Clock, CountyState, IndustryState, RealmState, Scenario};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Assignment {
    /// Realm `+0x0A`, `shieldIndex`, 1 … 5. **Zero means the walk gave this
    /// realm nothing** — it is above the lord count — and the caller falls back
/// to `FUN_0049C995`'s seed.
    pub(crate) shield: [u8; MAX_REALMS],
    /// Realm `+0x28`, the lord id. Zero for a human and for a realm out of
    /// play, which is what the original writes at the top of every iteration.
    pub(crate) lord: [u8; MAX_REALMS],
}

/// `Realms_AssignLords` (`0x0049CAAA`) — **the shield first, by position, and
/// then the lord from the shield.**
///
/// 1. Mark every **human's** chosen shield taken. The original reads
///    `g_playerSlots + realm * 0x2C + 0x25` — the six-slot record
///    `g_playerNames` is the `+0x04` of, so four bytes lower than the name —
///    guarded by `+0x26 == 0` (a person), and
///    page 4's `FUN_00432FAB` is what wrote it.
///
/// **This line used to read `let shield = realm`, under a doc comment that
/// stated that as the *mechanism*** — *"the colour slot is the realm id,
/// because `Game_SetupRealms` seeds `shieldIndex = i` and only a custom game's
/// colour picker permutes it."* The seed is real (`FUN_0049C995`) and the
/// conclusion drawn from it was not: the seed is what an *untouched* page 4
/// leaves, and `Realms_AssignLords` overwrites it for every AI on every run.
///
/// `docs/decisions.md` C130.
///
/// **`[D]` on the group.** The original picks the deterministic group 0 when
/// `DAT_0055302C == 1` and `(g_scenarioIndex & 3)` otherwise, and what
/// `DAT_0055302C` is has not been read. The scenario-varying path is taken here
/// because it is the one a single-player custom game reaches unless that flag
/// is set, and because it is the only reading under which the four groups exist
/// for a reason.
pub(crate) fn assign_lords(setup: &NewGame, lords: usize) -> Assignment {
    let group = setup.slot & 3;
    let human = setup.local_player as usize;
    let ai_lords = lords.saturating_sub(1);

    let mut a = Assignment { shield: [0; MAX_REALMS], lord: [0; MAX_REALMS] };
    let mut shield_taken = [false; 6];
    let mut lord_taken = [false; 8];

    if human >= 1 && human < MAX_REALMS {
        a.shield[human] = setup.shield;
        shield_taken[setup.shield as usize] = true;
    }

    let mut given = 0usize;
    for realm in 1..MAX_REALMS {
        if realm == human || given >= ai_lords {
            continue;
        }
        given += 1;
        let Some(shield) = (1..=5u8).find(|s| !shield_taken[*s as usize]) else { continue };
        shield_taken[shield as usize] = true;
        a.shield[realm] = shield;
        for n in 0..4usize {
            let at = group * 0x14 + shield as usize * 4 + n;
            let candidate = LORD_CHOICE.get(at).copied().unwrap_or(0);
            if candidate == 0 {
                continue;
            }
            if !lord_taken[candidate as usize & 7] {
                a.lord[realm] = candidate;
                lord_taken[candidate as usize & 7] = true;
                break;
            }
        }
    }
    a
}


/// `County_Reset` (`0x00451150`) — the opening economy of **every** county,
/// owned or not.
///
/// It runs after `Map_InitScenario` and before the seating, and the option rows
/// then overwrite five of its numbers on every county
/// (`Game_SetupRealmsAndCounties`' first loop). What survives is the ration, the
/// split, the weather, the dryness, the labour shares and the industry share —
/// and `tax_rate`, which **nothing sets at all**: the county record was zeroed
/// wholesale by `FUN_0046EA28` in `Game_NewGame`'s preamble and no new-game
/// path writes a tax rate,
/// `[D]`, and it is the answer to a question `docs/kingdom.md` does not ask.
pub(crate) fn county_reset(id: usize) -> CountyState {
    CountyState {
        owner: 0,
        population: reset::POPULATION,
        population_last: reset::POPULATION,
        happiness: reset::HAPPINESS,
        happiness_last: reset::HAPPINESS,
        shown_tax: 0,
        shown_ration: 0,
        shown_health: 0,
        shown_events: 0,
        d_hap_ration: 0,
        // **Zero here for the same reason `tax_rate` is**, and stated rather
        // than assumed: `FUN_0046EA28` zeroes the whole county record in
        // `Game_NewGame`'s preamble, and no new-game path read here writes any
        // of the three. The season the new game immediately runs is what fills
        // them — `Panels_RefreshAll`'s `Tax_RecomputePreview` turns `+0x0F`
        // into 5 at rate 0, which is what every shipped save holds. `[D]`.
        d_hap_health: 0,
        d_hap_tax_local: 0,
        tax_shown: 0,
        health_meter: reset::HEALTH_METER,
        health_band: reset::HEALTH_BAND,
        unrest: 0,
        births: 0,
        deaths: 0,
        emigrants: 0,
        immigrants: 0,
        pop_band: (reset::POPULATION - 1) / 25 + 1,
        anchor: (0, 0),
        neighbours: Vec::new(),
        tax_rate: 0,
        tax_collected: 0,
        ration_wanted: reset::RATION_WANTED,
        ration_achieved: 0,
        ration_split: reset::RATION_SPLIT,
        grain_eaten: 0,
        herd_eaten: 0,
        castle_type: 0,
        castle_building: 0,
        castle_switch: false,
        industry: industry_reset(),
        fields_fallow: 0,
        fields_cattle: 0,
        fields_grain: 0,
        fertility: 0,
        weather: Weather::from_index(reset::WEATHER).expect("3 is Cloudy"),
        dryness: reset::DRYNESS,
        grain: reset::GRAIN,
        herd: reset::HERD,
        labour: [0; JOB_COUNT],
        labour_wanted: [0; JOB_COUNT],
        labour_useful: [0; JOB_COUNT],
        labour_share: {
            let mut s = [0i32; JOB_COUNT - 1];
            let n = s.len().min(reset::LABOUR_SHARE.len());
            s[..n].copy_from_slice(&reset::LABOUR_SHARE[..n]);
            s
        },
        industry_share: reset::INDUSTRY_SHARE,
        field_tiles: [0; MAX_FIELDS],
        farm_style: (id & 1) as u8,
        purse: 0,
        merchant_count: 0,
        merchant_unit: 0,
        merchant_visits: 0,
        // **C161.** Every one of these is `County::new()`'s value,
        // which is what a new game got before the save path learned to carry
        // them — so this constructor's world is unchanged. Zero is also what
        // `County_Reset`'s zeroing of the record gives, except the industry
        // ramp, which `industry` above carries at `Industry::new`'s base.
        herd_change_expected: 0,
        herd_births_expected: 0,
        herd_deaths_expected: 0,
        grain_weather_change: 0,
        grain_event_change: 0,
        herd_weather_change: 0,
        herd_event_change: 0,
        grain_change_expected: 0,
        grain_sown_expected: 0,
        grain_grown_expected: 0,
        reclaim_fields_finishing: 0,
        reclaim_seasons_to_next: 0,
        happiness_avg: 0,
        happiness_sum: 0,
        d_hap_tax: 0,
        shown_army: 0,
        tax_hap_other: 0,
        shown_ale: 0,
        ale_happiness_given: 0,
        unrest_warned: false,
        pop_change_pct: 0,
        army: 0,
        largest_inflow: 0,
        inflow_sources: [0; l2_kingdom::county::MAX_INFLOW_SOURCES],
        emigrant_destination: 0,
        largest_inflow_source: 0,
        change_reason: 0,
        event_fired: false,
        event_id: 0,
        event_population_pct: 0,
        event_population_swing: 0,
        event_grain_pct: 0,
        event_herd_pct: 0,
        tax_suppressed: false,
        field_progress: [0; MAX_FIELDS],
        friendly_troops: 0,
        enemy_troops: 0,
        levy_surcharge: 0,
        castle_degraded: 0,
        castle_ruined: false,
        castle_level_left: 0,
        castle_percent: 0,
        castle_work_left: 0,
        castle_work_total: 0,
        castle_stone_owed: 0,
        castle_stone_total: 0,
        castle_wood_owed: 0,
        castle_wood_total: 0,
        siege_scars: l2_kingdom::siege::SiegeScars::default(),
        crop: [0; 3],
        pasture_cursor: 0,
        blight_cursor: 0,
        fields_grain_sown: 0,
        fields_grain_standing: 0,
        sow_shortfall: false,
        weapon_type: 0,
        mercenary_offer: 0,
    }
}

fn industry_reset() -> [IndustryState; 4] {
    let mut out = [IndustryState::default(); 4];
    for (slot, commodity) in [
        l2_kingdom::tables::Commodity::Wood,
        l2_kingdom::tables::Commodity::Iron,
        l2_kingdom::tables::Commodity::Weapons,
        l2_kingdom::tables::Commodity::Stone,
    ]
    .into_iter()
    .enumerate()
    {
        out[slot].efficiency = l2_kingdom::county::Industry::new(commodity).efficiency;
    }
    out
}


/// `Merchant_SpawnAll` (`0x00427ED0`) — one type-3 merchant, owner 6, on a free
/// road tile near each start county, **stopping at the first zero**.
pub(super) fn spawn_merchants(w: &MapWorld, map: &CampaignMap) -> Vec<(usize, Unit)> {
    let mut units = Units::new();
    let mut out = Vec::new();
    for (row, &county) in w.merchant_start.iter().enumerate() {
        if county == 0 {
            break;
        }
        let anchor = w.anchor.get(county as usize).copied().unwrap_or((0, 0));
        let Some((x, y)) = merchant::find_free_road_tile(map, &units, anchor)
            .or_else(|| merchant::find_free_open_tile(map, &units, anchor))
        else {
            continue;
        };
        let mut u = Unit::new(UnitKind::Merchant, 6, x, y);
        u.needs_destination = true;
        u.county = county;
        u.morale = 100;
        u.cargo_county = county;
        u.year_formed = 1;
        u.name_index = row as u8;
        if let Some(slot) = units.spawn(u.clone()) {
            out.push((slot, u));
        }
    }
    out
}


impl Default for RealmState {
    fn default() -> RealmState {
        RealmState {
            pairs: Default::default(),
            in_play: false,
            strength: 0,
            is_human: false,
            lord: 0,
            shield_index: 0,
            county_count: 0,
            peak_counties: 0,
            rank: 0,
            score: 0,
            gold: 0,
            wages: 0,
            iron: 0,
            stone: 0,
            wood: 0,
            weapons: [0; l2_kingdom::tables::WEAPON_TYPE_COUNT],
            army_names: [0; l2_kingdom::unit::ARMY_NAME_SLOTS],
            // C161: `Realm::new()`'s values, all zero, which is
            // what a new game had before the save path carried them.
            ai_step: 0,
            tax_hap_empire: 0,
            population_total: 0,
            population_mean: 0,
            population_last: 0,
            mean_happiness: 0,
            mean_health: 0,
            share_of_map_pct: 0,
            army_count: 0,
            total_men: 0,
            castle_count: 0,
            offer_pending: false,
            ally_candidate: 0,
            ally: 0,
            target_county: 0,
            taunt_timer: 0,
            taunt_stage: 0,
            war_target: 0,
            offer_timer: 0,
            crowned_once: false,
            weapon_rota: 0,
            voice_rotation: 0,
            muster_county: 0,
            raid_county: 0,
            muster_timer: 0,
            threat_realm: 0,
            attack_county: 0,
            raid_timer: 0,
            want: [0; 4],
            bankrupt_stage: 0,
            trade_spent_a: 0,
            trade_spent_b: 0,
            trade_received_a: 0,
            trade_received_b: 0,
            // `Game_SetupRealmsAndCounties` zeroes `+0xF4`/`+0xF8` beside the
            // trade pair (`0x0049C364`, `0x0049C37C`).
            tax_ledger: [0; 2],
        }
    }
}

