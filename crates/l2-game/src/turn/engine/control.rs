#![allow(unused_imports)]
use super::*;
use super::tick::*;
use super::battle::*;
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

/// Run the phase machine until the end-of-season pipeline has run once.
///
/// Returns `None` only if [`MAX_TICKS`] is reached, which would be a bug in a
/// wait condition.
///
/// # The shape of one tick
///
/// The original's frame loop is `Turn_Tick(); Units_Tick();` — two calls, in
/// that order, and this is them. `Turn_Tick` runs the phase's first-step work,
/// evaluates the wait and may advance; `Units_Tick` then steps every unit that
/// is walking, whatever phase that left current. Doing it the other way round
/// would let a unit ordered by a phase take its first step before the phase had
/// finished ordering the rest of them, which is a lockstep difference and not a
/// cosmetic one.
pub fn end_turn(game: &mut Game) -> Option<TurnOutcome> {
    // The non-interactive path: nothing may stop to ask, so every prompt is
    // answered by the policy and the machine cannot come back holding a
    // question. `Ask` reaching here would be a caller using the wrong door, and
    // so would a suspended idle battle — [`turn_in_flight`] is how a caller asks
    // whether either is outstanding, and both come back `None` here.
    match advance(game, Resume::Start, false) {
        TurnStep::Done(outcome) => Some(*outcome),
        _ => None,
    }
}

/// **Start a turn that is allowed to stop and ask.**
///
/// The interactive door. [`end_turn`] is the other one and answers every
/// prompt with the game's standing policy instead; the two run the same code
/// and differ only in whether a [`Settlement::Prompt`](l2_kingdom::battle::Settlement::Prompt)
/// battle is allowed to suspend the machine.
///
/// A suspended turn lives on the [`Game`] until it is answered. Only one can be
/// in flight, because there is only one campaign.
pub fn begin_turn(game: &mut Game) -> TurnStep {
    advance(game, Resume::Start, true)
}

/// **Answer the question on the table and carry the turn on.** The player has
/// clicked the thumb on screen `0x12`.
///
/// The battle is fought or calculated inside this call, so what comes back is
/// normally [`TurnStep::Report`].
pub fn answer_battle(game: &mut Game, answer: Answer) -> TurnStep {
    advance(game, Resume::Answer(answer), true)
}

/// **The player pressed the thumb up on screen `0x12`** — `FUN_0043B593` with
/// `g_uiHotspotId == 1`, which calls `Battle_Start` (`0x004778A0`).
///
/// Raises the battlefield instead of settling the question, and leaves the turn
/// suspended exactly where it was: the two armies are still standing on one tile
/// and neither record has been touched. [`finish_battle`] is the other end.
///
/// Answers `false` when
/// mustered, and the caller should then fall back to [`answer_battle`], which
/// settles it the way a headless turn would.
pub fn take_the_field(game: &mut Game) -> bool {
    let Some(q) = pending_question(game) else { return false };
    let seed = if q.is_siege {
        siege_seed(&game.kingdom)
    } else {
        battle_seed(
            &game.kingdom,
            Encounter { mover: q.attacker, occupant: q.defender, county: q.county },
        )
    };
    let Some(runner) =
        engagement::begin_fight(&mut game.kingdom, q.attacker, q.defender, q.castle_level, seed)
    else {
        return false;
    };
    game.battle = Some(Box::new(crate::battlefield::LiveBattle::new(
        runner,
        q.attacker,
        q.defender,
        q.county,
        q.castle_level,
        game.player,
        q.choice_owner,
    )));
    true
}

