#![allow(unused_imports)]
use super::*;
use super::pathfinding::*;
use super::tests_part::*;
use crate::county::{County, MAX_COUNTIES};
use crate::map::{coords, index, terrain, CampaignMap, CostMap, MAP_DIM, MAP_TILES};
use crate::realm::{Realm, MAX_REALMS};
use crate::unit::{UnitKind, Units, MAX_PATH};

/// `Unit_OrderMove` (`0x004A98FE`) — rebuild the cost map, fill, extract, and
/// hand the path to the unit.
///
/// > **Every write is inside the `if`.** `docs/armies.md` §2.3 renders the
/// > destination and `moveState = 2` as unconditional statements after it; they
/// > are not. A failed extraction leaves the unit
/// > destination, not moving, its previous path intact. Corrected in the
/// > document. `[D]`
/// >
/// > This is *not* the same as "an unreachable destination does
/// > nothing": `Move_ExtractPath` returns success with a zero-length path when
/// > the destination
/// > army stands still with `moveState = 2`. Only a dead end in the descent —
/// > which a fully relaxed field should not produce — fails.
pub fn order_move(map: &CampaignMap, units: &mut Units, id: usize, dest: (u8, u8), routing: Routing) -> Option<usize> {
    let cost = map.cost_map();
    let unit = units.get(id)?;
    let field = flood_fill(&cost, unit.tile(), routing);
    let path = extract_path(&cost, &field, dest)?;
    let steps = path.len();
    let dest_county = map.county_at(dest.0, dest.1);
    let unit = units.get_mut(id).expect("checked just above");
    unit.path = path;
    unit.dest = Some(dest);
    unit.dest_county = dest_county;
    unit.moving = true;
    unit.needs_destination = false;
    Some(steps)
}

/// Three callers run it statement for statement — the transport re-target
/// (`FUN_00429418`, turn phase 3), `Merchant_AdvanceAll` (phase 6)
/// AI's army walk — against [`order_move`] with [`Routing::Direct`], which is
/// what a *human* order does. So [`Routing::PreferRoads`]'s doc comment is
pub fn order_move_by_road(map: &CampaignMap, units: &mut Units, id: usize, dest: (u8, u8)) -> Option<usize> {
    let by_road = order_move(map, units, id, dest, Routing::PreferRoads);
    match by_road {
        Some(steps) if steps < crate::unit::MAX_PATH => Some(steps),
        _ => order_move(map, units, id, dest, Routing::Direct).or(by_road),
    }
}

/// `Unit_TryEnterTile` (`0x00466C3C`) — classify the tile in front.
pub fn try_enter(map: &CampaignMap, units: &Units, x: u8, y: u8) -> Entry {
    use crate::map::flags;
    if let Some(other) = units.at(x, y) {
        return Entry::Occupied(other);
    }
    let mut f = map.flags_at(x, y);
    if map.terrain_at(x, y) == terrain::TOWN {
        f &= !flags::SETTLEMENT;
    }
    if f & flags::ROAD != 0 {
        Entry::Road
    } else if f & flags::CASTLE != 0 {
        Entry::Castle
    } else if f & flags::SETTLEMENT != 0 {
        Entry::Settlement
    } else if f & flags::PLOT != 0 {
        Entry::Plot
    } else if f & flags::FARMLAND != 0 {
        Entry::Field
    } else {
        Entry::Open
    }
}

/// The three cases in which an occupied tile is **not** an obstacle —
/// `Unit_EnterOccupiedTile` (`0x004658C1`), read to the end.
///
/// > **Corrected, and this is what the correction was worth.** [`step`] treated
/// > every [`Entry::Occupied`] as the end of the move. Six merchants imported
/// > onto the England map jam within two seasons: three of them meet on the road
/// > junction between counties 11 and 12, each stops in front of the next, and
/// > none of them ever reaches a county again. In the original they walk through
/// > each other. `docs/armies.md` §2.7 described the ladder below the guards and
/// > did not mention them; correction **C40** in `docs/decisions.md`. `[V]` —
/// > four `if`s at the top of one function.
fn pass_through(
    map: &CampaignMap,
    units: &Units,
    kind: UnitKind,
    entry: Entry,
    x: u8,
    y: u8,
) -> Entry {
    let Entry::Occupied(other) = entry else { return entry };
    let mover_ignores = matches!(kind, UnitKind::Merchant | UnitKind::Transport);
    let occupant_is_merchant = units.get(other).map(|u| u.kind) == Some(UnitKind::Merchant);
    if !mover_ignores && !occupant_is_merchant {
        return entry;
    }
    if map.flags_at(x, y) & crate::map::flags::ROAD != 0 {
        Entry::Road
    } else {
        Entry::Open
    }
}

