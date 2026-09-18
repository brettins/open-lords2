#![allow(unused_imports)]
use super::*;
use super::settlement::*;
use crate::county::{County, MAX_COUNTIES};
use crate::math::pct;
use crate::realm::{Realm, MAX_REALMS};
use crate::tables::Tables;
use crate::unit::{ArmyNames, Unit, Units, TROOP_TYPES};

/// `Battle_ReturnToCampaign` (`0x004AB383`) — apply a settled battle to the
/// campaign.
///
/// > **Corrects `docs/armies.md` §7 and `docs/symbols.md` a third time.** Both
/// > say the loser survives with its siege lifted when it has ≥ 50 men
/// > *"under autocalc"*. The gate is `DAT_0056D5C8`, which
/// > [`auto_resolve`]'s first statement **clears**; it is raised in exactly one
/// > place, `UnitOrder_SiegeAttKnight`, when an all-knight AI besieger gives up
/// > on an unbreached wall. So it is a *siege-withdrawal* rule and under
/// > autocalc the loser is always destroyed. That path is unreachable from
/// > this crate and is deliberately not modelled here. `[V]` — the write and
/// > the three clears are the only four sites the flag has. See correction C31.
///
/// > ### ⚠ And C31 stopped one branch too early. **A besieger that loses but
/// > still has men is not destroyed either, and that rule has no flag on it.**
///
/// >
/// > ```c
/// > if (loser.besiegingCounty == 0 || loser.menTotal == 0) {
/// >     if (withdrawal) {
/// >         if (loser.menTotal < 50) { message 0x120; Army_Destroy(loser); }
/// >         else                       loser.besiegingCounty = 0;
/// >     } else Army_Destroy(loser);
/// > } else loser.besiegingCounty = 0;      /* <- the outer else */
/// > ```
/// >
/// > C31 read the inner test — the 50-men rule, gated on the withdrawal flag —
/// > and concluded that *"under autocalc the loser is always destroyed"*. That
/// > conclusion is true, but for a different reason than the one given: under
/// > autocalc the loser's men are set to **zero**, so the outer test passes and
/// > the inner one runs. In a **fought** siege the loser can walk off the field
/// > with men, and then the outer `else` fires: **the besieging army survives
/// > and its siege is lifted.** That is the rule a repulsed assault
/// > needs, it is reachable from `l2-sim` and from nowhere else, and both
/// > `docs/armies.md` §7 and C31 are silent about it. See correction C38.
#[allow(clippy::too_many_arguments)]
pub fn return_to_campaign(
    t: &Tables,
    counties: &mut [County; MAX_COUNTIES],
    realms: &mut [Realm; MAX_REALMS],
    units: &mut Units,
    names: &mut ArmyNames,
    verdict: Verdict,
    county: u8,
    is_siege: bool,
    withdrawal: bool,
    difficulty: u8,
    map: &crate::map::CampaignMap,
    explored: &mut crate::explore::Explored,
    restore: crate::conquest::Restore,
) -> Aftermath {
    let mut out = Aftermath::default();
    let (winner, loser) = (verdict.winner(), verdict.loser());
    let Some(loser_unit) = units.get(loser).cloned() else { return out };
    let Some(winner_unit) = units.get(winner).cloned() else { return out };

    if verdict.attacker_won {
        if loser_unit.garrison_county != 0 {
            if let Some(c) = counties.get_mut(county as usize) {
                c.garrison_unit = 0;
            }
            out.captures[0] = Some(crate::conquest::change_owner(
                t, counties, realms, units, winner_unit.owner, county, difficulty, map, explored,
                restore,
            ));
            out.county_taken_by = Some(winner_unit.owner);
        }
        if loser_unit.defence_mark != 0 {
            out.captures[1] = Some(crate::conquest::change_owner(
                t, counties, realms, units, winner_unit.owner, county, difficulty, map, explored,
                restore,
            ));
            out.county_taken_by = Some(winner_unit.owner);
        }
    } else if loser_unit.garrison_county != 0 {
        if let Some(c) = counties.get_mut(county as usize) {
            c.garrison_unit = 0;
        }
    }

    if let Some(w) = units.get_mut(winner) {
        if is_siege {
            // A siege that ended is a siege link that has to go, whichever
            // side won. The original clears the *besieger's* `+0x199` in the
            // A-wins branch and the *garrison's* `+0x19A` in the B-wins one —
            // the winner's own half in each case.
            w.besieging_county = 0;
            w.besieged_by = 0;
        }
        if verdict.attacker_won {
            w.besieged_by = 0;
            w.moves_used += WINNER_MOVE_COST;
            if !w.owner_is_human {
                w.moves_used = w.move_allowance - 1;
            }
        } else if !w.owner_is_human {
            w.moves_used += AI_DEFENDER_MOVE_COST;
        }
        out.winner_moves_used = w.moves_used;
    }
    if !is_siege {
        crate::siege::recompute_build_time(units, winner);
    }

    // `Army_ClearBattleSlots` (`0x004AA89F`) zeroes the four battle-only troop
    // slots on both sides. `Unit::troops` is the seven campaign counts and has
    // no room for them, so there is nothing to clear — the slots only exist
    // once sieges do.

    if loser_unit.owner != 0 && (loser_unit.owner as usize) < MAX_REALMS {
        out.offence = Some((loser_unit.owner, winner_unit.owner));
    }
    out.loser_owner = loser_unit.owner;

    // **`Army_WithdrawCasualties` (`0x004AD8CC`) — the price of leaving the
    // field, and it is charged *before* anything reads the loser's total.**
    let withdrawn_men = if withdrawal {
        Some(withdraw_casualties(t, units, realms, loser, difficulty))
    } else {
        None
    };
    out.withdrawal_casualties = withdrawn_men.map(|left| loser_unit.men - left);
    let loser_men = withdrawn_men.unwrap_or(loser_unit.men);

    let still_besieging = loser_unit.besieging_county != 0 && loser_men != 0;
    let survives = still_besieging || (withdrawal && loser_men >= WITHDRAWAL_SURVIVAL_MEN);
    if survives {
        if let Some(l) = units.get_mut(loser) {
            l.besieging_county = 0;
        }
        if let Some(g) = counties.get(county as usize).map(|c| c.garrison_unit) {
            if let Some(g) = units.get_mut(g) {
                if g.besieged_by == loser as u8 {
                    g.besieged_by = 0;
                }
            }
        }
        out.loser_siege_lifted = true;
    } else {
        crate::unit::destroy(t, units, realms, names, loser, difficulty);
        out.loser_destroyed = true;
    }
    out
}

