#![allow(unused_imports)]
use super::*;
use super::helpers::*;
use super::*;
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

/// * a type byte outside 1…4 — `g_unitTickTable`'s fifth slot is NULL and
///   nothing spawns a type-5 unit;
/// * an owner above 6 — 1…5 are realms and **6 is nobody**, which is what a
///   merchant and a county's own levied defence carry;
/// * a `+0x0C` that disagrees with `x`/`y`. That one is the **self-checking
///   invariant**: the field is `(y * 64 + x) * 8`, and nothing but the right
///   stride makes it agree on every occupied slot of every save.
///
/// `hiredBy` is an `i16` in the original and a slot in ours; a negative or
/// absent slot, or one that does not carry this band on its own `+0x197`, is
/// the same refusal as a unit standing on the wrong tile. The slots past the
/// in-play count are not read: `Mercenary_Init` never writes them, and every
/// save on this machine holds zero there.
fn read_mercenaries(
    save: &Save,
    county_count: usize,
    units: &[(usize, Unit)],
) -> Result<MercenaryBands, ImportError> {
    let in_play = save.i32_at(MERCENARY_BANDS_IN_PLAY)?;
    if !(0..=MERCENARY_BANDS as i32).contains(&in_play) {
        return Err(ImportError::MercenaryBandCount(in_play));
    }
    let mut bands = MercenaryBands::none();
    bands.set_in_play(in_play as usize);
    for band in 1..=in_play as usize {
        let at = MERCENARY_BANDS_VA + band as u32 * MERCENARY_BAND_STRIDE;
        let rules = &ROSTER[band];
        let constants = [
            ("start county", save.u8_at(at + 0x02)? as i32, rules.start_county as i32),
            ("troop type", save.u8_at(at + 0x05)? as i32, rules.troop.index() as i32),
            ("reload", save.i8_at(at + 0x07)? as i32, rules.period as i32),
            ("men", save.i32_at(at + 0x08)?, rules.men),
            ("price", save.i32_at(at + 0x0C)?, rules.price),
            ("listed wage", save.i32_at(at + 0x10)?, rules.listed_wage),
        ];
        for (field, file, roster) in constants {
            if file != roster {
                return Err(ImportError::MercenaryRoster { band, field, file, roster });
            }
        }
        let hired_by = save.i16_at(at)?;
        let b = Band {
            hired_by: hired_by as u16,
            offered_in: save.u8_at(at + 0x03)?,
            next_county: save.u8_at(at + 0x04)?,
            countdown: save.i8_at(at + 0x06)?,
            reload: save.i8_at(at + 0x07)?,
        };
        if b.offered_in as usize > county_count {
            return Err(ImportError::MercenaryState {
                band,
                field: "offered county",
                value: b.offered_in as i32,
            });
        }
        if hired_by != 0 {
            let carried = units
                .iter()
                .find(|(slot, _)| *slot as i32 == hired_by as i32)
                .filter(|(_, u)| u.kind == UnitKind::Army)
                .and_then(|(_, u)| u.mercenaries)
                .map(|m| m.band);
            if carried != Some(band as u8) {
                return Err(ImportError::MercenaryState {
                    band,
                    field: "hiring unit",
                    value: hired_by as i32,
                });
            }
        }
        bands.set_band_raw(band, b);
    }
    Ok(bands)
}

