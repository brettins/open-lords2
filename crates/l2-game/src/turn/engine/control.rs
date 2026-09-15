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

pub fn end_turn(game: &mut Game) -> Option<TurnOutcome> {
    match advance(game, Resume::Start, false) {
        TurnStep::Done(outcome) => Some(*outcome),
        _ => None,
    }
}

pub fn begin_turn(game: &mut Game) -> TurnStep {
    advance(game, Resume::Start, true)
}

pub fn answer_battle(game: &mut Game, answer: Answer) -> TurnStep {
    advance(game, Resume::Answer(answer), true)
}

/// **The player pressed the thumb up on screen `0x12`** — `FUN_0043B593` with
/// `g_uiHotspotId == 1`, which calls `Battle_Start` (`0x004778A0`).
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

/// * **it ended** — `Battle_CheckOutcome` (`0x00477DFC`) —
///   the simulation produced are written back;
/// * **the player retreated or autocalculated** — `FUN_0043BE65`, which runs
///   `Battle_AutoResolve` and **never calls `Battle_WriteBackCasualties`**. So
/// every man killed so far is unkilled,
///   armies as they walked on. Reproduced by dropping the runner on the floor
///   and answering [`Answer::Decline`], which is the same autocalc.
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
        p.pending_assault = None;
    }
    record(game, report);
    advance(game, Resume::Start, true)
}

pub fn dismiss_report(game: &mut Game) -> TurnStep {
    advance(game, Resume::Dismiss, true)
}

pub fn tick_turn(game: &mut Game) -> TurnStep {
    if game.turn.is_none() {
        return TurnStep::Stuck;
    }
    advance(game, Resume::Start, true)
}

/// The original's main loop is `if ((g_battlePhase == 0) && (ticksDue != 0))
/// { Turn_Tick(); Units_Tick(); }` and it runs *whenever the game is up*, not
/// only while a turn is being wound on (`docs/decisions.md` C35 quotes the call
/// site). That is what makes an army the player has just ordered walk away
/// while he watches, — the
/// second half of *"can't seem to move my army"*, and indistinguishable from
/// the order having been ignored.
///
/// `docs/decisions.md` C115.
///
/// `Battle_ChooseSettlement` (`0x004A6A30`) has no opinion about which frame an
/// army arrived on. `Units_Tick` is called from the frame loop next to
/// `Turn_Tick` (`docs/decisions.md` C35), `Unit_EnterOccupiedTile` and
/// `Army_AttackCounty` call the gate from inside it,
/// `g_screenId = 0x12` on the spot — the campaign then stands still because
/// `Units_Tick`'s own latch abandons the sweep.
pub fn tick_units_only(game: &mut Game) -> usize {
    let moved = game.sweep_units();
    game.tips.note_incursions(&moved.incursions, game.player);
    crate::arrival::post(game, &moved.posted);
    let stepped = moved.stepped;
    if let Some(e) = moved.battle() {
        raise_idle_battle(game, e);
    }
    ask_combine(game, &moved.contacts);
    stepped
}

fn advance(game: &mut Game, resume: Resume, interactive: bool) -> TurnStep {
    if game.turn.is_none() {
        for (id, realm) in game.kingdom.realms.iter().enumerate() {
            game.gold_last[id] = realm.gold;
        }
        game.turn = Some(TurnProgress::default());
    }
    let mut input = resume;

    loop {
        if game.turn.as_ref().is_some_and(|p| p.unseen.is_some()) {
            if !interactive || input == Resume::Dismiss {
                game.turn.as_mut().expect("checked").unseen = None;
                input = Resume::Start;
                continue;
            }
            let r = game.turn.as_ref().and_then(|p| p.unseen.clone()).expect("checked");
            return TurnStep::Report(Box::new(r));
        }

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
                    game.turn = None;
                    return TurnStep::Stuck;
                }
                p.ticks += 1;
                p.stage = Stage::Phase;
                let phase = game.kingdom.turn.phase;
                if game.kingdom.turn.step == 0 {
                    begin_phase(game, phase);
                }
            }
            Stage::Phase => {
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
                if interactive
                    && game.turn.as_ref().is_some_and(|p| p.stage == Stage::Begin)
                {
                    return TurnStep::Running;
                }
            }
        }
    }
}

fn finish_tick(game: &mut Game, interactive: bool) -> Option<TurnOutcome> {
    let Some(mut tail) = game.turn.as_mut().and_then(|p| p.tail.take()) else {
        if let Some(p) = game.turn.as_mut() {
            p.stage = Stage::Begin;
        }
        return None;
    };
    if let Some(e) = tail.battle.take() {
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
    crate::message::rearm_events(game, &report);
    game.last_report = Some(report.clone());
    game.rank_realms();
    let outcome = if interactive {
        game.campaign.outcome
    } else {
        crate::message::drain(game)
    };
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

