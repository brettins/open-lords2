#![allow(unused_imports)]
use super::*;
use super::restore::*;
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
pub(super) fn read_mercenaries(
    save: &Save,
    county_count: usize,
    units: &[(usize, Unit)],
) -> Result<MercenaryBands, ImportError> {
    let in_play = save.mercenary_bands_in_play()?;
    if !(0..=MERCENARY_BANDS as i32).contains(&in_play) {
        return Err(ImportError::MercenaryBandCount(in_play));
    }
    let mut bands = MercenaryBands::none();
    bands.set_in_play(in_play as usize);
    for band in 1..=in_play as usize {
        let f = save.mercenary_band(band)?;
        let rules = &ROSTER[band];
        let constants = [
            ("start county", f.start_county as i32, rules.start_county as i32),
            ("troop type", f.troop as i32, rules.troop.index() as i32),
            ("reload", f.reload as i32, rules.period as i32),
            ("men", f.men, rules.men),
            ("price", f.price, rules.price),
            ("listed wage", f.listed_wage, rules.listed_wage),
        ];
        for (field, file, roster) in constants {
            if file != roster {
                return Err(ImportError::MercenaryRoster { band, field, file, roster });
            }
        }
        let hired_by = f.hired_by;
        let b = Band {
            hired_by: hired_by as u16,
            offered_in: f.offered_in,
            next_county: f.next_county,
            countdown: f.countdown,
            reload: f.reload,
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

pub(super) fn read_unit(u: &l2_formats::save::Unit) -> Result<Unit, ImportError> {
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
        ordered: u.move_state == 1,
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
        // `+0x182 + e*6` and `+0x19C`. Defaulted, `Siege_BuildTick`
        // (`0x004A8507`) skipped every `ordered == 0` record, so `remaining`
        // was 0 and the first tick reported ready: phase 2 launched the assault
        // three seasons early. `docs/decisions.md` C235.
        engines: core::array::from_fn(|e| l2_kingdom::siege::EngineBuild {
            ordered: u.engines[e].ordered,
            percent: u.engines[e].percent,
            work_done: u.engines[e].work_done,
        }),
        siege_seasons_left: u.siege_seasons_left,
        // `+0x1A` and `+0x19B` — the AI's mission byte and the county it is
        // about (`l2_kingdom::ai_army::Mission`), read by `Ai_RunMission`
        // (`0x004A57AC`) and `Ai_MissionAssistAlly` (`0x004A599D`). Defaulted
        // to 0, every imported AI army lost its first turn to `run_mission`'s
        // fallback arm and then attacked: a garrison carrying `+0x1A = 5`
        // (`battle-before.sav` slot 4) became `Mission::SEEK_ENEMY` and walked
        // out of its castle on turn two.
        mission: u.mission,
        mission_county: u.mission_county,
    })
}

