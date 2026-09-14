#![allow(unused_imports)]
use super::*;
use super::resolution::*;
use super::fight_session::*;
use super::*;
use super::report::*;
use super::siege::*;
use super::tests::*;
use l2_kingdom::battle::{self, Aftermath, Settlement, Verdict};
use l2_kingdom::conquest::Attack;
use l2_kingdom::kingdom::Kingdom;
use l2_kingdom::unit::TROOP_TYPES;
use l2_sim::runner::{blank_field, BattleRunner, Muster};
use l2_sim::{End, Troop, SIDE_A, SIDE_B};

#[allow(clippy::too_many_arguments)]
pub(crate) fn resolve_battle(
    kingdom: &mut Kingdom,
    attacker: usize,
    defender: usize,
    county: u8,
    castle_level: Option<u8>,
    answer: Answer,
    seed: u64,
    fought: Option<BattleRunner>,
) -> Option<BattleReport> {
    let before = |k: &Kingdom, id: usize| k.campaign.units.get(id).map_or(0, |u| u.men);
    let (attacker_before, defender_before) = (before(kingdom, attacker), before(kingdom, defender));
    // Read now, not later: `return_to_campaign` destroys the loser's record, so
    // after it.
    let owner_of = |k: &Kingdom, id: usize| k.campaign.units.get(id).map_or(0, |u| u.owner);
    let (attacker_owner, defender_owner) = (owner_of(kingdom, attacker), owner_of(kingdom, defender));
    let roster = |k: &Kingdom, id: usize| k.campaign.units.get(id).map_or([0; TROOP_TYPES], roster_of);
    let (a_before, d_before) = (roster(kingdom, attacker), roster(kingdom, defender));

    let settlement = battle::settlement(
        &kingdom.campaign.units,
        attacker,
        defender,
        kingdom.options.fight_humans_only_byte,
    );
    let take_the_field = settlement == Settlement::Prompt && answer == Answer::TakeTheField;

    // **The repair bill, read before the runner is consumed.**
    // `Siege_RecordCastleDamage` (`0x004784CA`) is billed from the battle's own
    // accumulators, and [`conclude_fight`] takes the runner by value, so the six
    // numbers are lifted off it here.
    let mut castle_damage = fought.as_ref().map(|r| r.castle_damage()).unwrap_or_default();

    let (verdict, resolution) = if let Some(runner) = fought {
        // **The player watched it.** Everything after this point is the same
        // code the headless path runs; only the ticks came from somewhere else.
        conclude_fight(kingdom, attacker, defender, runner)
    } else if take_the_field {
        let (runner, verdict, resolution) =
            fight(kingdom, attacker, defender, castle_level, seed)?;
        castle_damage = runner;
        (verdict, resolution)
    } else {
        let verdict =
            battle::auto_resolve(&mut kingdom.campaign.units, attacker, defender, castle_level)?;
        (verdict, Resolution::Autocalc)
    };

    let attacker_after = before(kingdom, attacker);
    let defender_after = before(kingdom, defender);
    // Read **before** `return_to_campaign` empties the loser's record — after
    // it, the losing roster is seven zeros however the battle went, and the
    // screen would be unable to tell a wiped army from a missing one.
    let (a_after, d_after) = (roster(kingdom, attacker), roster(kingdom, defender));

    // `g_battleWithdrawal` is raised in exactly one place in the original —
    // `UnitOrder_SiegeAttKnight` — and `l2-sim` reports it the same way, as the
    // cause of the conclusion. Nothing else can set it, which is the whole
    // point of C31.
    let withdrawal = matches!(resolution, Resolution::Fought { cause: End::Withdrawal, .. });

    // **Bill the repair, before the county can change hands.**
    // `Siege_RecordCastleDamage` (`0x004784CA`) has exactly **one** caller and
    // it is not `Battle_ReturnToCampaign`, whatever `docs/symbols.md` says —
    // it is `FUN_004782C5`, the outcome banner's frame counter, at the moment
    // the 5,000 frames run out:
    //
    // ```c
    // if (screen == '+' && ++DAT_00568470 > 5000) {
    //     if (!skirmish && !multiplayer) Siege_RecordCastleDamage();
    //     if (choiceOwner == 1 && !skirmish) Battle_WriteBackCasualties();
    //     if (!skirmish) Battle_ReturnToCampaign(1);
    // }
    // ```
    //
    // Three rules come out of that call site and each of them is visible:
    //
    // * **Only a battle somebody watched to its end bills a repair.**
    // `Battle_Decline` and the retreat/autocalc button both leave for the
    // report screen without ever reaching this counter, so a player who
    //   knocks a wall down and then presses Autocalc un-knocks it down — the
    //   same un-doing the casualty write-back suffers on that path, and for the
    //   same reason.
    // * **It runs before the write-back and before the return**, so the county
    // is billed while it still belongs to the defender and the *conqueror*
    // inherits both the wreck and the bill. That ordering
//   sits above `return_to_campaign`.
    // * **`g_multiplayer` skips it entirely**, which cannot be right and is not
    //   reproduced: a peer that billed and a peer that did not would hold
    //   different counties. `docs/netcode.md` — the original's sync is the
    //   defect being replaced, not a model.
    if let Some(level) = castle_level {
        if let Some(c) = kingdom.counties.get_mut(county as usize) {
            l2_kingdom::siege::record_castle_damage(c, level, to_scars(castle_damage));
        }
    }

    let aftermath = {
        let restore = kingdom.restore();
        let Kingdom { counties, realms, campaign, options, tables, .. } = kingdom;
        battle::return_to_campaign(
            tables,
            counties,
            realms,
            &mut campaign.units,
            &mut campaign.names,
            verdict,
            county,
            castle_level.is_some(),
            withdrawal,
            options.difficulty,
            &campaign.map,
            &mut campaign.explored,
            restore,
        )
    };

    // `FUN_0049DF48` — `Battle_ReturnToCampaign`'s (`0x004AB383`) last call,
    // after `FUN_004AD426` and `Panels_RefreshAll`, and so **before**
    // `Defence_Disband` below, which the original runs after the return. Every
    // AI realm on the map re-manages its farms when any battle ends.
    kingdom.ai_manage_farms_after_battle();

    // **`Realm_RecountStrength` (`0x0049B42B`) on the loser's realm** is the
    // last thing both of `Battle_ReturnToCampaign`'s branches do, and it is
    // *not* here: it needs the county count and the local player, and neither
    // is a battle rule. [`Aftermath::loser_owner`] carries the argument out to
    // [`crate::turn`], which has a [`crate::game::Game`] and does it — the same
    // division as the diplomatic offence, which `l2-kingdom` reports rather
    // than applies.

    // A loser that **withdrew** is still standing, and it walked off having
    // paid `Army_WithdrawCasualties`. The two `after` numbers above were read
    // before that charge, so re-read the survivor: everything else is either
    // untouched or a record that no longer exists.
    let (attacker_after, defender_after, a_after, d_after) =
        if aftermath.withdrawal_casualties.is_some() && aftermath.loser_siege_lifted {
            (
                before(kingdom, attacker),
                before(kingdom, defender),
                roster(kingdom, attacker),
                roster(kingdom, defender),
            )
        } else {
            (attacker_after, defender_after, a_after, d_after)
        };

    let Kingdom { counties, realms, campaign, options, tables, .. } = kingdom;
    // `Defence_Disband` runs **after** the return, at the end of screen `0x13`
    // and immediately after `Battle_ReturnToCampaign(0)` on the silent path. A
    // defence that lost has already been destroyed and this finds nothing; a
    // defence that won is still standing and walks home.
    let defenders_returned = battle::disband_defence(
        tables,
        counties,
        realms,
        &mut campaign.units,
        &mut campaign.names,
        defender,
        options.difficulty,
    );

    // `Diplo_Offend(loserOwner, winnerOwner, 20)` — `Battle_ReturnToCampaign`
    // (`0x004AB383`) calls it inline and `battle::return_to_campaign` reports
    // it instead, because `l2-kingdom` had no diplomacy layer when that was
    // written. It has one now, and this is where the report is spent.
    //
    // **Losing a battle is the single largest thing that moves an AI's
    // opinion**, and until this line existed nothing in a played game moved one
    // at all: forty turns of England left every AI-to-AI standing saturated at
    // +30 by `AI_Diplomacy`'s heal, with no war target anywhere on the map.
    if let Some((loser, winner)) = aftermath.offence {
        l2_kingdom::diplomacy::offend(realms, loser, winner, battle::BATTLE_OFFENCE as i8);
    }

    Some(BattleReport {
        settlement,
        resolution,
        verdict,
        aftermath,
        defenders_returned,
        attacker_men: (attacker_before, attacker_after),
        defender_men: (defender_before, defender_after),
        county,
        is_siege: castle_level.is_some(),
        attacker_owner,
        defender_owner,
        attacker_roster: (a_before, a_after),
        defender_roster: (d_before, d_after),
        castle_damage,
    })
}

