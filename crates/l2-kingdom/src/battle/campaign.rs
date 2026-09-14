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
/// The two branches are **not** mirror images and the differences are the
/// content of the function:
///
/// | | attacker wins | defender wins |
/// |---|---|---|
/// | county changes hands | yes, if the defender was a garrison **or** carried a defence mark | **never** |
/// | county's garrison link cleared | if the defender was a garrison | if the *attacker* was a garrison |
/// | winner's moves | `+8`, then an **AI** is set to `allowance − 1` | an **AI** pays `+7`; a human pays nothing |
/// | loser | destroyed | destroyed |
/// | diplomacy | −20 from the loser's realm | −20 from the loser's realm |
///
/// **`unit.owner_is_human == false` is the AI**, which `docs/armies.md` §7 had
/// the other way round before it was corrected; the moves column above is the
/// shape that correction produces.
///
/// The county cannot change hands to a defender because `g_battleArmyB` is the
/// defender at all three of the original's call sites:
/// `County_ChangeOwner` anywhere in the B-wins branch. A defender that wins
/// keeps a county it already had — or, for a neutral county's levy, keeps it
/// neutral.
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
/// >
/// > The loser branch is two nested tests, not one:
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
///
/// # The Readme calls the withdrawal a *retreat*
///
/// The shipped `Readme.txt`'s *Retreats (pg82)*: *"Armies that retreat will
/// suffer some casualties. Any army that would have less than 50 men after
/// retreating is eliminated instead."* That is the inner branch in English, and
/// it is the game's own documentation of a rule the code reaches from **one**
/// place — `UnitOrder_SiegeAttKnight`, an all-knight AI besieger giving up on
/// an unbreached wall. The errata describe the rule as general and the shipped
/// binary makes it specific; where they differ the code is what shipped, and
/// `withdrawal` is a parameter here
/// reach the branch honestly.
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
        // The defender was the castle's garrison: the castle is no longer held,
        // and the county goes with it.
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
        // …or it was raised to defend the county, which is the same conclusion
        // by the other route. The original really does call `County_ChangeOwner`
        // twice when both hold. The second is **not** a no-op: the owner is
        // already the winner, so nothing changes hands, but the penalty is
        // taken again, the recount-plus-one counts the county twice into the
        // peak, and a second letter is posted.
        if loser_unit.defence_mark != 0 {
            out.captures[1] = Some(crate::conquest::change_owner(
                t, counties, realms, units, winner_unit.owner, county, difficulty, map, explored,
                restore,
            ));
            out.county_taken_by = Some(winner_unit.owner);
        }
    } else if loser_unit.garrison_county != 0 {
        // A garrison that sallied out and lost stops being the castle's
        // garrison — but the county does **not** change hands, because the
        // winner is the side that already held it.
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
                // A winning AI is left with exactly one move, whatever it spent
                // getting here. A winning human keeps everything but the eight.
                w.moves_used = w.move_allowance - 1;
            }
        } else if !w.owner_is_human {
            w.moves_used += AI_DEFENDER_MOVE_COST;
        }
        out.winner_moves_used = w.moves_used;
    }
    // `if (!isSiege) Siege_RecomputeBuildTime(winner)`. The winner lost men, so
    // a siege it is *itself* laying somewhere else now needs a different number
    // of seasons. On the siege path the link has just been cleared instead.
    if !is_siege {
        crate::siege::recompute_build_time(units, winner);
    }

    // `Army_ClearBattleSlots` (`0x004AA89F`) zeroes the four battle-only troop
    // slots on both sides. `Unit::troops` is the seven campaign counts and has
    // no room for them, so there is nothing to clear — the slots only exist
    // once sieges do.

    // `Diplo_Offend(loser.owner, winner.owner, 20)`. The original guards on
    // `loser.owner != 0`, which an **ownerless militia's 6 passes** — it then
    // indexes a five-realm table with 6. We refuse instead of reproducing an
    // out-of-bounds write: a county's own people have no realm to hold a
    // grudge with.
    if loser_unit.owner != 0 && (loser_unit.owner as usize) < MAX_REALMS {
        out.offence = Some((loser_unit.owner, winner_unit.owner));
    }
    out.loser_owner = loser_unit.owner;

    // **`Army_WithdrawCasualties` (`0x004AD8CC`) — the price of leaving the
    // field, and it is charged *before* anything reads the loser's total.**
    //
    // `if (g_battleWithdrawal == 1) Army_WithdrawCasualties(loser);` sits above
    // the whole loser branch in both arms of the original. Half of every troop
    // count, and a count under eleven is wiped outright; the total is rebuilt
    // by summing, so the two tests below read the *withdrawn* army and not the
    // one that walked on.
    let withdrawn_men = if withdrawal {
        Some(withdraw_casualties(t, units, realms, loser, difficulty))
    } else {
        None
    };
    out.withdrawal_casualties = withdrawn_men.map(|left| loser_unit.men - left);
    let loser_men = withdrawn_men.unwrap_or(loser_unit.men);

    // **The loser branch, both rules.** See the correction above the function.
    let still_besieging = loser_unit.besieging_county != 0 && loser_men != 0;
    let survives = still_besieging || (withdrawal && loser_men >= WITHDRAWAL_SURVIVAL_MEN);
    if survives {
        if let Some(l) = units.get_mut(loser) {
            l.besieging_county = 0;
        }
        // The garrison's half of the link goes with it, or the pair is left
        // half-connected for `Siege_StartPhase` to find next turn.
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
/// Four things a reimplementation gets wrong by default, and all four are in
/// those nine lines:
///
/// * **The mercenary band does not lose a man.** It is added to the rebuilt
///   total and never scaled — unlike the autocalc, which scales it with
///   everything else. A retreating army of hirelings retreats intact.
/// * **A line under eleven is wiped, not halved.** Six knights become none; six
///   knights and six peasants become nobody at all.
/// * **The total is rebuilt by summing**, so it cannot drift from the counts
///   however the integer division falls — the same discipline
///   [`auto_resolve`] uses.
/// * **The army stops where it stands.** The path is thrown away and the move
/// state cleared,
/// That is the half of this function that is not a
///   casualty rule at all, and it is why the army is not carried on into a
///   second fight on the same turn.
///
/// Returns the men left, which is the number
/// [`WITHDRAWAL_SURVIVAL_MEN`] is tested against.
///
/// > **The only thing in `Lords2.exe` that reaches this is a siege.**
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

// ------------------------------------------------------ §4 the levy goes home

/// `Defence_Disband` (`0x004ABA5A`) — **the temporary defence is undone and its
/// survivors walk back into the county.**
///
/// ```c
/// if (unit.defenceMark == 0) return;
/// if (unit.defenceMark < 2) {
///     county[unit.homeCounty].population += unit.menTotal;
///     county[unit.homeCounty].popArmy    += unit.menTotal;
///     Army_Destroy(unit);
/// } else {
///     unit.defenceMark = 0;
/// }
/// ```
///
/// **Every survivor, not a fraction**, and into the population *and* the
/// panel's *"Army"* line, Mark 2 — an army
/// that already existed and was pressed into defending — is not disbanded at
/// all; it only loses the mark.
///
/// This runs **after** [`return_to_campaign`], which is what makes it correct
/// in both directions: a defence that lost has already been destroyed and this
/// finds nothing, and a defence that won is still standing with its survivors
/// in [`Unit::men`](crate::unit::Unit::men).
///
/// Returns the men returned to the county.
///
/// > `[V]` **against the battle fixture triple, and it closes exactly.**
/// > County 3 holds 728 people in `battle-before.sav`; a 25 % militia of 182 is
/// > levied and it holds 546 in `battle-during.sav`; the defence wins with 36
/// > men and it holds 582 in `battle-after.sav`. `546 + 36 = 582`.
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

