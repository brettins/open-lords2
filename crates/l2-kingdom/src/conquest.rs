//! **Taking a county** — the mechanism the whole campaign layer exists to
//! produce, and the one `docs/armies.md` never names.
//!
//! The document traces the record, the levy, movement, supply, sieges and how a
//! battle *result* comes back, and there is a hole in the middle of it: what
//! makes an army standing on a county's castle *take* the county. That is
//! `FUN_004A6C68` (`0x004A6C68`, 1,264 bytes), called from the mover, and
//! nothing in the document mentions it, `FUN_004A50AE` which raises the
//! defence, or `FUN_004A72FE` which changes the owner. Three unnamed functions
//! between "armies move" and "there is a war". They are named here as
//! `Army_AttackCounty`, `County_RaiseDefence` (in [`crate::levy`]) and
//! `County_ChangeOwner`.
//!
//! # How a county is attacked at all
//!
//! **By stepping onto its castle tile.** `Unit_StepOnce` returns 5 for a
//! `plane0 & 0x40` tile, the move ends, and the mover then calls
//! `Transport_Deliver` and `Army_AttackCounty` with the tile's county. So the
//! castle site is not merely expensive terrain the pathfinder routes around —
//! it is the objective, and `docs/armies.md` §2.2's castle row (*"5 →
//! `Transport_Deliver`, move ends"*) is half the story. `[D]`
//!
//! # The siege gate is a single `if`
//!
//! ```c
//! unit.type == 1 && unit.owner != 0 && county.owner != unit.owner
//!   && (county.castleType == 0 || county.garrisonUnit == 0
//!       || garrison.owner == unit.owner)
//! ```
//!
//! A county with **both** a castle and a garrison in it cannot be walked into
//! at all — that is what forces a siege, and it is one condition rather than a
//! subsystem. Everything else falls through to a battle or an outright capture.
//! `[D]`
//!
//! # Scope
//!
//! Sieges and the battle handoff are out of this crate's scope. What is here is
//! the guard, the defence decision, and the capture; when the outcome is a
//! battle this reports it and stops, exactly the way
//! [`crate::movement::Offence`] reports a diplomatic hit rather than inventing
//! a diplomacy layer.

use crate::county::{County, MAX_COUNTIES};
use crate::levy::{self, Defence};
use crate::map::CampaignMap;
use crate::realm::{Realm, MAX_REALMS};
use crate::tables::Tables;
use crate::unit::{ArmyNames, UnitKind, Units};

/// The happiness a neutral county must be **below** to surrender without a
/// fight — `if (county.happiness < 11) g_battleArmyB = 0`.
///
/// In the shipped save every neutral county sits at 77, so on the England map
/// **every neutral county is a battle and none is a walk-in** at turn one. The
/// rule only bites once a county has been taxed or starved into misery, which
/// is the *"the people are wretched, my liege"* county of
/// `docs/armies.md` §2.5's greeting table. `[D]`
pub const SURRENDER_HAPPINESS: i32 = 11;

/// The happiness a county loses the moment it changes hands, by who took it.
///
/// ```c
/// penalty = realm.isHuman ? (difficulty * 20 + 10) : 30;
/// ```
///
/// **An AI conqueror always costs 30; a human's cost rises with the
/// difficulty** — 10 at Easy, 30 at Normal, 50 at Hard. So the two are equal at
/// Normal and the setting decides whether conquest is cheaper or dearer for the
/// player than for the AI. `[D]`
pub fn capture_happiness_penalty(conqueror_is_human: bool, difficulty: u8) -> i32 {
    if conqueror_is_human {
        difficulty as i32 * 20 + 10
    } else {
        30
    }
}

/// What happened when an army reached a county's castle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Attack {
    /// The guard refused: the unit is not an army, is ownerless, already owns
    /// the county, or the county has a castle *and* a garrison that is not the
    /// attacker's. The last is the siege case.
    Refused(Refusal),
    /// Nobody defended it. The county has changed hands.
    Captured,
    /// A defence was raised or found, and a battle is due between these two
    /// units. **This crate does not fight it** — the caller hands the pair to
    /// `l2-sim` and brings the result back.
    Battle { attacker: usize, defender: usize },
}

/// Why [`attack_county`] refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    NotAnArmy,
    /// The unit's owner byte is 0. An ownerless militia carries 6, not 0, so
    /// this really only excludes an empty slot.
    Ownerless,
    /// It is already yours.
    AlreadyYours,
    /// A castle **and** a garrison, and the garrison is not yours. This is the
    /// siege gate, and the only refusal that is a rule rather than a guard.
    Garrisoned,
}

/// The siege gate, on its own, because it is the interesting half of the guard
/// and a caller will want to ask it before ordering a march.
///
/// True when an army of `owner` could walk into the county — i.e. when the
/// county has no castle, or no garrison, or a garrison that is already the
/// attacker's.
pub fn can_be_entered(counties: &[County; MAX_COUNTIES], units: &Units, county: u8, owner: u8) -> bool {
    let Some(c) = counties.get(county as usize) else { return false };
    if c.castle_type == 0 || c.garrison_unit == 0 {
        return true;
    }
    units.get(c.garrison_unit).map(|g| g.owner) == Some(owner)
}

/// The moves an attack costs the attacker, charged before anything is decided.
pub const ATTACK_MOVE_COST: i32 = 8;