/// **`Army_WithdrawCasualties` (`FUN_004AD8CC`, `0x004AD8CC`) — what leaving the
/// field costs.**
///
/// ```c
/// total = 0;
/// for (t = 0; t < 7; t++) {
///     n = troops[t];
///     if (n < 0xB) n = 0; else n = n / 2;
///     troops[t] = n;  total += n;
/// }
/// menTotal = total;
/// menTotal += mercMen;          /* the band is NOT halved */
/// pathLen  = 0;                 /* +0x1C  */
/// moving   = 0;                 /* +0x14C */
/// Wages_ForUnit(unit);
/// ```
///
/// > `g_battleWithdrawal` (`0x0056D5C8`) has exactly one writer —
/// > `UnitOrder_SiegeAttKnight` (`0x0048D9CE`), an AI besieger whose whole
/// > force is knights facing an unbreached wall — and three clearers, one of
/// > which is [`auto_resolve`]'s first statement. **The Retreat button is not
/// > one of them**: `FUN_0043BA29`'s confirm reaches `FUN_0043BE65`, which is
/// > the autocalc and the return,
/// > the battle and, if the ladder says he lost, watched his army destroyed
/// > See this module's `retreat` note.
pub fn withdraw_casualties(
    t: &Tables,
    units: &mut Units,
    realms: &mut [Realm; MAX_REALMS],
    id: usize,
    difficulty: u8,
) -> i32 {
    let Some(unit) = units.get_mut(id) else { return 0 };
    let mut total = 0;
    for slot in unit.troops.iter_mut() {
        *slot = if *slot < WITHDRAWAL_WIPE_BELOW { 0 } else { *slot / 2 };
        total += *slot;
    }
    unit.men = total + unit.mercenary_men();
    unit.path.clear();
    unit.moving = false;
    let (owner, left) = (unit.owner, unit.men);
    crate::unit::refresh_wages(t, units, realms, owner, difficulty);
    left
}


/// `Defence_Disband` (`0x004ABA5A`) — **the temporary defence is undone and its
/// survivors walk back into the county.**
///
/// > `[V]` **against the battle fixture triple, and it closes exactly.**
pub fn disband_defence(
    t: &Tables,
    counties: &mut [County; MAX_COUNTIES],
    realms: &mut [Realm; MAX_REALMS],
    units: &mut Units,
    names: &mut ArmyNames,
    id: usize,
    difficulty: u8,
) -> i32 {
    let Some(unit) = units.get(id) else { return 0 };
    match unit.defence_mark {
        0 => 0,
        1 => {
            let (home, men) = (unit.home_county, unit.men);
            if let Some(c) = counties.get_mut(home as usize) {
                c.population += men;
                c.army += men;
            }
            crate::unit::destroy(t, units, realms, names, id, difficulty);
            men
        }
        _ => {
            if let Some(u) = units.get_mut(id) {
                u.defence_mark = 0;
            }
            0
        }
    }
}