/// **The battle the player was watching is over.**
///
/// Takes the live battle off [`Game`] and settles the suspended question with
/// it, which produces the [`TurnStep::Report`] screen `0x13` draws. The two ways
/// out of a battle land here differently,
///
/// * **it ended** — `Battle_CheckOutcome` (`0x00477DFC`) —
///   the simulation produced are written back;
/// * **the player retreated or autocalculated** — `FUN_0043BE65`, which runs
///   `Battle_AutoResolve` and **never calls `Battle_WriteBackCasualties`**. So
/// every man killed so far is unkilled,
///   armies as they walked on. Reproduced by dropping the runner on the floor
///   and answering [`Answer::Decline`], which is the same autocalc.
///
/// # The discard is deliberate, it is single-player-only.
///
/// Re-read at the instruction level, because this is the
/// line that reads like the seam being broken. `Battle_WriteBackCasualties`
/// (`0x0047F474`) has **five call sites in the whole binary**: two in
/// `Battle_CheckOutcome`'s post-banner arm, two in `FUN_004782C5`'s, and one in
/// `FUN_0043BDCD` — and that last one is inside `if (g_multiplayer != 0)`. The
/// single-player arm of the same `if` calls `FUN_0043BE65` and writes nothing
/// back. So **the same Autocalc button keeps the casualties in a network game
/// and throws them away in a solo one**; the Retreat confirms have no such arm
/// at all and discard in both. `docs/bugs.md` B76.
///
/// It is worth stating what this does *not* mean,
/// and wrong. A battle fought to its conclusion writes back — that is
/// [`engagement::conclude_fight`], asserted by three tests that go red if the
/// write-back is ablated. A battle between two AI realms never reaches the
/// simulation at all and is settled by the autocalc, which writes the survivors
/// into the campaign records as its whole purpose. **Only the early exit
/// discards**, and only here. `docs/decisions.md` C71.
///
/// And note what `Battle_AutoResolve` does *first*: it clears
/// `g_battleWithdrawal`. Pressing Retreat therefore does not perform a retreat
/// — it auto-resolves,
///
/// rules are reachable only from `UnitOrder_SiegeAttKnight`.
pub fn finish_battle(game: &mut Game) -> TurnStep {
    let Some(live) = game.battle.take() else { return TurnStep::Stuck };
    if live.autocalc {
        return answer_battle(game, Answer::Decline);
    }
    let seed = 0;
    let report = engagement::resolve_fought(
        &mut game.kingdom,
        live.attacker,
        live.defender,
        live.county,
        live.castle_level,
        seed,
        live.runner,
    );
    let Some(p) = game.turn.as_mut() else { return TurnStep::Stuck };
    p.question = None;
    let staged = p.pending_assault.take();
    if staged.is_some() {
        // The siege pump parked its assault on the question; the battle has now
// been fought, so the pump must be let go of it
        // a second time.
        p.pending_assault = None;
    }
    record(game, report);
    advance(game, Resume::Start, true)
}

/// **The player has finished looking at screen `0x13`.** Carries the suspended
/// turn on to whatever is next — another battle, or the end of the turn.
pub fn dismiss_report(game: &mut Game) -> TurnStep {
    advance(game, Resume::Dismiss, true)
}

/// **One frame of a turn in flight.** The screen's per-tick door.
///
/// Runs exactly one `Turn_Tick` / `Units_Tick` pair and comes back, so the map
/// is drawn between every pair of them and a unit that is walking is *seen* to
/// walk. See [`TurnStep::Running`] for why that is the whole fix for a
/// teleporting merchant.
///
/// Answers [`TurnStep::Stuck`] if no turn is in flight, because starting one
/// from here would turn a stray tick into a played turn.
pub fn tick_turn(game: &mut Game) -> TurnStep {
    if game.turn.is_none() {
        return TurnStep::Stuck;
    }
    advance(game, Resume::Start, true)
}