/// `Army_AttackCounty` (`FUN_004A6C68`, `0x004A6C68`) — an army has reached a
/// county's castle.
///
/// ```c
/// guard...
/// unit.movesUsed += 8;
/// unit.needsDestination = 1;
/// g_battleCounty = county;  g_battleIsSiege = 0;
/// if (county.owner == 0) {
///     if (county.happiness < 11) defender = 0;                /* surrender */
///     else defender = County_RaiseDefence(county, 25|40|50|60 by difficulty, 40, mode 2);
/// } else if (!realm[county.owner].isHuman) {
///     defender = FindDefender(county) or County_RaiseDefence(county, 40, 40, mode 1);
/// } else {
///     defender = FindDefender(county) or County_RaiseDefence(county, 40, 40, mode 0);
/// }
/// if (defender == 0) County_ChangeOwner(unit.owner, county);
/// else               ...battle...
/// ```
///
/// **A newly raised defence is flagged**, `unit[+0x167] = 1` for one raised on
/// the spot and `2` for an existing army pressed into the role.
/// `Battle_ReturnToCampaign` reads that byte to decide whether winning the
/// battle also wins the county — so `+0x167`, which `docs/armies.md` §1.5 lists
/// among the offsets *"not traced"*, is **the county-defence marker**. Written
/// here, read there: two sites, `[D]`.
///
/// The **existing-defender search** is `County_FindDefendingArmy`
/// (`FUN_0046D42C`), and it is not a scan of the county — it is a 4×4 block
/// around the county *town*, returning the largest army in it. See
/// [`find_defender`], which has the function.
#[allow(clippy::too_many_arguments)]
pub fn attack_county(
    t: &Tables,
    map: &CampaignMap,
    counties: &mut [County; MAX_COUNTIES],
    realms: &mut [Realm; MAX_REALMS],
    units: &mut Units,
    names: &mut ArmyNames,
    attacker: usize,
    county: u8,
    difficulty: u8,
    year: i32,
) -> Attack {
    let Some(u) = units.get(attacker) else { return Attack::Refused(Refusal::NotAnArmy) };
    if u.kind != UnitKind::Army {
        return Attack::Refused(Refusal::NotAnArmy);
    }
    if u.owner == 0 {
        return Attack::Refused(Refusal::Ownerless);
    }
    let owner = u.owner;
    let Some(c) = counties.get(county as usize) else { return Attack::Refused(Refusal::NotAnArmy) };
    if c.owner == owner {
        return Attack::Refused(Refusal::AlreadyYours);
    }
    if !can_be_entered(counties, units, county, owner) {
        return Attack::Refused(Refusal::Garrisoned);
    }

    if let Some(u) = units.get_mut(attacker) {
        u.moves_used += ATTACK_MOVE_COST;
        u.needs_destination = true;
        u.moving = false;
    }

    let county_owner = counties[county as usize].owner;
    // The mark that goes on the defender: `RAISED` for one levied on the spot,
    // `PRESSED` for an army that was already standing there. See
    // [`crate::unit::Unit::defence_mark`].
    let mut mark = RAISED;
    let defender = if county_owner == 0 {
        if counties[county as usize].happiness < SURRENDER_HAPPINESS {
            None
        } else {
            let percent = levy::MILITIA_PERCENT_BY_DIFFICULTY
                [(difficulty as usize).min(levy::MILITIA_PERCENT_BY_DIFFICULTY.len() - 1)];
            levy::raise_defence(
                t, map, counties, realms, units, names, county, percent, Defence::Militia, year,
            )
        }
    } else {
        let owner_is_human = realms.get(county_owner as usize).is_some_and(|r| r.is_human);
        match find_defender(units, counties, county) {
            Some(existing) => {
                // **The asymmetry is the original's.** `+0x167 = 2` is written
                // on the AI branch and *not* on the human one, so an existing
                // army defending a person's county is never marked at all.
                // `Defence_Disband` then never touches it — the same outcome
                // the 2 would have produced, reached by not writing anything.
                mark = if owner_is_human { UNMARKED } else { PRESSED };
                Some(existing)
            }
            None => {
                let mode = if owner_is_human { Defence::HumanCounty } else { Defence::AiCounty };
                levy::raise_defence(
                    t, map, counties, realms, units, names, county, levy::DEFENCE_PERCENT, mode, year,
                )
            }
        }
    };

    match defender {
        None => {
            change_owner(counties, realms, units, owner, county, difficulty);
            Attack::Captured
        }
        Some(defender) => {
            if let Some(d) = units.get_mut(defender) {
                d.defence_mark = mark;
                if mark == RAISED {
                    // `Unit_Spawn` memsets the record and nothing writes the
                    // allowance until `Army_Tick` runs, so a defence levied
                    // this instant has **no moves**: it stands where it was
                    // raised and fights. It never sees another tick — the
                    // battle disbands it — so this is the whole of its
                    // movement rule. `battle-during.sav` slot 6 reads 0/0.
                    d.move_allowance = 0;
                    d.moves_used = 0;
                }
            }
            Attack::Battle { attacker, defender }
        }
    }
}

/// `+0x167 = 1` — a defence levied out of the county's own people the moment
/// the attacker arrived. Winning with it takes the county; it is dissolved
/// afterwards and its survivors go home.
pub const RAISED: u8 = 1;
/// `+0x167 = 2` — an army that was already standing at the town. Winning with
/// it takes the county; it keeps its men and only loses the mark.
pub const PRESSED: u8 = 2;
/// No mark. A field battle, and a human's existing defender, which the original
/// never marks.
pub const UNMARKED: u8 = 0;