/// `Unit_StepOnce` (`0x0046634D`) — take exactly one step along the unit's
/// path.
pub fn step(
    map: &mut CampaignMap,
    counties: &mut [County; MAX_COUNTIES],
    realms: &[Realm; MAX_REALMS],
    units: &mut Units,
    id: usize,
) -> Option<Step> {
    let (next, kind, owner, owner_is_human) = {
        let u = units.get(id)?;
        if u.path.is_empty() || u.moves_used >= u.move_allowance {
            return None;
        }
        (u.path[0], u.kind, u.owner, u.owner_is_human)
    };
    if let Some(u) = units.get_mut(id) {
        u.sub_tile = 0;
        u.sub_frame = 0;
    }
    let (nx, ny) = next;
    let entry = pass_through(map, units, kind, try_enter(map, units, nx, ny), nx, ny);
    let tile_county = map.county_at(nx, ny);
    let mut out = Step::nothing(entry);

    match entry {
        Entry::Castle => {
            out.reached_castle = Some(tile_county);
            units.get_mut(id)?.moving = false;
            return Some(out);
        }
        Entry::Occupied(_) => {
            units.get_mut(id)?.moving = false;
            return Some(out);
        }
        Entry::Settlement => {
            // `Unit_ReachCastleBuilding` (`0x004686A0`) is the whole route into
            // both garrisoning and besieging, and without it a county's castle
            // could never be manned and never be besieged from the map. `[V]`
            let t = map.terrain_at(nx, ny);
            if t < terrain::DWELLING {
                let r = trample(map, counties, realms, units, id, nx, ny);
                out.charged = r.0;
                out.site_ruined = r.1;
            }
            if (terrain::CASTLE_FROM..=terrain::CASTLE_TO).contains(&t) {
                out.reached_castle_building = Some(tile_county);
            }
            units.get_mut(id)?.moving = false;
            return Some(out);
        }
        Entry::Plot => {
            // `Unit_BurnDwelling` (`0x00468AE2`) charges +7, exactly like
            // trampling — the row `docs/armies.md` §2.2 leaves as "—". It fires
            // only on an occupied plot in a county the unit does not own.
            if kind != UnitKind::Merchant
                && kind != UnitKind::Transport
                && map.terrain_at(nx, ny) == terrain::DWELLING
                && counties.get(tile_county as usize).map(|c| c.owner) != Some(owner)
            {
                let u = units.get_mut(id)?;
                u.moves_used += crate::tables::STEP_COST_TRAMPLE;
                out.charged = crate::tables::STEP_COST_TRAMPLE;
                // **The other half, and it is the whole of the rest of the
                // function.** `Unit_BurnDwelling` (`0x00468AE2`), `[V]` from
                // the decompilation, in its own order:
                map.set_terrain(nx, ny, terrain::DWELLING_BURNT);
                out.dwelling_burnt = Some(tile_county);
                if let Some(c) = counties.get_mut(tile_county as usize) {
                    c.population -= c.population / 4;
                }
                out.offence = Some(Offence {
                    against: counties.get(tile_county as usize).map_or(0, |c| c.owner),
                    by: owner,
                    amount: DWELLING_BURN_OFFENCE,
                });
            }
            units.get_mut(id)?.moving = false;
            return Some(out);
        }
        Entry::Field => {
            let (charged, destroyed, offence) =
                cross_field(map, counties, units, id, nx, ny, owner, owner_is_human);
            out.charged += charged;
            out.field_destroyed = destroyed;
            out.offence = offence;
        }
        Entry::Open | Entry::Road => {}
    }

    let u = units.get_mut(id)?;
    u.at_tile_edge = false;
    u.sub_tile = 1;
    u.on_road = entry == Entry::Road;
    let base = if u.on_road {
        crate::tables::STEP_COST_ROAD
    } else {
        crate::tables::STEP_COST_OPEN
    };
    u.moves_used += base;
    out.charged += base;

    let (dx, dy) = (nx as i32 - u.x as i32, ny as i32 - u.y as i32);
    if let Some(d) = STEP_DIRECTIONS.iter().position(|&s| s == (dx, dy)) {
        u.facing = d as u8;
    }
    u.x = nx;
    u.y = ny;
    u.path.remove(0);
    out.moved = true;

    // `Army_Tick` keeps `+0x10` equal to the tile's county byte and calls
    // `Unit_EnterCounty` whenever it changes.
    if u.county != tile_county {
        u.county = tile_county;
        out.entered_county = Some(tile_county);
    }
    Some(out)
}