/// **Raise both campaign records into `l2-sim`, run the battle, and write the
/// survivors back.**
///
/// `Battle_InitArmies` raises army A with side 4 and army B with side 0, and
/// the attacker is A at every one of the original's call sites — so the
/// *defender* is side 0, the side that deploys at the `0x04` marker.
/// [`BattleRunner::deploy_muster`] holds that convention and picks the
/// men-per-figure scale from the two totals.
///
/// The battle ends where `FUN_00477DFC` ends it — one side's men reaching
/// zero, or a withdrawal — and only then is the result written back.
/// **`FUN_0047F474` runs on the far side of the outcome banner's 5,000-tick
/// settle**, not at the moment the battle is decided; nothing changes while it
/// counts, so the settle is skipped here and
/// [`l2_sim::runner::SETTLE_TICKS`] carries the number for a caller that is
/// pacing a screen.
///
/// Writing back is that function: both records' eleven counts are zeroed and
/// rebuilt from the surviving figures, and the total follows the counts
/// than being scaled.
fn fight(
    kingdom: &mut Kingdom,
    attacker: usize,
    defender: usize,
    castle_level: Option<u8>,
    seed: u64,
) -> Option<(l2_sim::CastleDamage, Verdict, Resolution)> {
    let mut runner = begin_fight(kingdom, attacker, defender, castle_level, seed)?;
    // Nobody is watching, so nobody clicks. See the function's own header for
    // why this is here and not inside `begin_fight`.
    charge_for_the_absent_player(kingdom, &mut runner, attacker, defender);
    // The original's frame loop asks `FUN_00477DFC` every frame; asking every
    // hundredth costs at most ninety-nine ticks of a battle that is already
    // over, and no rule reads the tick count.
    while runner.tick < MAX_TICKS {
        runner.run(CHECK_EVERY);
        if runner.conclusion().is_some() {
            break;
        }
    }
    let damage = runner.castle_damage();
    let (verdict, resolution) = conclude_fight(kingdom, attacker, defender, runner);
    Some((damage, verdict, resolution))
}