/// Walk a unit for as long as it can walk, and resolve a castle it reaches.
///
/// This is the composition the campaign layer exists to provide, and the one a
/// turn actually calls: [`crate::movement::march`] takes the army as far as its
/// moves allow, and if the last step brought it to a county's castle,
/// [`attack_county`] decides whether that is a capture, a battle or a siege it
/// cannot start.
///
/// Returns the steps and the outcome of the castle, if it reached one.
#[allow(clippy::too_many_arguments)]
pub fn march_and_fight(
    t: &Tables,
    campaign: &mut crate::kingdom::Campaign,
    counties: &mut [County; MAX_COUNTIES],
    realms: &mut [Realm; MAX_REALMS],
    id: usize,
    difficulty: u8,
    year: i32,
) -> (Vec<crate::movement::Step>, Option<Attack>) {
    let steps = crate::movement::march(
        &mut campaign.map,
        counties,
        realms,
        &mut campaign.units,
        id,
    );
    let castle = steps.iter().rev().find_map(|s| s.reached_castle);
    let outcome = castle.map(|county| {
        attack_county(
            t,
            &campaign.map,
            counties,
            realms,
            &mut campaign.units,
            &mut campaign.names,
            id,
            county,
            difficulty,
            year,
        )
    });
    (steps, outcome)
}

/// `County_FindDefendingArmy` (`FUN_0046D42C`, `0x0046D42C`) — the army already
/// standing at the county's town, which defends it instead of a fresh levy.
///
/// ```c
/// uint County_FindDefendingArmy(int county) {
///     ax = county.anchorX;  ay = county.anchorY;          /* +0x6C / +0x6D */
///     if (ax - 2 < 0 || ax + 2 > 64 || ay - 2 < 0 || ay + 2 > 64) return 0;
///     best = 0;  bestMen = 0;
///     for (y = ay - 2; y < ay + 2; y++)
///       for (x = ax - 2; x < ax + 2; x++) {
///         u = g_tiles[y*64 + x].unit;                     /* the +5 plane */
///         if (u && units[u].owner == county.owner && units[u].kind == 1
///               && units[u].men > bestMen) { best = u; bestMen = units[u].men; }
///       }
///     return best;
/// }
/// ```
///
/// **This was modelled wrong in both halves until the function was read**, as
/// *"the lowest-numbered army of the county's owner standing in the county"*.
/// Both the scope and the tie-break were wrong:
///
/// * **The scope** is a **4×4 tile block around the county town**, not the
///   county. An army three tiles from the town does not defend it however deep
///   inside the county it stands.
/// * **The tie-break** is the **largest** army by [`crate::unit::Unit::men`],
///   not the first slot.
///
/// `[V]` on the arithmetic, which closes exactly: the scan advances `+8` per
/// column and `+0x1E0` to the next row, and `512 − 4×8 = 480 = 0x1E0`, so the
/// block is four wide and four tall and nothing else fits.
///
/// **The asymmetric `−2 … +1` window is the tell that the reading is right.**
/// It looks like an off-by-one until you know the county town is a 2×2 block
/// whose *bottom-right* corner is the anchor — with that, the window is exactly
/// the town plus the one-tile ring around it, and the rule states in a
/// sentence: **an army defends its county town by standing on it or beside it.**
///
/// The two readings disagree on shipped data. In `battle-before.sav` county 2's
/// town anchor is (31, 50) and its owner's only army stands at (30, 46) — the
/// county's own castle tile, four rows north of the town. The old version
/// returned that army and the original returns 0. Both are run against those
/// bytes in `tests/defence.rs`.
///
/// The original reads the occupying unit out of the tile record's `+5`
/// occupancy plane; [`crate::map::CampaignMap`] deliberately does not carry that
/// plane, so [`Units::at`] answers the same question from the unit array. The
/// tiles are visited in the original's row-major order, so a tie between two
/// equally large armies falls the same way.
pub fn find_defender(units: &Units, counties: &[County; MAX_COUNTIES], county: u8) -> Option<usize> {
    let c = counties.get(county as usize)?;
    let owner = c.owner;
    if owner == 0 {
        return None;
    }
    let (ax, ay) = (c.anchor_x as i32, c.anchor_y as i32);
    // The original's own bounds check, and it is `+2` on both sides even though
    // the scan only reaches `+1`: a town within two tiles of an edge defends
    // itself with a levy and nothing else.
    let dim = crate::map::MAP_DIM as i32;
    if ax - 2 < 0 || ax + 2 > dim || ay - 2 < 0 || ay + 2 > dim {
        return None;
    }

    let mut best = None;
    let mut best_men = 0;
    for y in (ay - 2)..(ay + 2) {
        for x in (ax - 2)..(ax + 2) {
            let Some(i) = units.at(x as u8, y as u8) else { continue };
            let Some(u) = units.get(i) else { continue };
            if u.kind == UnitKind::Army && u.owner == owner && u.men > best_men {
                best = Some(i);
                best_men = u.men;
            }
        }
    }
    best
}