pub fn march(
    map: &mut CampaignMap,
    counties: &mut [County; MAX_COUNTIES],
    realms: &[Realm; MAX_REALMS],
    units: &mut Units,
    id: usize,
) -> Vec<Step> {
    let mut steps = Vec::new();
    while let Some(s) = step(map, counties, realms, units, id) {
        let stop = s.entry.ends_the_move();
        steps.push(s);
        if stop {
            break;
        }
    }
    // **Where the walk stops.** [`step`] does not stop a unit on the commit
    // that empties its path, because `Unit_Step` (`0x00465D28`) does not: the
    // unit crosses into its last tile and the latched arm finds `field_0x1c ==
    // 0` at that tile's edge. `march` has no sub-tile counter, so the end of the
    // loop *is* that edge
    // [`crate::Kingdom::tick_units`] makes when the edge is really reached.
    if let Some(u) = units.get_mut(id) {
        if u.moving && u.path.is_empty() {
            u.moving = false;
            u.needs_destination = true;
        }
    }
    steps
}

/// `Unit_CrossField` (`0x0046673C`) — the extra 3 moves
///
/// > `[D]` — a plain reading of one `if`, and a negative claim, which is the
/// > weaker kind.
fn cross_field(
    map: &mut CampaignMap,
    counties: &mut [County; MAX_COUNTIES],
    units: &mut Units,
    id: usize,
    x: u8,
    y: u8,
    owner: u8,
    owner_is_human: bool,
) -> (i32, Option<u8>, Option<Offence>) {
    let Some(u) = units.get(id) else { return (0, None, None) };
    if matches!(u.kind, UnitKind::Merchant | UnitKind::Transport) {
        return (0, None, None);
    }
    if let Some(u) = units.get_mut(id) {
        u.moves_used += crate::tables::STEP_COST_FIELD_EXTRA;
    }
    let charged = crate::tables::STEP_COST_FIELD_EXTRA;

    let tile_county = map.county_at(x, y);
    let county_owner = counties.get(tile_county as usize).map_or(0, |c| c.owner);
    if county_owner == owner || !map.is_standing_field(x, y) {
        return (charged, None, None);
    }
    let offence = owner_is_human.then_some(Offence {
        against: county_owner,
        by: owner,
        amount: FIELD_TRAMPLE_OFFENCE,
    });
    if let Some(county) = counties.get_mut(tile_county as usize) {
        destroy_field(county, map, x, y);
    }
    (charged, Some(tile_county), offence)
}

