#![allow(unused_imports)]
use super::*;
use super::records::*;
use super::build::*;
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
    pub fn from_save(save: &Save) -> Result<Scenario, ImportError> {
        let g = save.globals()?;

        let county_count = g.county_count;
        if county_count < 1 || county_count > MAX_COUNTY_ID as i32 {
            return Err(ImportError::CountyCount(county_count));
        }
        let county_count = county_count as usize;

        if g.local_player < 1 || g.local_player >= MAX_REALMS as i32 {
            return Err(ImportError::LocalPlayer(g.local_player));
        }
        let local_player = g.local_player as u8;

        if !(1..=4).contains(&g.season) || !(1..=4).contains(&g.season_next) {
            return Err(ImportError::Clock { season: g.season, season_next: g.season_next });
        }

        let stored = save.counties()?;
        let mut counties: Vec<Option<CountyState>> = vec![None; MAX_COUNTY_ID as usize + 1];
        for c in stored.iter().take(county_count + 1).skip(1) {
            if c.owner as usize >= MAX_REALMS {
                return Err(ImportError::Owner { county: c.index, owner: c.owner });
            }
            let weather = Weather::from_index(c.weather)
                .ok_or(ImportError::Weather { county: c.index, byte: c.weather })?;
            let mut neighbours = Vec::with_capacity(c.neighbours().len());
            for &id in c.neighbours() {
                if id as usize == c.index || id < 1 || id as usize > county_count {
                    return Err(ImportError::Neighbour { county: c.index, id });
                }
                neighbours.push(id);
            }
            let s = &c.stored;
            counties[c.index] = Some(CountyState {
                owner: c.owner,
                population: c.population,
                population_last: c.pop_last,
                happiness: c.happiness as i32,
                happiness_last: c.happiness_last as i32,
                shown_tax: c.shown_tax as i32,
                shown_ration: c.shown_ration as i32,
                shown_health: c.shown_health as i32,
                shown_events: c.shown_events as i32,
                d_hap_ration: c.d_hap_ration as i32,
                d_hap_health: c.d_hap_health as i32,
                d_hap_tax_local: s.d_hap_tax_local as i32,
                tax_shown: s.tax_shown,
                health_meter: c.health_meter as i32,
                health_band: c.health_band.max(0) as u8,
                unrest: c.unrest,
                births: c.births,
                deaths: c.deaths,
                emigrants: c.emigrants,
                immigrants: c.immigrants,
                pop_band: c.pop_band as i32,
                anchor: (c.anchor_x, c.anchor_y),
                neighbours,
                tax_rate: c.tax_rate as i32,
                tax_collected: c.tax_collected,
                ration_wanted: c.ration_wanted as i32,
                ration_achieved: c.ration_achieved as i32,
                ration_split: c.ration_split as i32,
                grain_eaten: c.grain_eaten,
                herd_eaten: c.herd_eaten,
                castle_type: c.castle_type,
                castle_building: c.castle_building,
                castle_switch: s.castle_switch != 0,
                industry: read_industry(s),
                fields_fallow: c.fields_fallow as i32,
                fields_cattle: c.fields_cattle as i32,
                fields_grain: c.fields_grain as i32,
                fertility: c.fertility,
                weather,
                dryness: c.dryness as i32,
                grain: c.grain,
                herd: c.herd,
                labour: s.labour[0],
                labour_wanted: s.labour[1],
                labour_useful: s.labour[2],
                labour_share: s.labour_share,
                industry_share: s.industry_share as i32,
                field_tiles: read_field_tiles(s),
                farm_style: s.farm_style,
                purse: s.purse,
                merchant_count: s.merchant_count as i32,
                merchant_unit: s.merchant_unit,
                merchant_visits: s.merchant_visits,

                // C161. `docs/stored-fields.json` says what each is.
                herd_change_expected: s.herd_change_expected,
                herd_births_expected: s.herd_births_expected,
                herd_deaths_expected: s.herd_deaths_expected,
                grain_weather_change: s.grain_weather_change,
                grain_event_change: s.grain_event_change,
                herd_weather_change: s.herd_weather_change,
                herd_event_change: s.herd_event_change,
                grain_change_expected: s.grain_change_expected,
                grain_sown_expected: s.grain_sown_expected,
                grain_grown_expected: s.grain_grown_expected,
                reclaim_fields_finishing: s.reclaim_fields_finishing,
                reclaim_seasons_to_next: s.reclaim_seasons_to_next,
                happiness_avg: c.happiness_avg as i32,
                happiness_sum: c.happiness_sum,
                d_hap_tax: c.d_hap_tax as i32,
                shown_army: c.shown_army as i32,
                tax_hap_other: s.tax_hap_other as i32,
                shown_ale: s.shown_ale,
                ale_happiness_given: s.ale_happiness_given as i32,
                unrest_warned: s.unrest_warned != 0,
                pop_change_pct: s.pop_change_pct,
                army: s.army,
                largest_inflow: s.largest_inflow,
                inflow_sources: s.inflow_sources,
                emigrant_destination: s.emigrant_destination,
                largest_inflow_source: s.largest_inflow_source,
                change_reason: s.change_reason,
                event_fired: s.event_fired != 0,
                event_id: s.event_id,
                event_population_pct: s.event_population_pct as i32,
                event_population_swing: s.event_population_swing,
                event_grain_pct: s.event_grain_pct as i32,
                event_herd_pct: s.event_herd_pct as i32,
                tax_suppressed: s.tax_suppressed != 0,
                field_progress: s.field_progress,
                friendly_troops: s.friendly_troops,
                enemy_troops: s.enemy_troops,
                levy_surcharge: s.levy_surcharge,
                castle_degraded: s.castle_degraded,
                castle_ruined: s.castle_ruined != 0,
                castle_level_left: s.castle_level_left,
                castle_percent: s.castle_percent,
                castle_work_left: s.castle_work_left,
                castle_work_total: s.castle_work_total,
                castle_stone_owed: s.castle_stone_owed,
                castle_stone_total: s.castle_stone_total,
                castle_wood_owed: s.castle_wood_owed,
                castle_wood_total: s.castle_wood_total,
                siege_scars: l2_kingdom::siege::SiegeScars {
                    moat_filled: s.siege_moat_filled,
                    wall_damage: s.siege_wall_damage,
                    breach_score: s.siege_breach_score,
                    approach_score: s.siege_approach_score,
                    ramparts_breached: s.siege_ramparts_breached,
                    gate_open: s.siege_gate_open != 0,
                },
                crop: c.crop,
                pasture_cursor: s.pasture_cursor,
                blight_cursor: s.blight_cursor,
                fields_grain_sown: s.fields_grain_sown as i32,
                fields_grain_standing: s.fields_grain_standing as i32,
                sow_shortfall: s.sow_shortfall != 0,
                weapon_type: s.weapon_type as usize,
                mercenary_offer: s.mercenary_offer,
            });
        }

        let mut realms = Vec::with_capacity(MAX_REALMS);
        for r in save.realms()?.iter() {
            let s = &r.stored;
            realms.push(RealmState {
                ai_step: r.ai_step,
                tax_hap_empire: r.tax_hap_empire,
                peak_counties: s.peak_counties,
                population_total: s.population_total,
                population_mean: s.population_mean,
                population_last: s.population_last,
                mean_happiness: s.mean_happiness as i32,
                mean_health: s.mean_health as i32,
                share_of_map_pct: s.share_of_map_pct as i32,
                army_count: s.army_count,
                total_men: s.total_men,
                castle_count: s.castle_count as i32,
                offer_pending: s.offer_pending != 0,
                ally_candidate: s.ally_candidate,
                ally: s.ally,
                target_county: s.target_county,
                taunt_timer: s.taunt_timer,
                taunt_stage: s.taunt_stage,
                war_target: s.war_target,
                offer_timer: s.offer_timer,
                crowned_once: s.crowned_once != 0,
                weapon_rota: s.weapon_rota,
                voice_rotation: s.voice_rotation,
                muster_county: s.muster_county,
                raid_county: s.raid_county,
                muster_timer: s.muster_timer,
                threat_realm: s.threat_realm,
                attack_county: s.attack_county,
                raid_timer: s.raid_timer,
                want: s.want,
                bankrupt_stage: s.bankrupt_stage,
                trade_spent_a: s.trade_spent_a,
                trade_spent_b: s.trade_spent_b,
                trade_received_a: s.trade_received_a,
                trade_received_b: s.trade_received_b,
                army_names: s.army_names,
                tax_ledger: s.tax_ledger,
                // The 96 bytes at `+0x84` that nothing read until C83.
                pairs: {
                    let mut p = [l2_kingdom::realm::Pair::default(); l2_kingdom::realm::MAX_REALMS];
                    for (i, slot) in p.iter_mut().enumerate() {
                        let f = r.pairs[i];
                        *slot = l2_kingdom::realm::Pair {
                            standing: f.standing,
                            allied: f.allied,
                            grudge: f.grudge,
                            warnings_sent: f.warnings_sent,
                            at_war: f.at_war,
                            compliments_from: f.compliments_from,
                            best_gift: f.best_gift,
                            has_mail: f.has_mail,
                            help_price_multiple: f.help_price_multiple,
                        };
                    }
                    p
                },
                in_play: r.in_play(),
                strength: r.strength,
                is_human: r.is_human,
                lord: r.lord,
                shield_index: r.shield_index,
                county_count: r.county_count,
                rank: r.rank,
                score: r.score,
                gold: r.gold,
                wages: r.wages,
                iron: r.iron,
                stone: r.stone,
                wood: r.wood,
                weapons: {
                    let mut w = [0i32; l2_kingdom::tables::WEAPON_TYPE_COUNT];
                    let n = w.len().min(r.weapons.len());
                    w[..n].copy_from_slice(&r.weapons[..n]);
                    w
                },
            });
        }

        let mut units = Vec::new();
        for u in save.units()?.iter().filter(|u| u.is_live()) {
            units.push((u.index, read_unit(u)?));
        }
        let mercenaries = read_mercenaries(save, county_count, &units)?;
        for (id, c) in counties.iter().enumerate() {
            let Some(c) = c else { continue };
            if c.mercenary_offer != 0 && mercenaries.get(c.mercenary_offer).is_none() {
                return Err(ImportError::MercenaryOffer { county: id, band: c.mercenary_offer });
            }
        }

        Ok(Scenario {
            county_count,
            local_player,
            weather_county: g.weather_county.clamp(1, county_count as i32) as usize,
            options: Options {
                difficulty: g.opt_difficulty.clamp(0, 3) as u8,
                advanced_farming: g.opt_advanced_farming != 0,
                armies_eat: g.opt_armies_eat != 0,
                // **NOT read from the save, and now we know why.** This used to
// say `l2-formats` did not expose `g_optFightHumansOnly`
                // (0x0053F284). Adding it to that list turned the battle
                // fixtures red with `NotSaved`, and the block table says the
                // reason: **the original does not save this option.** Seven
                // four-byte entries cover 0x0053F23C, F258, F25C, F260, F264,
                // F268 and F26C, and 0x0053F284 is in none of them.
                fight_humans_only_byte: l2_kingdom::battle::FIGHT_HUMANS_ONLY_DEFAULT,
                exploration: g.opt_exploration != 0,
                time_limit: g.opt_time_limit,
                // `docs/decisions.md` C62.
                quirks: l2_kingdom::Quirks::FAITHFUL,
            },
            clock: Clock {
                season: g.season as u8,
                season_next: g.season_next as u8,
                year: g.year,
                turn_count: g.turn_count.max(0) as u32,
            },
            counties,
            realms,
            map: read_map(save)?,
            mercenaries,
            explored: read_explored(save, local_player)?,
            units,
            routes: {
                let rows = save.merchant_routes()?;
                let mut routes = MerchantRoutes::none();
                for (row, cells) in rows.iter().enumerate().take(ROUTES) {
                    let mut slots = [0u8; ROUTE_SLOTS];
                    slots.copy_from_slice(&cells[..ROUTE_SLOTS]);
                    routes.set_row(row, slots);
                }
                routes
            },
            merchant_start: save.merchant_start_counties()?,
        })
    }

}