/// `County_ChangeOwner` (`FUN_004A72FE`, `0x004A72FE`) — the county changes
/// hands.
///
/// ```c
/// realm[new].countyCount++;
/// ...messages, one of nine, by how many counties the taker now holds...
/// county.owner  = new;
/// penalty = realm[new].isHuman ? difficulty * 20 + 10 : 30;
/// if (county.happiness < penalty) { shownEvents -= happiness; happiness = 0; }
/// else                            { happiness -= penalty; shownEvents -= penalty; }
/// county.shield = realm[new].shield;
/// realm[new].peakCounties = max(peakCounties, countyCount);
/// ```
///
/// Two things worth naming. **The conquest penalty is drawn on the *"From
/// events"* line** (`+0x17`, `L2.eng` group 85 index 9), not on the army line —
/// so a county the player has just taken shows its resentment where a plague or
/// a fire would show. And the same clamp shape as the levy: the panel is
/// debited what was actually taken.
///
/// The nine messages, `0x72`…`0x7E`, are the *"we have taken"* / *"we have
/// lost"* letters; this crate has no message layer for them yet and they are
/// left to the caller.
///
/// Returns the happiness the county lost.
pub fn change_owner(
    counties: &mut [County; MAX_COUNTIES],
    realms: &mut [Realm; MAX_REALMS],
    units: &Units,
    new_owner: u8,
    county: u8,
    difficulty: u8,
) -> i32 {
    let is_human = realms.get(new_owner as usize).is_some_and(|r| r.is_human);
    let Some(c) = counties.get_mut(county as usize) else { return 0 };

    c.owner = new_owner;
    let penalty = capture_happiness_penalty(is_human, difficulty);
    if c.happiness < penalty {
        c.shown_events -= c.happiness;
        c.happiness = 0;
    } else {
        c.happiness -= penalty;
        c.shown_events -= penalty;
    }

    // The original also writes county `+0x07` from the taker's shield byte, so
    // the county draws the new banner. [`County`] has no such field — it is
    // presentation, and the realm's own `shield_index` is where a renderer
    // would read it from.

    // The castle, if any, is no longer garrisoned by the loser. The original
    // clears `+0x1BC` on the battle path rather than here; doing it here as
    // well is the same state and keeps a walk-in capture from leaving a
    // garrison pointing at a county its owner no longer holds.
    let garrison = c.garrison_unit;
    if garrison != 0 && units.get(garrison).map(|u| u.owner) != Some(new_owner) {
        counties[county as usize].garrison_unit = 0;
    }

    recount_realm_counties(counties, realms);
    penalty
}