/// **`Units_Tick` on a frame that is not part of a turn.**
///
/// The original's main loop is `if ((g_battlePhase == 0) && (ticksDue != 0))
/// { Turn_Tick(); Units_Tick(); }` and it runs *whenever the game is up*, not
/// only while a turn is being wound on (`docs/decisions.md` C35 quotes the call
/// site). That is what makes an army the player has just ordered walk away
/// while he watches, — the
/// second half of *"can't seem to move my army"*, and indistinguishable from
/// the order having been ignored.
///
/// A unit walks at most `moveAllowance - movesUsed` tiles and then stops,
/// player who sits on the map gets no extra movement out of it;
/// `Pass::UnitsResetMoves` at the end of the season is what starts it again.
///
/// # A battle raised here goes through the same gate, and this is the bug that
/// made it
///
/// This used to read: *"A battle raised here is settled by the standing policy
///
/// suspend, and a screen `0x12` raised from an idle frame would have nothing to
/// carry on afterwards."*
///
/// **Every word of that was true and the conclusion was wrong.** A player
/// reported it as *"it was me attacking a town and it just immediately
/// resolved."* He had ordered the march and then watched it, which is this
/// door — so his battle met `game.field_policy`, [`Answer::Decline`], the
/// autocalc, and no screen was raised at all. The three seam tests that cover
/// the prompt all press End Turn in the same breath as the order, so the army
/// only ever arrived inside the turn machine and none of them could see it.
/// `docs/decisions.md` C115.
///
/// `Battle_ChooseSettlement` (`0x004A6A30`) has no opinion about which frame an
/// army arrived on. `Units_Tick` is called from the frame loop next to
/// `Turn_Tick` (`docs/decisions.md` C35), `Unit_EnterOccupiedTile` and
/// `Army_AttackCounty` call the gate from inside it,
/// `g_screenId = 0x12` on the spot — the campaign then stands still because
/// `Units_Tick`'s own latch abandons the sweep.
/// flight. So the three settlements are answered here
/// inside a turn,
///
/// Returns how many tiles were entered,
/// frame needs repainting.
pub fn tick_units_only(game: &mut Game) -> usize {
    let moved = game.sweep_units();
    game.tips.note_incursions(&moved.incursions, game.player);
    // `Unit_EnterCounty`'s and `County_ChangeOwner`'s letters, onto this peer's
    // ring. See `crate::arrival`.
    crate::arrival::post(game, &moved.posted);
    let stepped = moved.stepped;
    if let Some(e) = moved.battle() {
        raise_idle_battle(game, e);
    }
    ask_combine(game, &moved.contacts);
    stepped
}

/// The whole turn loop, in both its interactive and its headless form.
///
/// `resume` is what the caller is handing back — nothing, an answer, or "I have
/// seen the result". `interactive` decides whether the loop may stop at all:
/// without it every question is answered by
/// [`Game::field_policy`](crate::game::Game::field_policy), no report is shown,
/// and the loop runs to the end.
fn advance(game: &mut Game, resume: Resume, interactive: bool) -> TurnStep {
    if game.turn.is_none() {
        for (id, realm) in game.kingdom.realms.iter().enumerate() {
            game.gold_last[id] = realm.gold;
        }
        game.turn = Some(TurnProgress::default());
    }
    let mut input = resume;

    loop {
        // **An unshown result outranks everything**, including the next battle:
        // the original does not move the campaign on while `0x13` is up.
        if game.turn.as_ref().is_some_and(|p| p.unseen.is_some()) {
            if !interactive || input == Resume::Dismiss {
                game.turn.as_mut().expect("checked").unseen = None;
                input = Resume::Start;
                continue;
            }
            let r = game.turn.as_ref().and_then(|p| p.unseen.clone()).expect("checked");
            return TurnStep::Report(Box::new(r));
        }

        // **A question outranks the rest.** Nothing else in the turn may run
        // while two armies are standing on the same tile unresolved.
        if let Some(q) = game.turn.as_ref().and_then(|p| p.question) {
            let a = match input {
                Resume::Answer(a) => {
                    input = Resume::Start;
                    a
                }
                _ if interactive => return TurnStep::Ask(q),
                _ => game.field_policy,
            };
            settle_question(game, q, a);
            continue;
        }

// **An idle-frame battle is over**
        // it.** Falling through to the stage machine here is what would turn
        // *"he answered the prompt"* into *"the season advanced"*. See
        // [`TurnProgress::idle`].
        if game.turn.as_ref().is_some_and(|p| p.idle) {
            game.turn = None;
            return TurnStep::Running;
        }

        let stage = match game.turn.as_ref() {
            Some(p) => p.stage,
            None => return TurnStep::Stuck,
        };
        match stage {
            Stage::Begin => {
                let p = game.turn.as_mut().expect("checked above");
                if p.ticks >= MAX_TICKS {
// Give the half-turn back:
                    // a stuck turn that could not be re-entered would wedge the
                    // campaign for the rest of the session.
                    game.turn = None;
                    return TurnStep::Stuck;
                }
                p.ticks += 1;
                p.stage = Stage::Phase;
                let phase = game.kingdom.turn.phase;
                // `step == 0` is the machine's own "this is the first call of
                // this phase", which is when the original's phase handlers kick
                // off work.
                if game.kingdom.turn.step == 0 {
                    begin_phase(game, phase);
                }
            }
            Stage::Phase => {
                // Phase 2's pump, one assault at a time. `begin_phase` loaded
// it and left it here, which is what
                // makes an assault askable.
                if game.turn.as_ref().is_some_and(|p| p.siege.is_some()) {
                    pump_siege(game);
                    continue;
                }
                run_phase_tick(game);
            }
            Stage::Tail => {
                if let Some(outcome) = finish_tick(game, interactive) {
                    return TurnStep::Done(Box::new(outcome));
                }
                // **The end of a whole tick, and where an interactive caller
                // gets its frame back.** `Stage::Begin` is the only stage that
                // means "a tick finished": `finish_tick` returns without
                // setting it when a battle interrupted the sweep, and that is
                // not a frame boundary — the two armies are standing on one
                // tile with the fight unresolved.
                //
                // A turn therefore takes as many frames as it takes ticks,
                // which is the whole of `TurnStep::Running`. See there.
                if interactive
                    && game.turn.as_ref().is_some_and(|p| p.stage == Stage::Begin)
                {
                    return TurnStep::Running;
                }
            }
        }
    }
}