/// The same, plus the four battle-only slots a siege fills. `extra` is empty
/// for a field battle, so the two paths are one function.
pub(crate) fn muster_with(
    kingdom: &Kingdom,
    id: usize,
    extra: l2_kingdom::siege::BattleEngines,
) -> Option<Vec<(Troop, u32)>> {
    let u = kingdom.campaign.units.get(id)?;
    let mut counts = [0u32; 11];
    for (slot, men) in counts.iter_mut().zip(u.troops.iter()) {
        *slot = (*men).max(0) as u32;
    }
    if let Some(band) = u.mercenaries {
        counts[band.troop.index()] += band.men().max(0) as u32;
    }
    // Troop types 7…10. **These are counts of engines, not of men**: one
    // catapult is one figure whatever the men-per-figure scale, and
    // `BattleRunner::raise_men` multiplies them back up for exactly that
    // reason.
    for (slot, count) in extra.counts().into_iter().enumerate() {
        counts[7 + slot] = count.max(0) as u32;
    }
    debug_assert_eq!(TROOP_TYPES, 7, "the campaign record carries seven of the eleven");
    Some(
        (0..11)
            .filter(|&t| counts[t] > 0)
            .map(|t| (l2_sim::ALL_TROOPS[t], counts[t]))
            .collect(),
    )
}