fn read_unit(u: &l2_formats::save::Unit) -> Result<Unit, ImportError> {
    let kind =
        UnitKind::from_byte(u.kind).ok_or(ImportError::UnitKind { unit: u.index, byte: u.kind })?;
    if u.owner as usize > MAX_REALMS {
        return Err(ImportError::UnitOwner { unit: u.index, owner: u.owner });
    }
    if !u.tile_offset_agrees() {
        return Err(ImportError::UnitTile { unit: u.index, x: u.x, y: u.y, offset: u.tile_offset });
    }
    let mut troops = [0i32; TROOP_TYPES];
    for (t, slot) in troops.iter_mut().enumerate() {
        *slot = u.troops[t] as i32;
    }
    Ok(Unit {
        owner: u.owner,
        owner_is_human: u.owner_is_human,
        shield: u.shield,
        player_driven: u.player_driven,
        kind,
        facing: u.facing,
        x: u.x,
        y: u.y,
        county: u.county,
        home_county: u.home_county,
        // The original has no "no destination" encoding for `+0x16`/`+0x17` —
        // the pair is always a tile, and `needs_destination` is the bit that
        // says whether it means anything. Carried as `Some` for that reason:
        dest: Some((u.dest_x, u.dest_y)),
        path: u.path(),
        moving: u.move_state != 0,
        on_road: u.on_road,
// **`+0x149 … +0x14B` are not read**, and this is a default
        // an import. `l2_formats::save::Unit` stops at `+0x14C`; adding the
        // three bytes is `l2-formats`' business and `docs/agents.md` reserves
        // that crate for the lead session.
//
        // counter is not being consulted. A unit whose last march ended
        // part-way through a tile carries that progress on disk and would get
        // it back; here it starts the next order from the near edge instead —
        // at worst fifteen-sixteenths of one tile's crossing, once, on the
        // first leg after a load. `docs/decisions.md` **C134**.
//
        // The latch defaults **set**, which is `Unit_Spawn`'s own value
        // (`0x0046E1B0`: `field_0x14b |= 1`) and the state the original leaves
        // a unit in when it stops for want of moves — `Unit_Step`'s budget
        // test is inside the latched arm,
        // on a tile edge by construction. Defaulting it clear would make every
        // imported unit stand still for its first crossing.
        sub_tile: 0,
        sub_frame: 0,
        at_tile_edge: true,
        name_index: u.name_index,
        needs_destination: u.needs_destination,
        dest_county: u.dest_county,
        moves_used: u.moves_used as i32,
        move_allowance: u.move_allowance as i32,
        starvation: u.starvation as i32,
        wages: u.wages,
        // `+0x164`. A merchant keeps its **route cursor** in the low byte of the
        // same field; `l2_kingdom::merchant::cursor_of` is the reading that
        // knows which is which.
        year_formed: u.year_formed as i32,
        morale: u.morale as i32,
        men: u.men,
        troops,
        // `+0x195…+0x197`. A band id of 0 is no band,
        // type mean nothing without it.
        mercenaries: (u.merc_band != 0)
            .then(|| {
                TroopType::from_index(u.merc_troop as usize)
                    .map(|troop| Mercenaries { band: u.merc_band, troop, men: u.merc_men })
            })
            .flatten(),
        garrison_county: u.garrison_county,
        besieging_county: u.besieging_county,
        besieged_by: u.besieged_by,
        // **`+0x167` is one byte and `Unit` models it as two fields**, because
        // the meanings: the county-defence
        // mark on an army or a mob, the cargo county on a transport, and — this
        // one is neither — the county a *merchant* was spawned in, which
        // `Merchant_SpawnAll` writes once and nothing ever updates.
        defence_mark: if kind.is_combatant() { u.role } else { 0 },
        cargo_county: if kind.is_combatant() { 0 } else { u.role },
        // The siege build records and the countdown are
        // yet - the unit block holds them and `l2-formats` does not surface them -
        // so an imported army starts with no engines ordered. A besieging army
        // imported mid-build therefore resumes at zero work, which is wrong and
// is recorded here: it needs the three `+0x16C` words
        // and the countdown adding to `l2_formats::save`.
        engines: Default::default(),
        siege_seasons_left: 0,
        // Unit `+0x1A` and `+0x19B` — the AI's mission byte and the county it
        // is about (`l2_kingdom::ai_army::Mission`). **Neither is surfaced by
        // `l2_formats::save`'s unit block yet**, so an imported army starts
        // with mission 0, which the AI's own dispatcher normalises to
        // `Mission::SEEK_ENEMY` on its first turn. That is one lost turn per
// imported army and it is stated: reading them is
        // two more bytes off the same record, in the same place the siege
        // records above are still missing from.
        //
        // It costs more than it looks on a save with a garrison in it. A
        // garrison carries `+0x1A = 5`, and imported as 0 it becomes an
        // attacker — so it walks out of its castle. `battle-before.sav` slot 4
        // is exactly that case.
        mission: 0,
        mission_county: 0,
    })
}

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
            let at = |offset: u32| COUNTY_BASE + (c.index * COUNTY_STRIDE) as u32 + offset;
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
                d_hap_health: county_i8(save, c.index, D_HAP_HEALTH)?,
                d_hap_tax_local: county_i8(save, c.index, D_HAP_TAX_LOCAL)?,
                tax_shown: county_i32(save, c.index, TAX_SHOWN)?,
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
                castle_switch: save
                    .u8_at(COUNTY_BASE + (c.index * COUNTY_STRIDE) as u32 + CASTLE_SWITCH)?
                    != 0,
                industry: read_industry(save, c.index)?,
                fields_fallow: c.fields_fallow as i32,
                fields_cattle: c.fields_cattle as i32,
                fields_grain: c.fields_grain as i32,
                fertility: c.fertility,
                weather,
                dryness: c.dryness as i32,
                grain: c.grain,
                herd: c.herd,
                labour: read_labour(save, c.index, 0)?,
                labour_wanted: read_labour(save, c.index, 4)?,
                labour_useful: read_labour(save, c.index, 8)?,
                labour_share: {
                    let base = COUNTY_BASE + (c.index * COUNTY_STRIDE) as u32 + LABOUR_SHARE_BASE;
                    let mut shares = [0i32; JOB_COUNT - 1];
                    for (job, share) in shares.iter_mut().enumerate() {
                        *share = save.i32_at(base + job as u32 * 4)?;
                    }
                    shares
                },
                industry_share: save
                    .i8_at(COUNTY_BASE + (c.index * COUNTY_STRIDE) as u32 + INDUSTRY_SHARE)?
                    as i32,
                field_tiles: read_field_tiles(save, c.index)?,
                farm_style: save
                    .u8_at(COUNTY_BASE + (c.index * COUNTY_STRIDE) as u32 + FARM_STYLE)?,
                purse: save.i32_at(COUNTY_BASE + (c.index * COUNTY_STRIDE) as u32 + PURSE)?,
                merchant_count: save
                    .u8_at(COUNTY_BASE + (c.index * COUNTY_STRIDE) as u32 + MERCHANT_COUNT)?
                    as i32,
                merchant_unit: save
                    .u8_at(COUNTY_BASE + (c.index * COUNTY_STRIDE) as u32 + MERCHANT_UNIT)?,
                merchant_visits: save
                    .i32_at(COUNTY_BASE + (c.index * COUNTY_STRIDE) as u32 + MERCHANT_VISITS)?,

                // C161. `docs/stored-fields.json` says what each is.
                herd_change_expected: save.i32_at(at(stored::HERD_CHANGE_EXPECTED))?,
                herd_births_expected: save.i32_at(at(stored::HERD_BIRTHS_EXPECTED))?,
                herd_deaths_expected: save.i32_at(at(stored::HERD_DEATHS_EXPECTED))?,
                grain_weather_change: save.i32_at(at(stored::GRAIN_WEATHER_CHANGE))?,
                grain_event_change: save.i32_at(at(stored::GRAIN_EVENT_CHANGE))?,
                herd_weather_change: save.i32_at(at(stored::HERD_WEATHER_CHANGE))?,
                herd_event_change: save.i32_at(at(stored::HERD_EVENT_CHANGE))?,
                grain_change_expected: save.i32_at(at(stored::GRAIN_CHANGE_EXPECTED))?,
                grain_sown_expected: save.i32_at(at(stored::GRAIN_SOWN_EXPECTED))?,
                grain_grown_expected: save.i32_at(at(stored::GRAIN_GROWN_EXPECTED))?,
                reclaim_fields_finishing: save.i32_at(at(stored::RECLAIM_FIELDS_FINISHING))?,
                reclaim_seasons_to_next: save.i32_at(at(stored::RECLAIM_SEASONS_TO_NEXT))?,
                happiness_avg: save.i8_at(at(stored::HAPPINESS_AVG))? as i32,
                happiness_sum: save.i32_at(at(stored::HAPPINESS_SUM))?,
                d_hap_tax: save.i8_at(at(stored::D_HAP_TAX))? as i32,
                shown_army: save.i8_at(at(stored::SHOWN_ARMY))? as i32,
                tax_hap_other: save.i8_at(at(stored::TAX_HAP_OTHER))? as i32,
                shown_ale: save.i32_at(at(stored::SHOWN_ALE))?,
                ale_happiness_given: save.u8_at(at(stored::ALE_HAPPINESS_GIVEN))? as i32,
                unrest_warned: save.u8_at(at(stored::UNREST_WARNED))? != 0,
                pop_change_pct: save.i32_at(at(stored::POP_CHANGE_PCT))?,
                army: save.i32_at(at(stored::POP_ARMY))?,
                largest_inflow: save.i32_at(at(stored::LARGEST_INFLOW))?,
                inflow_sources: {
                    let mut ids = [0u8; l2_kingdom::county::MAX_INFLOW_SOURCES];
                    for (slot, id) in ids.iter_mut().enumerate() {
                        *id = save.u8_at(at(stored::INFLOW_SOURCES + slot as u32))?;
                    }
                    ids
                },
                emigrant_destination: save.u8_at(at(stored::EMIGRANT_DESTINATION))?,
                largest_inflow_source: save.u8_at(at(stored::LARGEST_INFLOW_SOURCE))?,
                change_reason: save.u8_at(at(stored::CHANGE_REASON))?,
                event_fired: save.u8_at(at(stored::EVENT_FIRED))? != 0,
                event_id: save.u16_at(at(stored::EVENT_ID))?,
                event_population_pct: save.i8_at(at(stored::EVENT_POPULATION_PCT))? as i32,
                event_population_swing: save.i32_at(at(stored::EVENT_POPULATION_SWING))?,
                event_grain_pct: save.i8_at(at(stored::EVENT_GRAIN_PCT))? as i32,
                event_herd_pct: save.i8_at(at(stored::EVENT_HERD_PCT))? as i32,
                tax_suppressed: save.u8_at(at(stored::TAX_SUPPRESSED))? != 0,
                field_progress: {
                    let mut progress = [0u16; MAX_FIELDS];
                    for (slot, p) in progress.iter_mut().enumerate() {
                        *p = save.u16_at(at(stored::FIELD_PROGRESS + slot as u32 * 2))?;
                    }
                    progress
                },
                friendly_troops: save.i32_at(at(stored::FRIENDLY_TROOPS))?,
                enemy_troops: save.i32_at(at(stored::ENEMY_TROOPS))?,
                levy_surcharge: save.i32_at(at(stored::LEVY_SURCHARGE))?,
                castle_degraded: save.u8_at(at(stored::CASTLE_DEGRADED))?,
                castle_ruined: save.u8_at(at(stored::CASTLE_RUINED))? != 0,
                castle_level_left: save.u8_at(at(stored::CASTLE_LEVEL_LEFT))?,
                castle_percent: save.u8_at(at(stored::CASTLE_PERCENT))?,
                castle_work_left: save.i32_at(at(stored::CASTLE_WORK_LEFT))?,
                castle_work_total: save.i32_at(at(stored::CASTLE_WORK_TOTAL))?,
                castle_stone_owed: save.i32_at(at(stored::CASTLE_STONE_OWED))?,
                castle_stone_total: save.i32_at(at(stored::CASTLE_STONE_TOTAL))?,
                castle_wood_owed: save.i32_at(at(stored::CASTLE_WOOD_OWED))?,
                castle_wood_total: save.i32_at(at(stored::CASTLE_WOOD_TOTAL))?,
                siege_scars: l2_kingdom::siege::SiegeScars {
                    moat_filled: save.u16_at(at(stored::SIEGE_MOAT_FILLED))?,
                    wall_damage: save.u16_at(at(stored::SIEGE_WALL_DAMAGE))?,
                    breach_score: save.i32_at(at(stored::SIEGE_BREACH_SCORE))?,
                    approach_score: save.i32_at(at(stored::SIEGE_APPROACH_SCORE))?,
                    ramparts_breached: save.u8_at(at(stored::SIEGE_RAMPARTS_BREACHED))?,
                    gate_open: save.u8_at(at(stored::SIEGE_GATE_OPEN))? != 0,
                },
                crop: [
                    save.i32_at(at(stored::CROP))?,
                    save.i32_at(at(stored::CROP + 4))?,
                    save.i32_at(at(stored::CROP + 8))?,
                ],
                pasture_cursor: save.u8_at(at(stored::PASTURE_CURSOR))?,
                blight_cursor: save.u8_at(at(stored::BLIGHT_CURSOR))?,
                fields_grain_sown: save.u8_at(at(stored::FIELDS_GRAIN_SOWN))? as i32,
                fields_grain_standing: save.u8_at(at(stored::FIELDS_GRAIN_STANDING))? as i32,
                sow_shortfall: save.u8_at(at(stored::SOW_SHORTFALL))? != 0,
                weapon_type: save.u8_at(at(stored::WEAPON_TYPE))? as usize,
                mercenary_offer: save.u8_at(at(stored::MERCENARY_OFFER))?,
            });
        }

        let mut realms = Vec::with_capacity(MAX_REALMS);
        for r in save.realms()?.iter() {
            let at = |offset: u32| REALM_BASE + (r.index * REALM_STRIDE) as u32 + offset;
            realms.push(RealmState {
                ai_step: save.i32_at(at(0x000))?,
                tax_hap_empire: save.i8_at(at(0x028))?,
                peak_counties: save.u8_at(at(0x02A))?,
                population_total: save.i32_at(at(0x010))?,
                population_mean: save.i32_at(at(0x014))?,
                population_last: save.i32_at(at(0x018))?,
                mean_happiness: save.u8_at(at(0x00C))? as i32,
                mean_health: save.u8_at(at(0x058))? as i32,
                share_of_map_pct: save.u8_at(at(0x060))? as i32,
                army_count: save.u8_at(at(0x02C))?,
                total_men: save.i32_at(at(0x054))?,
                castle_count: save.u8_at(at(0x04C))? as i32,
                offer_pending: save.u8_at(at(0x01C))? != 0,
                ally_candidate: save.u8_at(at(0x080))?,
                ally: save.u8_at(at(0x081))?,
                target_county: save.u8_at(at(0x0E8))?,
                taunt_timer: save.u8_at(at(0x0E9))?,
                taunt_stage: save.u8_at(at(0x0EA))?,
                war_target: save.u8_at(at(0x0EB))?,
                offer_timer: save.i8_at(at(0x0EC))?,
                crowned_once: save.u8_at(at(0x0ED))? != 0,
                weapon_rota: save.i32_at(at(0x06C))?,
                voice_rotation: save.u8_at(at(0x159))?,
                muster_county: save.u8_at(at(0x0E5))?,
                raid_county: save.u8_at(at(0x0E6))?,
                muster_timer: save.u8_at(at(0x045))?,
                threat_realm: save.u8_at(at(0x048))?,
                attack_county: save.u8_at(at(0x04B))?,
                raid_timer: save.u8_at(at(0x15A))?,
                want: [
                    save.i32_at(at(0x070))?,
                    save.i32_at(at(0x074))?,
                    save.i32_at(at(0x078))?,
                    save.i32_at(at(0x07C))?,
                ],
                bankrupt_stage: save.u8_at(at(0x158))?,
                trade_spent_a: save.i32_at(at(0x104))?,
                trade_spent_b: save.i32_at(at(0x108))?,
                trade_received_a: save.i32_at(at(0x10C))?,
                trade_received_b: save.i32_at(at(0x110))?,
                army_names: {
                    let mut row = [0u8; l2_kingdom::unit::ARMY_NAME_SLOTS];
                    for (slot, n) in row.iter_mut().enumerate() {
                        *n = save.u8_at(at(stored::REALM_ARMY_NAMES + slot as u32))?;
                    }
                    row
                },
                tax_ledger: [save.i32_at(at(0x0F4))?, save.i32_at(at(0x0F8))?],
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

    fn skeleton(&self, seed: u64, tables: Tables) -> Kingdom {
        let mut k = Kingdom::with_tables(seed, tables);
        k.campaign.map = self.map.clone();
        k.campaign.explored = self.explored.clone();
        k.campaign.routes = self.routes.clone();
        // `Mercenary_Hire` and `Mercenary_AdvanceAll` both index this table, and
        // the county's `+0x1AD` names a slot in it: without it a loaded game has
        // no band in play
        k.campaign.mercenaries = self.mercenaries.clone();
        let mut units = Units::new();
        for (slot, unit) in &self.units {
            if *slot < MAX_UNITS {
                units.put(*slot, unit.clone());
            }
        }
        k.campaign.units = units;
        k.options = self.options;
        k.weather_county = self.weather_county;
        assert!(
            k.set_county_count(self.county_count),
            "from_save bounds the county count before it is stored"
        );

        for (id, r) in self.realms.iter().enumerate().take(MAX_REALMS).skip(1) {
            let RealmState {
                in_play,
                strength,
                is_human,
                lord,
                shield_index,
                county_count,
                peak_counties,
                rank,
                score,
                gold,
                wages,
                iron,
                stone,
                wood,
                weapons,
                pairs,
                army_names,
                ai_step: _,
                tax_hap_empire: _,
                population_total: _,
                population_mean: _,
                population_last: _,
                mean_happiness: _,
                mean_health: _,
                share_of_map_pct: _,
                army_count: _,
                total_men: _,
                castle_count: _,
                offer_pending: _,
                ally_candidate: _,
                ally: _,
                target_county: _,
                taunt_timer: _,
                taunt_stage: _,
                war_target: _,
                offer_timer: _,
                crowned_once: _,
                weapon_rota: _,
                voice_rotation: _,
                muster_county: _,
                raid_county: _,
                muster_timer: _,
                threat_realm: _,
                attack_county: _,
                raid_timer: _,
                want: _,
                bankrupt_stage: _,
                trade_spent_a: _,
                trade_spent_b: _,
                trade_received_a: _,
                trade_received_b: _,
                tax_ledger: _,
            } = r;
            let realm = &mut k.realms[id];
            realm.in_play = *in_play;
            realm.strength = *strength;
            realm.is_human = *is_human || id == self.local_player as usize;
            realm.lord = *lord;
            // `Game_SetupRealms` (`0x0049C5xx`) initialises every realm with
            // `g_realms[i].shieldIndex = i`, and only a custom game's colour
            // picker permutes it (`0x0049CE1F` walks a free-slot pool), which is
            // why a default game read either way comes out the same. Taken from
// the save's own byte at realm `+0x0A`,
            // custom game's flags are its own colours and not the realm order.
            realm.shield_index = *shield_index;
            realm.county_count = *county_count;
            realm.peak_counties = *peak_counties;
            realm.rank = *rank;
            realm.score = *score;
            realm.gold = *gold;
            realm.wages = *wages;
            realm.iron = *iron;
            realm.stone = *stone;
            realm.wood = *wood;
            realm.weapons = *weapons;
            // **The diplomatic matrix, **
            // `scenario::from_save` ran `Diplo_Init` because nothing read
            // these bytes; a mid-game save came back with every alliance and
            // every grudge gone. `docs/decisions.md` C83.
            realm.pairs = *pairs;
            k.campaign.names.set_counters(id as u8, *army_names);
        }

        for id in self.county_ids() {
            let Some(s) = &self.counties[id] else { continue };
            let c = &mut k.counties[id];
            *c = County::new();
            c.owner = s.owner;
            c.anchor_x = s.anchor.0;
            c.anchor_y = s.anchor.1;
            for &n in &s.neighbours {
                c.add_neighbour(n);
            }
            c.tax_rate = s.tax_rate;
            c.ration_wanted = s.ration_wanted;
            c.ration_split = s.ration_split;
            c.castle_type = s.castle_type;
            c.castle_building = s.castle_building;
            c.castle_switch = s.castle_switch;
            // **The three bytes that say whether an industry runs.** They were
            // not imported at all until C57, so every county arrived claiming
// all four resources and all four switches on — so no
            // county ever showed a mine in its village and why the map's
            // industry toggles all started in the wrong position. Everything
            // else in the record (`output`, `efficiency`, `capacity`,
            // `total`) is still `County::new()`'s.
            //
            // **And the forecast the row draws**,
            // held at zero — so every industry row was blank until the first
            // season ended. C153, and [`IndustryState::next_season`].
            for (slot, s) in s.industry.iter().enumerate() {
                c.industry[slot].has_resource = s.has_resource;
                c.industry[slot].disabled_seasons = s.disabled_seasons;
                c.industry[slot].enabled = s.enabled;
                c.industry[slot].next_season = s.next_season;
            }
            c.field_tiles = s.field_tiles;
            c.fertility = s.fertility;
            c.weather = s.weather;
            c.dryness = s.dryness;
            c.grain = s.grain;
            c.herd = s.herd;
            c.labour = s.labour;
            c.labour_wanted = s.labour_wanted;
            c.labour_useful = s.labour_useful;
            c.labour_share = s.labour_share;
            c.industry_share = s.industry_share;
            // `+0x1FE`. Not carried at all until now — see [`FARM_STYLE`].
            c.farm_style = s.farm_style;
            // `FUN_0044D913` is called from everywhere a county's herd or
            // pasture can change, county setup included,
            // arrives with its crowding already computed. Deriving it here
            field::recount(c, &self.map);
            c.herd_crowding = land::herd_crowding(&tables, c.herd, c.fields_cattle);
        }

        // The original keeps both halves — county `+0x1BC` names the unit and
        // unit `+0x198` names the county (`docs/armies.md` §2, both **[V]**) —
        // and `Castle_Garrison` writes them together. We import only the unit's
        // half, as [`Unit::garrison_county`], while **every** consumer reads the
        // county's: `conquest`'s ownership test, `divide`, `siege`'s
        // still-inside check, and `Map_DrawFrame`'s castle flag, which is gated
        // on `county.garrisonUnit != 0`. `County::new` seeds it to 0 and no
        // import path overwrote it,
        // garrisoned anywhere** — including the five siege fixtures that exist
        // for exactly that position,
        // symptom that exposed it was the castle flag never drawing; the
        // consequences in `conquest` and `siege` were the same bug and had not
        // been noticed. C59.
        for (slot, unit) in &self.units {
            let county = unit.garrison_county as usize;
            if county == 0 || *slot >= MAX_UNITS {
                continue;
            }
            if let Some(c) = k.counties.get_mut(county) {
                if c.garrison_unit == 0 {
                    c.garrison_unit = *slot;
                }
            }
        }
        k
    }
}