/// The rest of the tick: the battle, the contacts, and — on the last tick of
/// the turn — the season report.
///
/// The order is the original's and is not cosmetic: the battle is fought before
/// the contacts are recorded and before the report is delivered,
/// whose last army dies this tick is eliminated by [`Game::rank_realms`] on the
/// same turn.
fn finish_tick(game: &mut Game, interactive: bool) -> Option<TurnOutcome> {
    let Some(mut tail) = game.turn.as_mut().and_then(|p| p.tail.take()) else {
        if let Some(p) = game.turn.as_mut() {
            p.stage = Stage::Begin;
        }
        return None;
    };
    if let Some(e) = tail.battle.take() {
        // Put the rest of the tick back before anything can suspend: the
        // question is answered at the top of the loop and lands here again.
        if let Some(p) = game.turn.as_mut() {
            p.tail = Some(tail);
        }
        raise_battle(game, e, interactive);
        return None;
    }
    let p = game.turn.as_mut()?;
    p.contacts.extend(tail.contacts.iter().copied());
    p.stage = Stage::Begin;

    let report = tail.report?;
    game.turns_played += 1;
    // `Event_RollAll` raised latches this season; this peer's posting marks
    // come down for them. See [`crate::message::rearm_events`].
    crate::message::rearm_events(game, &report);
    game.last_report = Some(report.clone());
    // `Turn_Tick`'s phase 7 calls `Score_RankRealms` after `Season_Advance` —
    // one of its five callers,
    // of a season in which nobody died. `Pass::ScoreRank` inside the pipeline
    // has already ranked; this adds the leader/trailer scan that the pass
    // deliberately does not own, because the pass does not know who the local
    // player is.
    game.rank_realms();
    // **The headless door shows its own messages.** `crate::message::drain` is
    // `Msg_Pump` + the window's first-frame arms + `Msg_Dismiss`, run to
    // exhaustion with no window and no click, and it is the *same* ladder the
    // message screen walks one click at a time. `end_turn` cannot raise a
    // screen, — and ends the same way.
    //
    // `turn::begin_turn`, the interactive door, does **not** call this: there
    // the map screen is up, `Machine::update` pumps,
    // corner button. See `docs/arms.json`, group `messages`.
    let outcome = if interactive {
        game.campaign.outcome
    } else {
        crate::message::drain(game)
    };
    // **The turn is over, so the progress goes.** Every scrap of it moves into
    // the outcome — nothing is left on the `Game`, because a `Game` carrying a
    // finished turn's leavings is a `Game` that no longer round-trips through a
    // save, and `tests/save.rs` compares the whole struct.
    let p = game.turn.take()?;
    Some(TurnOutcome {
        report,
        ticks: p.ticks,
        outcome,
        steps: p.steps,
        contacts: p.contacts,
        pending_battles: p.pending_battles,
        battles: p.reports,
    })
}

