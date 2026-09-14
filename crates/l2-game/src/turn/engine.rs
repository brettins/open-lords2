#![allow(unused_imports)]
use super::*;
use super::types::*;
use super::battle::*;
use super::ai::*;
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

/// **The halt where the original walks on** — [`Contact::Blocked`] with the
/// player's own army standing on the tile.
///
/// `Unit_EnterOccupiedTile`'s rung 3 is *"same owner — merge, but only on an
/// explicit merge order"*,
/// army walks over its own. Ours stops (`Contact::Blocked`'s own note). The
/// question it stops on is the original's: `Map_ConfirmMoveOrder`
/// (`0x004A9252`) raises `L2.eng` 10/5 *"Combine armies?"* off
/// `g_hoverMergeUnit` and `MoveOrder_ConfirmCombine` (`0x004A975D`) merges into
/// the standing unit. **When** we ask is ours — [`Game::combine_ask`].
///
/// Only the local player's own two armies: an AI's pair merges with no prompt
/// (`Contact::Merged`), and nobody may be asked about another realm's units.
fn ask_combine(game: &mut Game, contacts: &[Contact]) {
    if game.combine_ask.is_some() {
        return;
    }
    let units = &game.kingdom.campaign.units;
    game.combine_ask = contacts.iter().find_map(|c| {
        let Contact::Blocked { mover, occupant } = *c else { return None };
        let (m, o) = (units.get(mover)?, units.get(occupant)?);
        let mine = |u: &l2_kingdom::Unit| {
            u.owner == game.player && u.kind == l2_kingdom::unit::UnitKind::Army
        };
        (mine(m) && mine(o)).then_some((mover, occupant))
    });
}

/// `Battle_ChooseSettlement` for a battle raised outside a turn. See
/// [`tick_units_only`].
fn raise_idle_battle(game: &mut Game, e: Encounter) {
    use l2_kingdom::battle::Settlement;
    let settlement = l2_kingdom::battle::settlement(
        &game.kingdom.campaign.units,
        e.mover,
        e.occupant,
        game.kingdom.options.fight_humans_only_byte,
    );
    if settlement == Settlement::Silently || game.turn.is_some() {
        // **Nobody's but the lords'**,
        // screen, no report, the autocalc and on with the frame. Through
        // [`record`] like every other battle, so the losing realm is recounted
        // here too —
        // itself has nowhere to go and `record` drops it,
        // the half that must not be dropped with it.
        //
// The `game.turn.is_some()` half is a guard: the map
        // screen does not run this sweep while a turn is in flight, and
        // clobbering a suspended turn with an idle one would lose a season.
        let attack = Attack::Battle { attacker: e.mover, defender: e.occupant };
        let seed = battle_seed(&game.kingdom, e);
        let answer = game.field_policy;
        let report = engagement::resolve(&mut game.kingdom, attack, e.county, answer, seed);
        record(game, report);
        return;
    }
    // A person is in it. Suspend the campaign the way a turn's battle does, and
    // mark the suspension as *not a turn* so that answering it winds nothing on.
    game.turn = Some(TurnProgress { idle: true, ..TurnProgress::default() });
    let q = Question { county: e.county, ..question_for(game, e.mover, e.occupant, None) };
    if settlement == Settlement::Prompt {
        game.turn.as_mut().expect("just installed").question = Some(q);
        return;
    }
    // `Settlement::Reported` — *Fight humans only?* is on and the other side is
    // the AI's. The autocalc runs and the player is **told** on screen `0x13`
// The report used to be dropped here.
    let answer = game.field_policy;
    settle_question(game, q, answer);
}

