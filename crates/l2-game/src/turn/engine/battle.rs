#![allow(unused_imports)]
use super::*;
use super::control::*;
use super::tick::*;
use super::*;
use super::types::*;
use super::battle::*;
use super::ai_part::*;
use super::tests::*;
use l2_kingdom::ai::{self, AiStep};
use l2_kingdom::conquest::Attack;
use l2_kingdom::phase::{Phase, PhaseWait};
use l2_kingdom::units_tick::{Contact, Encounter};
use l2_kingdom::victory::Outcome;
use l2_kingdom::{Kingdom, SeasonReport};
use crate::engagement::{self, Answer, BattleReport, SiegePhase};
use crate::game::Game;

/// File a settled battle: recount the losing realm, then onto the turn's list
/// and onto the one screen `0x13` has still to show.
///
/// **The recount is `Realm_RecountStrength` (`0x0049B42B`), the last statement
/// of both of `Battle_ReturnToCampaign`'s branches**, and it is one of only
/// four places in the original where a realm can be eliminated — the only one
/// that fires the instant a realm's last army dies
/// own turn to come round. [`crate::engagement`] cannot make the call, because
/// it needs the county count and the local player and neither is a battle rule;
/// [`l2_kingdom::battle::Aftermath::loser_owner`] carries the argument here,
/// where there is a [`Game`] to raise the *"Defeat!"* letter on and to rank
/// with.
///
/// `l2_kingdom::victory::recount_strength`'s own doc comment has listed the two
/// post-battle sites among its four callers since it was written, and nothing
/// called it from either. `docs/decisions.md` C71.
fn record(game: &mut Game, report: Option<BattleReport>) {
    let Some(report) = report else { return };
    // `County_ChangeOwner`'s letters. They are posted inside
    // `Battle_ReturnToCampaign`, before the `Realm_RecountStrength` at its
    // bottom,
    crate::arrival::post_captures(game, &report.aftermath.captures);
    game.recount_realm(report.aftermath.loser_owner);
    if let Some(p) = game.turn.as_mut() {
        p.unseen = Some(report.clone());
        p.reports.push(report);
    }
}

/// The unit sweep raised a battle: ask about it, or fight it now.
fn raise_battle(game: &mut Game, e: Encounter, interactive: bool) {
    let settlement = l2_kingdom::battle::settlement(
        &game.kingdom.campaign.units,
        e.mover,
        e.occupant,
        game.kingdom.options.fight_humans_only_byte,
    );
    if interactive && settlement == l2_kingdom::battle::Settlement::Prompt {
        let q = question_for(game, e.mover, e.occupant, None);
        let p = game.turn.as_mut().expect("raised inside a turn");
        p.question = Some(Question { county: e.county, ..q });
        return;
    }
    let answer = game.field_policy;
    settle_question(
        game,
        Question { county: e.county, ..question_for(game, e.mover, e.occupant, None) },
        answer,
    );
}

/// Read the two records into a [`Question`] while both still exist.
fn question_for(
    game: &Game,
    attacker: usize,
    defender: usize,
    castle_level: Option<u8>,
) -> Question {
    let is_siege = castle_level.is_some();
    let units = &game.kingdom.campaign.units;
    let read = |id: usize| {
        units.get(id).map_or((0u8, 0i32, 0u8, false, [0; l2_kingdom::unit::TROOP_TYPES]), |u| {
            // **`roster_of`, not `u.troops`** — `FUN_004224E7` folds the
            // mercenary band into its own row, because `Mercenary_Hire`
            // (`0x004AC7F3`) put it in `+0x168` and nowhere else.
            (u.owner, u.men, u.besieging_county, u.owner_is_human, crate::engagement::roster_of(u))
        })
    };
    let (attacker_owner, attacker_men, besieged, a_human, attacker_roster) = read(attacker);
    // The defender's `ownerIsHuman` is deliberately **not** read:
    // `Battle_ChooseSettlement` never looks at it, and reading it is what the
    // paraphrase below replaced got wrong.
    let (defender_owner, defender_men, _, _, defender_roster) = read(defender);
    let county = if is_siege { besieged } else { units.get(defender).map_or(0, |u| u.county) };
    // **`g_battleChoiceOwner`**, and this is `Battle_ChooseSettlement`
// (`0x004A6A30`), because the paraphrase was
    // wrong in the one case that mattered:
    //
    // ```c
    // g_battleChoiceOwner = 0;
    // if (units[armyA].owner == localPlayer) g_battleChoiceOwner = 1;
    // if (units[armyB].owner == localPlayer)
    //     g_battleChoiceOwner = units[armyA].ownerIsHuman ? 2 : 1;
    // ```
    //
    // Two `if`s, not an `else if` chain, and **the B arm overrides the A arm**.
    // Read it out loud: *a human defender attacked by an AI holds the choice
    // himself.* Only two humans put the choice in the other man's hands.
    //
    // > **What this cost.** The version here tested `!d_human` where the
    // > original tests `!a_human`,
    // > handed `choice_owner = 2` — the *"your opponent has the choice"* notice,
    // > which `Battle_ChooseSettlement` draws with **zero widgets**
    // > (`DAT_00554408 = 2` only under `choiceOwner == 1`). In the original a
    // > bystander's prompt waits for the multiplayer answer timeout; single
    // > player has no such timeout, so ours waited for ever with the turn
    // > suspended and both armies standing on one tile. **A player besieged by
    // > an AI could not end his turn.** It was unreachable until phase 2 could
    // > raise a siege prompt, which is the shape `docs/agents.md` C27 keeps
    // > describing: a defect nobody could get to is a defect nobody finds.
    // > `docs/decisions.md` `C80`.
    //
    // It is also the gate on every garrison verb there is: the defender's
    // drawbridge button lives on the battlefield, and until this was right no
    // besieged human could reach the battlefield at all.
    let me = game.player;
    let mut choice_owner = 0u8;
    if me == attacker_owner {
        choice_owner = 1;
    }
    if me == defender_owner {
        choice_owner = if a_human { 2 } else { 1 };
    }
    Question {
        attacker,
        defender,
        county,
        is_siege,
        attacker_owner,
        defender_owner,
        attacker_men,
        defender_men,
        attacker_roster,
        defender_roster,
        choice_owner,
        castle_level,
    }
}

/// Settle the question on the table, whichever kind it is, and clear it.
fn settle_question(game: &mut Game, q: Question, answer: Answer) {
    let Some(p) = game.turn.as_mut() else { return };
    p.question = None;
    let staged = p.pending_assault.take();
    if let Some(assault) = staged {
        let Some(mut phase) = p.siege.take() else { return };
        let seed = siege_seed(&game.kingdom);
        let report = phase.settle(&mut game.kingdom, assault, answer, seed);
        let p = game.turn.as_mut().expect("settling inside a turn");
        p.siege = Some(phase);
        record(game, report);
        return;
    }
    let attack = Attack::Battle { attacker: q.attacker, defender: q.defender };
    let seed = battle_seed(
        &game.kingdom,
        Encounter { mover: q.attacker, occupant: q.defender, county: q.county },
    );
    match engagement::resolve(&mut game.kingdom, attack, q.county, answer, seed) {
        Some(report) => record(game, Some(report)),
        None => {
// Not resolvable — a slot is no longer a unit. Reported.
            // swallowed,
            if let Some(p) = game.turn.as_mut() {
                p.pending_battles.push(Encounter {
                    mover: q.attacker,
                    occupant: q.defender,
                    county: q.county,
                });
            }
        }
    }
}