/// Realm `+0x29` — how many counties each realm holds, and `+0x2A`, the most it
/// has ever held. Rebuilt rather than incremented, because a capture moves a
/// county between two realms and incrementing one without decrementing the
/// other is how a count drifts.
pub fn recount_realm_counties(counties: &[County; MAX_COUNTIES], realms: &mut [Realm; MAX_REALMS]) {
    for r in realms.iter_mut() {
        r.county_count = 0;
    }
    for c in counties.iter().skip(1) {
        if let Some(r) = realms.get_mut(c.owner as usize) {
            if c.owner != 0 {
                r.county_count = r.county_count.saturating_add(1);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Reaching the castle building
// ---------------------------------------------------------------------------

/// What an army did when it walked onto a standing castle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CastleArrival {
    /// It is your county: the army is inside. The slot is the garrison — which
    /// is **not** the arriving army when it merged into one already there.
    Garrisoned(usize),
    /// Your county, and the castle will not hold that many men. Nothing moved.
    GarrisonFull,
    /// Somebody else's: a siege is laid, or was refused for one of
    /// [`crate::siege::SiegeRefusal`]'s reasons.
    Siege(Result<(), crate::siege::SiegeRefusal>),
    /// Not an army, or the slot is empty.
    NotAnArmy,
}

/// `Unit_ReachCastleBuilding` (`0x004686A0`) — **the fork an army walks into**,
/// and the only route into either half.
///
/// ```c
/// if (unit.kind != 1) return;
/// if (unit.owner == county.owner) Army_Garrison(unit, county);
/// else                            Army_BeginSiege(unit, county);
/// ```
///
/// Two unrelated functions agree on the test — `Map_HoverUnitTarget` offers
/// *"Garrison castle?"* or *"Besiege castle?"* from the same owner comparison —
/// which is what makes it `[V]`.
///
/// Note it is the **county's** owner, not the garrison's: marching onto a
/// castle in your own county that somebody else's garrison is sitting in
/// garrisons *into* them, and [`garrison_apply`] then merges the two armies.
/// That is the original's behaviour and it looks like a bug; it is unreachable
/// in practice because `County_ChangeOwner` evicts a foreign garrison.
pub fn reach_castle_building(
    t: &Tables,
    map: &CampaignMap,
    counties: &mut [County; MAX_COUNTIES],
    realms: &[Realm; MAX_REALMS],
    units: &mut Units,
    army: usize,
    county: u8,
    season: u8,
) -> CastleArrival {
    let Some(u) = units.get(army) else { return CastleArrival::NotAnArmy };
    if u.kind != UnitKind::Army {
        return CastleArrival::NotAnArmy;
    }
    let owner = u.owner;
    if counties.get(county as usize).map(|c| c.owner) == Some(owner) {
        match garrison_apply(t, map, counties, realms, units, army, county) {
            Some(slot) => CastleArrival::Garrisoned(slot),
            None => CastleArrival::GarrisonFull,
        }
    } else {
        CastleArrival::Siege(crate::siege::begin_siege(
            t, counties, realms, units, army, county, season,
        ))
    }
}

/// The moves `Army_GarrisonApply` charges for stepping inside.
pub const GARRISON_MOVE_COST: i32 = 5;

/// `Army_GarrisonApply` (`0x004A79A3`) — **put an army in the castle**.
///
/// ```c
/// men = unit.men + (county.garrisonUnit ? garrison.men : 0);
/// if (men > g_castleGarrisonCap[county.castleType]) { unit.state = 2; return; }
/// if (county.garrisonUnit == 0) {
///     county.garrisonUnit = unit;  unit.garrisonCounty = county;
///     unit.x = county.castleX;  unit.y = county.castleY;   /* a teleport */
///     unit.movesUsed += 5;  unit.destCounty = county;
/// } else Army_Combine(county.garrisonUnit, unit);
/// Army_RecountCountyTroops();
/// ```
///
/// **The move onto the castle tile is a teleport**, not a step: the army is
/// standing on the tile *outside* when this runs and is placed on the castle
/// block itself. That is why a garrison is drawn inside the castle rather than
/// beside it, and why the campaign map draws a garrisoned unit hollow.
///
/// Returns the garrison's slot, or `None` when the castle will not hold them —
/// the one refusal that lives in the body rather than in the move-order
/// confirmation. **Nothing is charged and nothing moves on a refusal**; the
/// army is left standing where it was.
///
/// `map` is read only, for the castle tile: `County_FindCastleTile` caches it in
/// county `+0x74`/`+0x75` and [`crate::map::castle_tile`] finds it instead.
pub fn garrison_apply(
    t: &Tables,
    map: &CampaignMap,
    counties: &mut [County; MAX_COUNTIES],
    realms: &[Realm; MAX_REALMS],
    units: &mut Units,
    army: usize,
    county: u8,
) -> Option<usize> {
    let sitting = counties.get(county as usize)?.garrison_unit;
    let castle_type = counties[county as usize].castle_type;
    if let Some(u) = units.get_mut(army) {
        u.needs_destination = true;
    }
    let men = units.get(army)?.men + units.get(sitting).map_or(0, |g| g.men);
    if men > crate::industry::garrison_cap(t, castle_type) {
        if let Some(u) = units.get_mut(army) {
            u.moving = false;
            // `unit.state = 2` in the C above, and **the mission has to go with
            // it**. Without this an AI lord marches the same men at the same
            // full castle every turn: step 7 gives the garrison order, the
            // order is refused here, the mission is still GARRISON, and next
            // turn it gives the same order. Found by `ai-lords-play` playing
            // forty turns, not by reading this function.
            u.mission = crate::ai_army::Mission::SEEK_ENEMY;
        }
        return None;
    }
    let slot = if sitting == 0 {
        let tile = crate::map::castle_tile(map, county);
        let u = units.get_mut(army)?;
        u.garrison_county = county;
        if let Some(tile) = tile {
            let (x, y) = crate::map::coords(tile);
            u.x = x;
            u.y = y;
        }
        u.path.clear();
        u.moving = false;
        u.needs_destination = true;
        u.player_driven = true;
        u.moves_used += GARRISON_MOVE_COST;
        u.dest_county = county;
        u.county = county;
        counties[county as usize].garrison_unit = army;
        army
    } else {
        // `Army_Combine` folds the newcomer into the sitting garrison and frees
        // the slot. A refusal there — two mercenary bands, or over the army cap
        // — leaves both armies standing, which is the original's silence.
        match crate::unit::combine(units, sitting, army) {
            Ok(_) => sitting,
            Err(_) => return None,
        }
    };
    units.recount_county_troops(counties, realms);
    Some(slot)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::{CampaignMap, MAP_DIM, MAP_TILES};
    use crate::unit::Unit;

    const T: &Tables = &Tables::DEFAULT;

    fn two_county_map() -> CampaignMap {
        let mut m = CampaignMap::empty();
        for i in 0..MAP_TILES {
            m.county[i] = if i % MAP_DIM < 32 { 1 } else { 2 };
        }
        m
    }

    fn world() -> ([County; MAX_COUNTIES], [Realm; MAX_REALMS]) {
        let mut counties: [County; MAX_COUNTIES] = core::array::from_fn(|_| County::new());
        let mut realms: [Realm; MAX_REALMS] = core::array::from_fn(|_| Realm::new());
        for id in 1..=2 {
            counties[id].population = 500;
            counties[id].happiness = 77;
        }
        counties[1].owner = 1;
        realms[1].in_play = true;
        realms[1].is_human = true;
        realms[1].shield_index = 2;
        realms[2].in_play = true;
        (counties, realms)
    }

    fn attacker(units: &mut Units, owner: u8, county: u8) -> usize {
        let mut u = Unit::new(UnitKind::Army, owner, 40, 10);
        u.men = 400;
        u.troops[0] = 400;
        u.county = county;
        u.home_county = 1;
        units.spawn(u).unwrap()
    }

    /// **The siege gate.** A castle alone is not enough, a garrison alone is
    /// not enough; both together, in someone else's hands, is what shuts the
    /// county.
    #[test]
    fn a_county_needs_both_a_castle_and_a_garrison_to_be_shut() {
        let (mut counties, _) = world();
        let mut units = Units::new();
        let garrison = units.spawn(Unit::new(UnitKind::Army, 1, 0, 0)).unwrap();

        counties[1].castle_type = 0;
        counties[1].garrison_unit = 0;
        assert!(can_be_entered(&counties, &units, 1, 2), "open country");

        counties[1].castle_type = 3;
        assert!(can_be_entered(&counties, &units, 1, 2), "a castle nobody is in");

        counties[1].castle_type = 0;
        counties[1].garrison_unit = garrison;
        assert!(can_be_entered(&counties, &units, 1, 2), "a garrison with no castle");

        counties[1].castle_type = 3;
        assert!(!can_be_entered(&counties, &units, 1, 2), "both: this is a siege");
        assert!(can_be_entered(&counties, &units, 1, 1), "…unless the garrison is yours");
    }

    /// In the shipped scenario **every county has `garrisonUnit = 0`**, so at
    /// turn one every county on the map is enterable and nothing is a siege.
    #[test]
    fn the_shipped_position_has_no_sieges_in_it() {
        let (counties, _) = world();
        let units = Units::new();
        for id in 1..=2u8 {
            assert_eq!(counties[id as usize].garrison_unit, 0);
            assert!(can_be_entered(&counties, &units, id, 9));
        }
    }

    #[test]
    fn a_shut_county_refuses_the_attack_rather_than_starting_a_battle() {
        let m = two_county_map();
        let (mut counties, mut realms) = world();
        let mut units = Units::new();
        let mut names = ArmyNames::new();
        let a = attacker(&mut units, 2, 1);
        let g = units.spawn(Unit::new(UnitKind::Army, 1, 1, 1)).unwrap();
        counties[1].castle_type = 4;
        counties[1].garrison_unit = g;

        let out = attack_county(T, &m, &mut counties, &mut realms, &mut units, &mut names, a, 1, 0, 1268);
        assert_eq!(out, Attack::Refused(Refusal::Garrisoned));
        assert_eq!(counties[1].owner, 1, "and nothing changed hands");
        assert_eq!(units.get(a).unwrap().moves_used, 0, "not even the eight moves");
    }

    /// A wretched neutral county surrenders without a fight — and that is the
    /// only walk-in there is.
    #[test]
    fn a_wretched_neutral_county_surrenders_and_a_contented_one_fights() {
        let m = two_county_map();
        let (mut counties, mut realms) = world();
        counties[2].happiness = 10;
        let mut units = Units::new();
        let mut names = ArmyNames::new();
        let a = attacker(&mut units, 1, 2);

        let out = attack_county(T, &m, &mut counties, &mut realms, &mut units, &mut names, a, 2, 0, 1268);
        assert_eq!(out, Attack::Captured);
        assert_eq!(counties[2].owner, 1);
        assert_eq!(units.get(a).unwrap().moves_used, ATTACK_MOVE_COST);

        // At 11 it fights.
        let (mut counties, mut realms) = world();
        counties[2].happiness = SURRENDER_HAPPINESS;
        let mut units = Units::new();
        let a = attacker(&mut units, 1, 2);
        let out = attack_county(T, &m, &mut counties, &mut realms, &mut units, &mut names, a, 2, 0, 1268);
        assert!(matches!(out, Attack::Battle { .. }), "got {out:?}");
        assert_eq!(counties[2].owner, 0, "the county is not taken by walking in");
    }

    /// The shipped position again: every neutral county is at 77, so **every
    /// neutral capture is a battle**.
    #[test]
    fn a_neutral_county_at_the_shipped_happiness_raises_a_militia() {
        let m = two_county_map();
        let (mut counties, mut realms) = world();
        let mut units = Units::new();
        let mut names = ArmyNames::new();
        let a = attacker(&mut units, 1, 2);

        let out = attack_county(T, &m, &mut counties, &mut realms, &mut units, &mut names, a, 2, 0, 1268);
        let Attack::Battle { attacker: att, defender } = out else { panic!("got {out:?}") };
        assert_eq!(att, a);
        let d = units.get(defender).unwrap();
        assert_eq!(d.owner, crate::levy::OWNERLESS);
        assert_eq!(d.men, 125, "a quarter of five hundred, at difficulty 0");
        assert_eq!(d.county, 2);
    }

    /// A county too small to raise a defence is captured outright even at full
    /// happiness — the [`crate::levy::DEFENCE_MIN_POPULATION`] floor.
    #[test]
    fn a_county_of_fewer_than_forty_people_is_simply_taken() {
        let m = two_county_map();
        let (mut counties, mut realms) = world();
        counties[2].population = 30;
        let mut units = Units::new();
        let mut names = ArmyNames::new();
        let a = attacker(&mut units, 1, 2);
        assert_eq!(
            attack_county(T, &m, &mut counties, &mut realms, &mut units, &mut names, a, 2, 0, 1268),
            Attack::Captured
        );
        assert_eq!(counties[2].owner, 1);
    }

    #[test]
    fn an_existing_army_in_an_owned_county_defends_it_rather_than_a_fresh_levy() {
        let m = two_county_map();
        let (mut counties, mut realms) = world();
        counties[2].owner = 2;
        counties[2].anchor_x = 45;
        counties[2].anchor_y = 20;
        let mut units = Units::new();
        let mut names = ArmyNames::new();
        let a = attacker(&mut units, 1, 2);
        // On the town's own anchor tile, which is inside the 4x4 window.
        let mut standing = Unit::new(UnitKind::Army, 2, 45, 20);
        standing.men = 300;
        standing.county = 2;
        let existing = units.spawn(standing).unwrap();

        let out = attack_county(T, &m, &mut counties, &mut realms, &mut units, &mut names, a, 2, 0, 1268);
        assert_eq!(out, Attack::Battle { attacker: a, defender: existing });
        assert_eq!(units.len(), 2, "nothing new was levied");
        assert_eq!(counties[2].population, 500, "and nobody was called up");
    }

    // --- the defender search ------------------------------------------------

    /// An army of `owner`, `men` strong, standing on `(x, y)` in county 2.
    fn standing(units: &mut Units, owner: u8, x: u8, y: u8, men: i32) -> usize {
        let mut u = Unit::new(UnitKind::Army, owner, x, y);
        u.men = men;
        u.county = 2;
        units.spawn(u).unwrap()
    }

    /// County 2 owned by realm 2, with its town anchored where the caller says.
    fn owned_county_two(anchor: (u8, u8)) -> ([County; MAX_COUNTIES], [Realm; MAX_REALMS]) {
        let (mut counties, mut realms) = world();
        counties[2].owner = 2;
        counties[2].anchor_x = anchor.0;
        counties[2].anchor_y = anchor.1;
        realms[2].is_human = false;
        (counties, realms)
    }

    /// **The case that was wrong on shipped data.**
    ///
    /// In `battle-before.sav` county 2's town anchor is (31, 50) and the only
    /// army its owner has — its castle garrison — stands at (30, 46), one
    /// column left and *four rows north*. The reading this function used to
    /// carry, *"the lowest-numbered army of the county's owner standing in the
    /// county"*, returns that army. `County_FindDefendingArmy` scans four rows
    /// around the anchor and returns 0, so the county levies a fresh defence
    /// instead. `tests/defence.rs` runs both readings on the real bytes.
    ///
    /// The control below is the same army moved to (30, 49), which *is* in the
    /// window — so the test fails for the geometry and not for some other
    /// reason.
    #[test]
    fn an_army_four_rows_from_the_town_does_not_defend_it() {
        let (counties, _) = owned_county_two((31, 50));

        let mut units = Units::new();
        standing(&mut units, 2, 30, 46, 300);
        assert_eq!(
            find_defender(&units, &counties, 2),
            None,
            "battle-before.sav: the army at (30,46) is outside the town's 4x4 block"
        );

        let mut units = Units::new();
        let close = standing(&mut units, 2, 30, 49, 300);
        assert_eq!(
            find_defender(&units, &counties, 2),
            Some(close),
            "…and one row nearer, it defends"
        );
    }

    /// **The window is `−2 … +1`, not `−2 … +2`.** The town is a 2×2 whose
    /// bottom-right corner is the anchor, so the block is the town plus the
    /// one-tile ring around it — asymmetric, and that asymmetry is the tell.
    #[test]
    fn the_defence_window_is_the_town_block_plus_its_one_tile_ring() {
        let (counties, _) = owned_county_two((31, 50));
        let inside = |x: u8, y: u8| {
            let mut units = Units::new();
            let u = standing(&mut units, 2, x, y, 300);
            find_defender(&units, &counties, 2) == Some(u)
        };

        for x in 29..=32u8 {
            for y in 48..=51u8 {
                assert!(inside(x, y), "({x},{y}) is inside the block");
            }
        }
        for (x, y) in [(28, 50), (33, 50), (31, 47), (31, 52), (28, 47), (33, 52)] {
            assert!(!inside(x, y), "({x},{y}) is outside it");
        }
    }

    /// **The tie-break is size, not slot order.** A small army spawned first
    /// does not beat a large one spawned second, which is exactly what the old
    /// `iter().find(..)` did.
    #[test]
    fn the_largest_army_beside_the_town_defends_it_not_the_earliest_slot() {
        let (counties, _) = owned_county_two((31, 50));
        let mut units = Units::new();
        let _small = standing(&mut units, 2, 29, 48, 40);
        let big = standing(&mut units, 2, 32, 51, 400);
        let _middling = standing(&mut units, 2, 31, 50, 200);
        assert_eq!(find_defender(&units, &counties, 2), Some(big));
    }

    /// Only the owner's own armies, and only armies. A besieger of another
    /// realm sitting on the town, and the owner's own merchant, are both
    /// invisible to it.
    #[test]
    fn the_defence_search_ignores_other_realms_and_other_unit_kinds() {
        let (counties, _) = owned_county_two((31, 50));
        let mut units = Units::new();
        standing(&mut units, 1, 31, 50, 900);
        let mut trader = Unit::new(UnitKind::Merchant, 2, 30, 50);
        trader.men = 900;
        trader.county = 2;
        units.spawn(trader).unwrap();
        assert_eq!(find_defender(&units, &counties, 2), None);

        let own = standing(&mut units, 2, 29, 49, 10);
        assert_eq!(find_defender(&units, &counties, 2), Some(own), "ten men still beat nobody");
    }

    /// The original's own bounds check: a town within two tiles of the map's
    /// edge finds nobody at all, whoever is standing beside it. `ax − 2 < 0` or
    /// `ax + 2 > 64` and the function returns 0 before it scans.
    #[test]
    fn a_town_against_the_map_edge_finds_nobody() {
        for anchor in [(1u8, 50u8), (63, 50), (31, 1), (31, 63)] {
            let (counties, _) = owned_county_two(anchor);
            let mut units = Units::new();
            standing(&mut units, 2, anchor.0, anchor.1, 300);
            assert_eq!(find_defender(&units, &counties, 2), None, "anchor {anchor:?}");
        }
    }

    #[test]
    fn attacking_your_own_county_is_refused() {
        let m = two_county_map();
        let (mut counties, mut realms) = world();
        let mut units = Units::new();
        let mut names = ArmyNames::new();
        let a = attacker(&mut units, 1, 1);
        assert_eq!(
            attack_county(T, &m, &mut counties, &mut realms, &mut units, &mut names, a, 1, 0, 1268),
            Attack::Refused(Refusal::AlreadyYours)
        );
    }

    #[test]
    fn a_merchant_cannot_take_a_county() {
        let m = two_county_map();
        let (mut counties, mut realms) = world();
        let mut units = Units::new();
        let mut names = ArmyNames::new();
        let trader = units.spawn(Unit::new(UnitKind::Merchant, 1, 40, 10)).unwrap();
        assert_eq!(
            attack_county(T, &m, &mut counties, &mut realms, &mut units, &mut names, trader, 2, 0, 1268),
            Attack::Refused(Refusal::NotAnArmy)
        );
    }

    // --- changing hands ----------------------------------------------------

    /// The capture penalty, and the surprise in it: it is drawn on the
    /// *"From events"* line rather than the army line.
    #[test]
    fn a_captured_county_loses_happiness_on_the_events_line() {
        let (mut counties, mut realms) = world();
        let units = Units::new();
        counties[2].happiness = 77;
        let lost = change_owner(&mut counties, &mut realms, &units, 1, 2, 0);
        assert_eq!(lost, 10, "a human at difficulty 0");
        assert_eq!(counties[2].happiness, 67);
        assert_eq!(counties[2].shown_events, -10);
        assert_eq!(counties[2].owner, 1);
    }

    /// An AI conqueror always costs 30; a human's cost tracks the difficulty,
    /// so the two are equal at Normal.
    #[test]
    fn the_conquest_penalty_is_flat_for_an_ai_and_scales_for_a_human() {
        for d in 0..=3u8 {
            assert_eq!(capture_happiness_penalty(false, d), 30, "difficulty {d}");
        }
        assert_eq!(capture_happiness_penalty(true, 0), 10);
        assert_eq!(capture_happiness_penalty(true, 1), 30, "equal at Normal");
        assert_eq!(capture_happiness_penalty(true, 2), 50);
    }

    #[test]
    fn a_county_that_cannot_pay_the_penalty_goes_to_zero_and_the_panel_agrees() {
        let (mut counties, mut realms) = world();
        let units = Units::new();
        counties[2].happiness = 4;
        assert_eq!(change_owner(&mut counties, &mut realms, &units, 1, 2, 2), 50);
        assert_eq!(counties[2].happiness, 0);
        assert_eq!(counties[2].shown_events, -4, "what was taken, not the fifty");
    }

    #[test]
    fn changing_hands_moves_the_county_between_the_two_realms_counts() {
        let (mut counties, mut realms) = world();
        let units = Units::new();
        counties[2].owner = 2;
        recount_realm_counties(&counties, &mut realms);
        assert_eq!((realms[1].county_count, realms[2].county_count), (1, 1));

        change_owner(&mut counties, &mut realms, &units, 1, 2, 0);
        assert_eq!((realms[1].county_count, realms[2].county_count), (2, 0));
    }

    /// **The whole slice, end to end**: levy an army out of a county, march it
    /// across the border, and take the neighbour.
    ///
    /// This is the turn `docs/plan.md` item 4 asks for, minus the battle. It
    /// exists because every other test here builds an army by hand, and a layer
    /// whose pieces each work but do not compose is the failure
    /// `docs/decisions.md` C21 is about.
    #[test]
    fn an_army_can_be_raised_marched_across_a_border_and_take_a_county() {
        use crate::levy::{self, LevyBasket, Muster};
        use crate::movement::Routing;

        let mut map = two_county_map();
        // A castle site on county 2's side, and a road most of the way to it.
        map.set_flags(50, 10, crate::map::flags::CASTLE);
        for x in 5..50u8 {
            map.set_flags(x, 10, crate::map::flags::ROAD);
        }
        let (mut counties, mut realms) = world();
        counties[2].population = 20; // too small to raise a defence
        realms[1].weapons = [100, 0, 0, 0, 0, 0];
        let mut units = Units::new();
        let mut names = ArmyNames::new();

        // Raise it.
        let levy = levy::set_percent(T, &counties[1], 20);
        assert_eq!(levy.men, 100);
        let mut basket = LevyBasket::seed(&realms[1], levy.men);
        basket.equip(crate::unit::TroopType::Crossbowman, 100);
        let army = levy::create_army(
            T, &map, &mut counties, &mut realms, &mut units, &mut names, &basket,
            Muster { realm: 1, county: 1, happiness_cost: levy.happiness_cost, year: 1268 },
        )
        .expect("county 1 has room");
        assert_eq!(counties[1].population, 400);
        assert_eq!(units.get(army).unwrap().men, 100);

        // March it. It is on a road for most of the way, so fifteen moves go a
        // long way, but not the whole way in one season.
        let mut campaign = crate::kingdom::Campaign::new();
        campaign.map = map;
        campaign.units = units;
        campaign.names = names;
        campaign.units.get_mut(army).unwrap().x = 5;
        campaign.units.get_mut(army).unwrap().y = 10;

        let mut seasons = 0;
        let outcome = loop {
            seasons += 1;
            assert!(seasons < 20, "it should not take twenty seasons to cross a map");
            crate::movement::order_move(&campaign.map, &mut campaign.units, army, (50, 10), Routing::Direct);
            let (_, outcome) =
                march_and_fight(T, &mut campaign, &mut counties, &mut realms, army, 0, 1268);
            if let Some(o) = outcome {
                break o;
            }
            campaign.units.reset_moves();
        };

        assert_eq!(outcome, Attack::Captured);
        assert_eq!(counties[2].owner, 1, "county 2 has changed hands");
        assert_eq!(realms[1].county_count, 2);
        assert_eq!(campaign.units.get(army).unwrap().county, 2, "and the army is standing in it");
    }

    #[test]
    fn taking_a_county_lifts_the_losers_garrison() {
        let (mut counties, mut realms) = world();
        let mut units = Units::new();
        let g = units.spawn(Unit::new(UnitKind::Army, 2, 0, 0)).unwrap();
        counties[2].owner = 2;
        counties[2].garrison_unit = g;
        change_owner(&mut counties, &mut realms, &units, 1, 2, 0);
        assert_eq!(counties[2].garrison_unit, 0);
    }
}