/// `County_DestroyField` (`0x00469E5B`) — remove one field and its share of
/// what it was carrying.
///
/// ```c
/// if (terrain < 0x0F && (byte)+0x206 != 0) {                     /* a grain field */
///     share = (fieldsGrain <= +0x206) ? PctOf(1, +0x206) : 0;
///     lost  = Pct(crop, min(share, 100));
///     crop -= lost;   cropLost += lost;
///     if (fieldsGrain <= +0x206) +0x206--;
///     fieldsGrain--;
/// } else if (terrain > 0x0E && fieldsCattle != 0 && herd != 0) {   /* a pasture */
///     lost = Pct(herd, PctOf(1, fieldsCattle));
///     herd -= lost;   herdLost += lost;
///     fieldsCattle--;
/// }
/// Terrain_Set(tile, 0, 0);                                        /* always */
/// ```
///
/// **`PctOf(1, n)` is `100 / n`** — what share of the county's fields this one
/// field is — and `Pct(crop, share)` then takes that share of the crop. So
/// wrecking one of eight grain fields costs an eighth of the standing crop, and
/// the crop that goes is `+0x244`, the middle of the three growth stages.
///
/// **`+0x206` is [`County::fields_grain_standing`]**, and until it existed this
/// divided by `fields_grain` and said so. The difference is the `else 0`: a
/// county that has painted *more* grain than it sowed loses no crop at all
/// when one field is wrecked, because the share is only charged while
/// `fieldsGrain` is within the sown count. And the step-down is what
/// `Grain_SeasonTick`'s picture divides by
/// wheat on every other tile of the county. `docs/decisions.md`
/// C195.
///
/// both arms returned early when their guard failed and left the tile
/// standing. `Terrain_Set(tile, 0)` is outside the `if`. `[D]`
///
/// The original also accumulates what was lost into two display fields
/// (`+0x234` and `+0x27C`) that [`County`] does not carry; the loss is
/// returned instead.
pub fn destroy_field(county: &mut County, map: &mut CampaignMap, x: u8, y: u8) -> i32 {
    use crate::math::{pct, pct_of};
    let terrain_byte = map.terrain_at(x, y);
    let standing = county.fields_grain_standing & 0xFF;
    let lost = if terrain_byte < terrain::PASTURE_FROM && standing != 0 {
        let within = county.fields_grain <= standing;
        let share = if within { pct_of(1, standing).min(100) } else { 0 };
        let lost = pct(county.crop[1], share);
        county.crop[1] -= lost;
        if within {
            county.fields_grain_standing -= 1;
        }
        county.fields_grain -= 1;
        lost
    } else if terrain_byte >= terrain::PASTURE_FROM && county.fields_cattle != 0 && county.herd != 0 {
        let share = pct_of(1, county.fields_cattle);
        let lost = pct(county.herd, share);
        county.herd -= lost;
        county.fields_cattle -= 1;
        lost
    } else {
        0
    };
    map.set_terrain(x, y, 0);
    lost
}

/// `Unit_TrampleTile` (`0x0046873F`) — an army marching over a resource site
/// shuts it down.
///
/// **`[V]` the placer and the trampler agree three for three.**
fn trample(
    map: &mut CampaignMap,
    counties: &mut [County; MAX_COUNTIES],
    _realms: &[Realm; MAX_REALMS],
    units: &mut Units,
    id: usize,
    x: u8,
    y: u8,
) -> (i32, Option<(u8, usize)>) {
    let Some(u) = units.get(id) else { return (0, None) };
    if matches!(u.kind, UnitKind::Merchant | UnitKind::Transport) {
        return (0, None);
    }
    let owner = u.owner;
    let tile_county = map.county_at(x, y);
    let Some(county) = counties.get_mut(tile_county as usize) else { return (0, None) };
    if county.owner == owner {
        return (0, None);
    }
    let t = map.terrain_at(x, y);
    let Some((ruined, record)) = RUIN_LADDER.iter().copied().find(|&(from, to, _)| t >= from && t < to).map(|(_, to, r)| (to, r)) else {
        return (0, None);
    };
    if t == ruined {
        return (0, None);
    }
    map.set_terrain(x, y, ruined);
    county.industry[record].efficiency = 0;
    county.industry[record].disabled_seasons = crate::tables::TRAMPLE_DISABLED_SEASONS;
    if let Some(u) = units.get_mut(id) {
        u.moves_used += crate::tables::STEP_COST_TRAMPLE;
    }
    (crate::tables::STEP_COST_TRAMPLE, Some((tile_county, record)))
}

