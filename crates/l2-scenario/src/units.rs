#![allow(unused_imports)]
use super::*;
use super::map::*;
use super::save_helpers::*;
use super::scenario_impl::*;
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

/// One `g_units` record, checked and converted.
///
/// Three refusals, and each of them means "the array is being read wrong"
///
///
/// * a type byte outside 1…4 — `g_unitTickTable`'s fifth slot is NULL and
///   nothing spawns a type-5 unit;
/// * an owner above 6 — 1…5 are realms and **6 is nobody**, which is what a
///   merchant and a county's own levied defence carry;
/// * a `+0x0C` that disagrees with `x`/`y`. That one is the **self-checking
///   invariant**: the field is `(y * 64 + x) * 8`, and nothing but the right
///   stride makes it agree on every occupied slot of every save.
/// `g_mercBands` and `g_mercBandsInPlay`, checked and converted. See
/// [`MERCENARY_BANDS_VA`] for the layout and the evidence.
///
/// Only the four walk fields and the hirer are carried per band, because that
/// is all [`MercenaryBands`] holds: the other six are `Mercenary_Init`'s copy of
/// the static roster, and the kingdom reads them from
/// [`l2_kingdom::mercenary::ROSTER`]. So the six are **compared**
/// imported, and a disagreement is a refusal — carrying the file's bands with
/// the roster's prices would be a different game that looked like this one.
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
        // dropping it while the unit is idle would lose the tile the info panel
        // still draws.
        dest: Some((u.dest_x, u.dest_y)),
        path: u.path(),
        moving: u.move_state != 0,
        on_road: u.on_road,
// **`+0x149 … +0x14B` are not read**, and this is a default
        // an import. `l2_formats::save::Unit` stops at `+0x14C`; adding the
        // three bytes is `l2-formats`' business and `docs/agents.md` reserves
        // that crate for the lead session.
        //
// What it costs is bounded and is stated: the
        // original writes its save from phase 7, **after** `Units_ResetMoves`,
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
        // **Taken from the file, not from the type.** Each type's tick handler
        // rewrites this unconditionally, so the six shipped merchants carry 0
        // and a defence raised mid-turn carries 0; an importer that wrote 15 or
        // 10 here would be inventing a number the game had not got round to.
        // `docs/armies.md` §8b.4.
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
        //
        // So the byte goes to the field its type gives it, and the merchant's
        // birthplace lands in `cargo_county` beside the transport's destination
        // because that is the non-army half of the alias. Nothing reads it back
        // for a merchant; `Scenario::merchant_start` is what checks it.
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

