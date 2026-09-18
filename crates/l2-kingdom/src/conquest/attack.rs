#![allow(unused_imports)]
use super::*;
use super::ownership::*;
use super::castle::*;
use crate::county::{County, MAX_COUNTIES};
use crate::levy::{self, Defence};
use crate::map::CampaignMap;
use crate::realm::{Realm, MAX_REALMS};
use crate::tables::{Season, Tables};
use crate::unit::{ArmyNames, UnitKind, Units};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Attack {
    Refused(Refusal),
    Captured(Capture),
    Battle { attacker: usize, defender: usize },
}

pub fn can_be_entered(counties: &[County; MAX_COUNTIES], units: &Units, county: u8, owner: u8) -> bool {
    let Some(c) = counties.get(county as usize) else { return false };
    if c.castle_type == 0 || c.garrison_unit == 0 {
        return true;
    }
    units.get(c.garrison_unit).map(|g| g.owner) == Some(owner)
}

/// `Army_AttackCounty` (`FUN_004A6C68`, `0x004A6C68`) — an army has reached a
/// county's castle.
///
/// **A newly raised defence is flagged**, `unit[+0x167] = 1` for one raised on
/// the spot and `2` for an existing army pressed into the role.
///
/// `Battle_ReturnToCampaign` reads that byte to decide whether winning the
/// battle also wins the county — so `+0x167`, which `docs/armies.md` §1.5 lists
/// among the offsets *"not traced"*, is **the county-defence marker**. Written
/// here, read there: two sites, `[D]`.
///
/// The **existing-defender search** is `County_FindDefendingArmy`
/// (`FUN_0046D42C`), and it is a 4×4 block
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
    explored: &mut crate::explore::Explored,
    restore: Restore,
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
    let mut mark = RAISED;
    let defender = if county_owner == 0 {
        if counties[county as usize].happiness < SURRENDER_HAPPINESS {
            None
        } else {
            let percent = levy::MILITIA_PERCENT_BY_DIFFICULTY
                [(difficulty as usize).min(levy::MILITIA_PERCENT_BY_DIFFICULTY.len() - 1)];
            levy::raise_defence(
                t, map, counties, realms, units, names, county, percent, Defence::Militia, year,
                explored,
            )
        }
    } else {
        let owner_is_human = realms.get(county_owner as usize).is_some_and(|r| r.is_human);
        match find_defender(units, counties, county) {
            Some(existing) => {
                // **The asymmetry is the original's.** `+0x167 = 2` is written
                // on the AI branch and *not* on the human one, so an existing
                // army defending a person's county is never marked at all.
                mark = if owner_is_human { UNMARKED } else { PRESSED };
                Some(existing)
            }
            None => {
                let mode = if owner_is_human { Defence::HumanCounty } else { Defence::AiCounty };
                levy::raise_defence(
                    t, map, counties, realms, units, names, county, levy::DEFENCE_PERCENT, mode, year,
                    explored,
                )
            }
        }
    };

    match defender {
        None => Attack::Captured(change_owner(
            t, counties, realms, units, owner, county, difficulty, map, explored, restore,
        )),
        Some(defender) => {
            if let Some(d) = units.get_mut(defender) {
                d.defence_mark = mark;
                if mark == RAISED {
                    d.move_allowance = 0;
                    d.moves_used = 0;
                }
            }
            Attack::Battle { attacker, defender }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn march_and_fight(
    t: &Tables,
    campaign: &mut crate::kingdom::Campaign,
    counties: &mut [County; MAX_COUNTIES],
    realms: &mut [Realm; MAX_REALMS],
    id: usize,
    difficulty: u8,
    year: i32,
    restore: Restore,
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
            &mut campaign.explored,
            restore,
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
/// `[V]` on the arithmetic, which closes exactly: the scan advances `+8` per
/// column and `+0x1E0` to the next row, and `512 − 4×8 = 480 = 0x1E0`, so the
/// block is four wide and four tall and nothing else fits.
pub fn find_defender(units: &Units, counties: &[County; MAX_COUNTIES], county: u8) -> Option<usize> {
    let c = counties.get(county as usize)?;
    let owner = c.owner;
    if owner == 0 {
        return None;
    }
    let (ax, ay) = (c.anchor_x as i32, c.anchor_y as i32);
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