/// **The sortie** — `FUN_00437535`'s tail, the body of `Army_LeaveCastle`
/// (`0x004374C4`):
///
/// ```c
/// if (unit.besiegedBy && Battle_BeginFromCampaign(unit, unit.besiegedBy))
///     g_battleCounty = county;
/// ```
///
/// `Battle_BeginFromCampaign` (`0x004A7158`) takes the marching garrison as
/// `g_battleArmyA` and the besieger as `g_battleArmyB`; the caller then
/// **overwrites `g_battleCounty`** with the castle's county.
/// county here is the one left and not the occupant's.
///
/// The button is pressed on an ordinary frame, so this is
/// [`raise_idle_battle`]'s gate — `Battle_ChooseSettlement` either way.
pub(crate) fn raise_sortie(game: &mut Game, garrison: usize, besieger: usize, county: u8) {
    raise_idle_battle(game, Encounter { mover: garrison, occupant: besieger, county });
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

/// The phase's own tick and the unit sweep that follows it, up to the point
/// where a battle may interrupt.
fn run_phase_tick(game: &mut Game) {
    let phase = game.kingdom.turn.phase;
    if phase == Phase::PlayersTurn {
        let mut granted = game.turn.as_ref().is_some_and(|p| p.granted);
        drive_ai(&mut game.kingdom, &mut granted);
        if let Some(p) = game.turn.as_mut() {
            p.granted = granted;
        }
    }
    let (_, report) = game.kingdom.tick(settled(&game.kingdom, phase));

    // `Units_Tick`, immediately after `Turn_Tick` and outside the phase
    // machine entirely. See the module documentation.
    // `Game::sweep_units` also holds the frame each tick handler writes first.
    let moved = game.sweep_units();
    // `DAT_00553210`, the invasion tip's flag. See `crate::tip`.
    game.tips.note_incursions(&moved.incursions, game.player);
    // The same sweep's letters, the same door as `tick_units_only`'s.
    crate::arrival::post(game, &moved.posted);
    let Some(p) = game.turn.as_mut() else { return };
    p.steps += moved.stepped;
    p.stage = Stage::Tail;
    p.tail = Some(Tail { battle: moved.battle(), contacts: moved.contacts, report });
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

/// One assault of phase 2, asked about or settled.
fn pump_siege(game: &mut Game) {
    let Some(mut phase) = game.turn.as_mut().and_then(|p| p.siege.take()) else { return };
    let Some(assault) = phase.next(&mut game.kingdom) else {
        // The cursor ran off the end; phase 2 is done and the tick carries on.
        return;
    };
    // `Turn_Tick`'s phase-2 arm: `DAT_0055403C = 0; Siege_LaunchAssault(…)`.
    // The flag is the one the turn timer's restart waits on.
    game.turn_clock.assault_launched();
    let seed = siege_seed(&game.kingdom);
    // A refusal is not a battle and cannot be asked about: the siege has
    // already been lifted by `siege::assault` and there is nothing to fight.
    let level = match assault {
        l2_kingdom::siege::Assault::Battle { castle_level, .. } => Some(castle_level),
        _ => None,
    };
    let question = SiegePhase::settlement(&game.kingdom, assault).and_then(|(a, d, s)| {
        (s == l2_kingdom::battle::Settlement::Prompt).then(|| question_for(game, a, d, level))
    });
    match question {
        Some(q) => {
            // Park the assault on the question so the answer settles this one.
            let p = game.turn.as_mut().expect("pump runs inside a turn");
            p.siege = Some(phase);
            p.pending_assault = Some(assault);
            p.question = Some(q);
        }
        None => {
            let report = phase.settle(&mut game.kingdom, assault, game.field_policy, seed);
            let p = game.turn.as_mut().expect("pump runs inside a turn");
            p.siege = Some(phase);
            record(game, report);
        }
    }
}

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

/// Answer the current phase's wait.
///
/// Three of the four unit phases ask the unit array directly. Phase 4's wait is
/// the AI's and `Kingdom::tick` overrides whatever is passed for it.
///
/// **Phase 2 is answered `true` here because [`begin_phase`] has already run
/// the whole of it.** `Turn_Tick`'s phase-2 arm is a pump — validate, build,
/// assault, repeat until the cursor comes up empty — and
/// [`crate::engagement::run_siege_phase`] runs that pump to exhaustion in one
/// call. The two agree on every number because
/// the cursor only ever advances and no other phase runs between its steps; the
/// difference is that ours does not spread the sieges over as many `Turn_Tick`
/// calls, which nothing outside the phase can observe. See the comment on
/// [`begin_phase`]'s `ArmyMovement` arm.
fn settled(kingdom: &Kingdom, phase: Phase) -> bool {
    match phase.wait() {
        PhaseWait::Units(kind) => !kingdom.units_moving(kind),
        PhaseWait::Sieges => true,
        PhaseWait::Steps(_) | PhaseWait::AllRealmsDone | PhaseWait::Immediate => true,
    }
}

/// The seed for the siege phase's assaults, from state both peers agree on.
///
/// The same rule as [`battle_seed`]: the turn counter and nothing that is a
/// clock, an address or an iteration order. `run_siege_phase` adds the round
/// number to it,
/// same phase run twice fights the same two.
fn siege_seed(kingdom: &Kingdom) -> u64 {
    let mut z = (kingdom.turn_count as u64)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ 0x5165_6765_0000_0002;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

