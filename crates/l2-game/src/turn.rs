//! Ending a turn: driving `l2-kingdom`'s phase machine all the way round.
//!
//! `Turn_Tick` (`0x0049A010`) dispatches on `g_turnPhase` every frame and
//! `Turn_AdvancePhase` wraps 7 back to 1. `l2-kingdom` models that as
//! [`TurnMachine`](l2_kingdom::phase::TurnMachine) and deliberately stops
//! there: **three** of the seven phases wait on units moving, one waits on
//! sieges, one on the AI, and the machine answers none of them itself. The
//! caller does. This module is that caller.
//!
//! > The line above used to say *"four of the seven phases wait on units
//! > moving, and units are not that crate's state"*. Both halves have since
//! > stopped being true. Units **are** `l2-kingdom`'s state — `Campaign` lives
//! > inside `Kingdom` because two lockstep peers have to agree about where an
//! > army stands — and phase 2 waits on sieges rather than armies
//! > (`docs/decisions.md` C35). What is left on this side of the seam is not
//! > ownership of the units; it is the two things below that genuinely are not
//! > rules: what a phase's wait *means* to an application, and who fights a
//! > battle.
//!
//! # What the spine has to supply
//!
//! Three things, and none of them is in `l2-kingdom` because none of them is a
//! rule:
//!
//! 1. **The waits, and the mover they wait on.** This used to read:
//!
//!    > *"Phases 2, 3, 5 and 6 wait for armies, transports, peasant mobs and
//!    > merchants to stop moving. None of those exist yet, so they are settled
//!    > the moment they start, and `settled = true` says exactly that."*
//!
//!    They exist now. Four of the seven phases were no-ops, so nothing on the
//!    campaign map ever moved during a played turn: two units could never meet,
//!    a merchant never walked its route, an enemy army never trampled a
//!    resource site, and `disabled_seasons` never counted down. That is what
//!    this module now does, through [`l2_kingdom::units_tick`].
//!
//!    **The one thing that changed shape while doing it**: the mover is *not*
//!    inside a phase. `Units_Tick` is a sibling of `Turn_Tick` in the original's
//!    frame loop, called immediately after it and never reading `g_turnPhase`,
//!    so every unit that is walking takes a step on **every** tick of the turn.
//!    The phases only originate the game's own orders — transports in 3, mobs
//!    in 5, merchants in 6 — and then wait for them to stop. Phase 2
//!    originates nothing and waits on sieges, not armies; `docs/kingdom.md`
//!    §3.1 said otherwise and is corrected (`docs/decisions.md` C35).
//!
//! 2. **The battle.** When two enemy armies meet, `l2-kingdom` reports the pair
//!    and refuses to fight them — it cannot depend on `l2-sim` and must not.
//!    [`resolve_battle`] hands them to [`crate::engagement`], which is the only
//!    module in the workspace that depends on both. A played turn therefore
//!    produces a battle *result*: one of the two armies is gone, the county may
//!    have changed hands, and a levied defence has walked home.
//!
//! 3. **The AI's turn.** Phase 4 will not end until every realm's `aiStep`
//!    reaches its threshold, and nothing was calling [`l2_kingdom::ai::run_step`]
//!    at all — the crate exposes the machine and the handlers and had no
//!    driver. [`drive_ai`] is the driver: one step per realm per tick, realms in
//!    ascending index order, dispatching the four handlers `l2-kingdom`
//!    implements and skipping the ten whose state lives elsewhere.
//!
//! # Determinism
//!
//! Nothing here reads a clock, allocates a decision, or iterates anything but
//! an ascending index range. A turn is a pure function of the kingdom it
//! started from: the same save ended twice produces the same numbers, which is
//! what `tests/turn.rs` asserts by running one twice. The unit sweep keeps that
//! property because it walks slots 1 … 150 in order and stops at the first
//! battle — `docs/netcode.md` §5's rule, and the reason the stop is reproduced
//! rather than tidied away.

use l2_kingdom::ai::{self, AiStep};
use l2_kingdom::conquest::Attack;
use l2_kingdom::phase::{Phase, PhaseWait};
use l2_kingdom::units_tick::{Contact, Encounter};
use l2_kingdom::victory::Outcome;
use l2_kingdom::{Kingdom, SeasonReport};

use crate::engagement::{self, Answer, BattleReport, SiegePhase};
use crate::game::Game;

/// How many `Turn_Tick` calls one whole turn may take before we conclude the
/// machine is not going to come round.
///
/// A real turn takes far fewer — `tests/turn.rs` pins the actual number — and
/// this exists only so a bug in a wait condition is a diagnosable stop rather
/// than a hang in the event loop.
pub const MAX_TICKS: u32 = 512;

/// What one end-of-turn did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnOutcome {
    pub report: SeasonReport,
    /// How many phase ticks it took. Interesting only as a check on the
    /// machine; nothing branches on it.
    pub ticks: u32,
    /// `DAT_0053F0C4` after this turn: whether the game is now over, and how.
    /// [`Outcome::InPlay`] almost always. See [`crate::victory`].
    pub outcome: Outcome,
    /// How many tiles were entered by anything, over the whole turn. Zero on a
    /// turn where nobody had anywhere to go — and the number that was
    /// necessarily zero while four phases did nothing.
    pub steps: usize,
    /// Everything the unit sweep reported, in the order it happened. Battles,
    /// captures and blocked moves.
    pub contacts: Vec<Contact>,
    /// Battles that were raised and **not fought** — normally empty, because
    /// [`resolve_battle`] fights them. An entry here means
    /// [`crate::engagement::resolve`] declined the pair, which it does when
    /// either slot is no longer a unit. Reported rather than swallowed.
    pub pending_battles: Vec<Encounter>,
    /// **Every battle this turn settled, oldest first** — one per army that
    /// walked into an enemy, plus one per siege assault phase 2 launched.
    ///
    /// A turn used to be able to destroy two armies and hand back nothing that
    /// said so; the caller had to diff the unit array. This is what screen
    /// `0x13` draws, and what a message log would read.
    pub battles: Vec<BattleReport>,
}

impl TurnOutcome {
    /// Counties that changed hands during the turn.
    pub fn captures(&self) -> impl Iterator<Item = (usize, u8)> + '_ {
        self.contacts.iter().filter_map(|c| match c {
            Contact::Castle { unit, county, outcome: l2_kingdom::conquest::Attack::Captured } => {
                Some((*unit, *county))
            }
            _ => None,
        })
    }
}

/// Run the phase machine until the end-of-season pipeline has run once.
///
/// Returns `None` only if [`MAX_TICKS`] is reached, which would be a bug in a
/// wait condition rather than something a player can cause.
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
    // question. `Ask` reaching here would be a caller using the wrong door.
    match advance(game, Resume::Start, false) {
        TurnStep::Done(outcome) => Some(*outcome),
        _ => None,
    }
}

/// **Where a turn got to.** [`begin_turn`]'s and [`answer_battle`]'s answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TurnStep {
    /// The turn came round. Boxed because a [`TurnOutcome`] is far larger than
    /// the other two arms and every caller matches on the enum.
    Done(Box<TurnOutcome>),
    /// **The turn is suspended, and the player is being asked.** *"A Battle is
    /// to be fought. Will you take the field?"* — `L2.eng` group 80, screen
    /// `0x12`.
    ///
    /// The kingdom is mid-turn and must not be touched until
    /// [`answer_battle`] carries it on: the two armies are standing on the same
    /// tile with the battle unresolved, which is exactly the state the original
    /// leaves the campaign in while the prompt is up.
    Ask(Question),
    /// **A battle has been settled and the player has not been shown it.**
    /// *"The Battle is decided."* — `L2.eng` group 81, screen `0x13`.
    ///
    /// The turn is still suspended: the original puts `0x13` up the instant the
    /// battle ends and the campaign does not move again until the corner button
    /// is clicked. [`dismiss_report`] is that click.
    Report(Box<BattleReport>),
    /// **One `Turn_Tick` has happened and the turn is not over.** Draw a frame
    /// and call [`tick_turn`] again.
    ///
    /// # This is what gives a turn a duration
    ///
    /// The interactive door used to run the phase machine to completion unless
    /// a battle stopped it, so on a turn with no battle in it **every phase
    /// happened between two frames**. A merchant walked its entire route in the
    /// time it took to return from `Machine::handle`, which on screen is a
    /// teleport; an army never appeared to march; and there was no interval
    /// during which a screen could be dark. A player reported all three
    /// (*"the merchant seems to just teleport on end turn and the screen
    /// doesn't go dark"*) and they were one defect.
    ///
    /// The original's turn is spread over frames by construction: `Turn_Tick`
    /// is called once per frame from the main loop, `Units_Tick` right after
    /// it, and three of the seven phases do nothing but **wait for the units
    /// they started to stop moving**. Those waits are not bookkeeping — they
    /// are the pacing, and they are what makes a march something a player can
    /// watch. `docs/decisions.md` C35 established where the mover is dispatched
    /// from; this is the other half of the same fact, which is that the
    /// dispatch happens *many times*.
    ///
    /// [`end_turn`], the headless door, never returns this: a caller with no
    /// frames to spread a turn over has nothing to do with it.
    Running,
    /// The phase machine did not come round inside [`MAX_TICKS`]. A bug in a
    /// wait condition rather than anything a player can cause.
    Stuck,
}

/// **A battle waiting for an answer.** Everything a screen needs to name the
/// two sides without reaching into the kingdom, plus enough to settle it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Question {
    pub attacker: usize,
    pub defender: usize,
    /// The county fought over — the besieged one for an assault.
    pub county: u8,
    /// A siege assault rather than a field battle. `L2.eng` group 80 index 7 is
    /// *"The Siege commences."* where a field battle draws index 0.
    pub is_siege: bool,
    /// Which realm each side belongs to, read while both records still exist.
    pub attacker_owner: u8,
    pub defender_owner: u8,
    pub attacker_men: i32,
    pub defender_men: i32,
    /// The seven troop counts each side is taking onto the field — the roster
    /// screen `0x12` draws down the middle of its window.
    pub attacker_roster: crate::engagement::Roster,
    pub defender_roster: crate::engagement::Roster,
    /// **Whose choice it is** — `g_battleChoiceOwner`, which picks between
    /// `L2.eng` group 80's indices 1, 2 and 3 and decides whether the two thumb
    /// widgets are drawn at all.
    ///
    /// 1 *"Will you take the field?"* — the local player chooses, and gets the
    /// buttons. 2 *"Your opponent has the choice…"*. 0 *"The opponents are
    /// deciding how they will fight this engagement."* — a battle between two
    /// other realms, which draws no buttons.
    ///
    /// **`[I]` on which of two humans holds the choice**, which cannot arise
    /// until there are two: the attacker is taken to hold it, because the
    /// attacker is the side that pressed. The three strings and the "no buttons
    /// for a bystander" rule are `[V]`.
    pub choice_owner: u8,
}

impl Question {
    /// Whether `player` is the attacker, the defender, or neither.
    pub fn side_of(&self, player: u8) -> Option<bool> {
        if player == self.attacker_owner {
            Some(true)
        } else if player == self.defender_owner {
            Some(false)
        } else {
            None
        }
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
/// while he watches, instead of standing still until he presses End Turn — the
/// second half of *"can't seem to move my army"*, and indistinguishable from
/// the order having been ignored.
///
/// A unit walks at most `moveAllowance - movesUsed` tiles and then stops, so a
/// player who sits on the map gets no extra movement out of it;
/// `Pass::UnitsResetMoves` at the end of the season is what starts it again.
///
/// A battle raised here is settled by the standing policy rather than by a
/// prompt: this is not a turn, there is no [`TurnProgress`] to suspend, and a
/// screen `0x12` raised from an idle frame would have nothing to carry on
/// afterwards. Returns how many tiles were entered, so a caller can decide
/// whether the frame needs repainting.
pub fn tick_units_only(game: &mut Game) -> usize {
    let moved = game.kingdom.tick_units();
    let stepped = moved.stepped;
    if let Some(e) = moved.battle() {
        let answer = game.field_policy;
        let attack = Attack::Battle { attacker: e.mover, defender: e.occupant };
        let seed = battle_seed(&game.kingdom, e);
        let _ = engagement::resolve(&mut game.kingdom, attack, e.county, answer, seed);
    }
    stepped
}

/// Whether a turn is suspended waiting on an answer.
pub fn pending_question(game: &Game) -> Option<Question> {
    game.turn.as_ref().and_then(|p| p.question)
}

/// The battle a suspended turn has settled and not yet shown — what screen
/// `0x13` draws.
pub fn pending_report(game: &Game) -> Option<&BattleReport> {
    game.turn.as_ref().and_then(|p| p.unseen.as_ref())
}

/// Whether a turn is in flight at all — which is the state a caller must not
/// start a second one from, and must not save from.
pub fn turn_in_flight(game: &Game) -> bool {
    game.turn.is_some()
}

/// What the caller is handing back to a suspended turn.
///
/// A plain `Option<Answer>` could not express the difference between *"carry
/// on, I have seen the result"* and *"carry on, I have nothing to say"*, and
/// the two are different: the first consumes a report and the second would hand
/// the same report back for ever.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Resume {
    /// Start a turn, or carry one on with nothing to contribute.
    Start,
    /// The answer to *"will you take the field?"*.
    Answer(Answer),
    /// The result screen has been seen.
    Dismiss,
}

/// Where the tick loop is when it is put down mid-turn.
///
/// Three points, and they are the three places a turn can be interrupted: at
/// the top of a tick, part-way through phase 2's assaults, and after the unit
/// sweep has raised a battle but before the tick's bookkeeping has run. A turn
/// resumed at the wrong one would run `begin_phase` twice or lose a season
/// report, so the stage is stored rather than guessed from the other fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Stage {
    /// Start a tick: count it, and run the phase's first-call work.
    #[default]
    Begin,
    /// Run the phase itself — or, while phase 2's pump is loaded, one assault
    /// of it.
    Phase,
    /// Finish the tick the unit sweep interrupted.
    Tail,
}

/// What the second half of an interrupted tick still has to do.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Tail {
    contacts: Vec<Contact>,
    report: Option<SeasonReport>,
    /// The battle the sweep raised, still unresolved.
    battle: Option<Encounter>,
}

/// **A turn, mid-flight.** Everything [`end_turn`]'s loop used to hold in
/// locals, hoisted so the loop can be left and re-entered.
///
/// It lives on the [`Game`] rather than being handed back to the caller because
/// a half-run turn is not something a caller may drop: the kingdom is in a state
/// no rule describes — two armies on one tile, a season report computed and not
/// yet delivered — and the only safe thing to do with it is finish it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TurnProgress {
    ticks: u32,
    steps: usize,
    contacts: Vec<Contact>,
    pending_battles: Vec<Encounter>,
    /// The AI's resource grant runs once a turn. See `run_handler`.
    granted: bool,
    stage: Stage,
    /// Phase 2's assault pump while it is loaded.
    siege: Option<SiegePhase>,
    /// The question on the table, if the turn is suspended.
    question: Option<Question>,
    /// The assault the question is about, already launched and not yet settled.
    /// `None` when the question is about a field battle.
    pending_assault: Option<l2_kingdom::siege::Assault>,
    tail: Option<Tail>,
    /// Battles settled this turn, oldest first — what [`TurnOutcome::battles`]
    /// is built from.
    reports: Vec<BattleReport>,
    /// The one the player has not been shown yet. Held separately because a
    /// screen has to see it *before* the turn ends, and because dropping it on
    /// the floor would be the difference between a battle happening and a
    /// battle being noticed.
    unseen: Option<BattleReport>,
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

        let stage = match game.turn.as_ref() {
            Some(p) => p.stage,
            None => return TurnStep::Stuck,
        };
        match stage {
            Stage::Begin => {
                let p = game.turn.as_mut().expect("checked above");
                if p.ticks >= MAX_TICKS {
                    // Give the half-turn back rather than leaving it parked:
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
                // it and left it here rather than running it, which is what
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
    let moved = game.kingdom.tick_units();
    let Some(p) = game.turn.as_mut() else { return };
    p.steps += moved.stepped;
    p.stage = Stage::Tail;
    p.tail = Some(Tail { battle: moved.battle(), contacts: moved.contacts, report });
}

/// The rest of the tick: the battle, the contacts, and — on the last tick of
/// the turn — the season report.
///
/// The order is the original's and is not cosmetic: the battle is fought before
/// the contacts are recorded and before the report is delivered, so a realm
/// whose last army dies this tick is eliminated by [`Game::rank_realms`] on the
/// same turn rather than the next one.
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
    game.last_report = Some(report.clone());
    // `Turn_Tick`'s phase 7 calls `Score_RankRealms` after `Season_Advance` —
    // one of its five callers, and the one that can crown a survivor at the end
    // of a season in which nobody died. `Pass::ScoreRank` inside the pipeline
    // has already ranked; this adds the leader/trailer scan that the pass
    // deliberately does not own, because the pass does not know who the local
    // player is.
    game.rank_realms();
    let outcome = game.campaign.settle(game.player);
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
    let seed = siege_seed(&game.kingdom);
    // A refusal is not a battle and cannot be asked about: the siege has
    // already been lifted by `siege::assault` and there is nothing to fight.
    let question = SiegePhase::settlement(&game.kingdom, assault).and_then(|(a, d, s)| {
        (s == l2_kingdom::battle::Settlement::Prompt).then(|| question_for(game, a, d, true))
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
            record(p, report);
        }
    }
}

/// File a settled battle: onto the turn's list, and onto the one screen `0x13`
/// has still to show.
fn record(p: &mut TurnProgress, report: Option<BattleReport>) {
    if let Some(report) = report {
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
        let q = question_for(game, e.mover, e.occupant, false);
        let p = game.turn.as_mut().expect("raised inside a turn");
        p.question = Some(Question { county: e.county, ..q });
        return;
    }
    let answer = game.field_policy;
    settle_question(
        game,
        Question { county: e.county, ..question_for(game, e.mover, e.occupant, false) },
        answer,
    );
}

/// Read the two records into a [`Question`] while both still exist.
fn question_for(game: &Game, attacker: usize, defender: usize, is_siege: bool) -> Question {
    let units = &game.kingdom.campaign.units;
    let read = |id: usize| {
        units.get(id).map_or((0u8, 0i32, 0u8, false, [0; l2_kingdom::unit::TROOP_TYPES]), |u| {
            (u.owner, u.men, u.besieging_county, u.owner_is_human, u.troops)
        })
    };
    let (attacker_owner, attacker_men, besieged, a_human, attacker_roster) = read(attacker);
    let (defender_owner, defender_men, _, d_human, defender_roster) = read(defender);
    let county = if is_siege { besieged } else { units.get(defender).map_or(0, |u| u.county) };
    // `g_battleChoiceOwner`. A player who owns neither army is only told about
    // the battle; otherwise the attacker holds the choice, and the defender is
    // told that his opponent holds it.
    let me = game.player;
    let choice_owner = if me != attacker_owner && me != defender_owner {
        0
    } else if me == defender_owner && a_human {
        2
    } else if me == attacker_owner || !d_human {
        1
    } else {
        2
    };
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
        record(p, report);
        return;
    }
    let attack = Attack::Battle { attacker: q.attacker, defender: q.defender };
    let seed = battle_seed(
        &game.kingdom,
        Encounter { mover: q.attacker, occupant: q.defender, county: q.county },
    );
    match engagement::resolve(&mut game.kingdom, attack, q.county, answer, seed) {
        Some(report) => {
            if let Some(p) = game.turn.as_mut() {
                record(p, Some(report));
            }
        }
        None => {
            // Not resolvable — a slot is no longer a unit. Reported rather than
            // swallowed, exactly as it always was.
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
/// call rather than one assault a tick. The two agree on every number because
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
/// number to it, so two assaults in one phase fight different battles and the
/// same phase run twice fights the same two.
fn siege_seed(kingdom: &Kingdom) -> u64 {
    let mut z = (kingdom.turn_count as u64)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ 0x5165_6765_0000_0002;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// **The battle seam.** Two enemy armies have met, and somebody has to fight
/// them.
///
/// `l2-kingdom` reports the pair and stops the mover; it cannot resolve the
/// fight because resolving it means `l2-sim`, and `docs/plan.md`'s dependency
/// rule is one way. [`crate::engagement`] is the only module in the workspace
/// that depends on both, and this is the campaign's way in.
///
/// > This was written as a marked stub — the mover landed while battle
/// > resolution was being built on another branch, and an unfought pair went
/// > into [`TurnOutcome::pending_battles`] so that filling the seam in would
/// > change a test rather than pass either way. Both have landed, and the
/// > handoff is a real call now. `pending_battles` stays, because there is
/// > still one case that does not resolve: see below.
///
/// # The player *is* asked now, and this is not the door it happens at
///
/// This used to carry a note saying `end_turn` had no screen to raise *"will
/// you take the field?"* on and therefore answered [`Answer::Decline`], and
/// that when the map screen could raise the prompt *"this is the one line that
/// changes"*. It was not one line, and the note was wrong about where the
/// change belonged: a prompt has to **suspend the turn**, because the campaign
/// is left mid-tick with two armies on one tile while the player thinks. So the
/// interactive path is [`begin_turn`] and [`answer_battle`], and this function
/// is what remains for a caller resolving one encounter by itself — a test, or
/// anything outside the turn loop. It answers the game's standing
/// [`Game::field_policy`](crate::game::Game::field_policy).
///
/// # The seed is state, never a clock
///
/// `docs/netcode.md`: two peers fight the same battle or they are not playing
/// the same game. The seed is mixed from the turn counter, the two unit slots
/// and the county — all of which both peers agree on — rather than drawn from
/// [`l2_kingdom::Kingdom::rng`], because drawing would advance the generator
/// the weather and the event deck share and make a turn with a battle in it
/// roll different weather from the same turn without one.
///
/// Returns the encounter **unfought** if it could not be resolved, which is
/// then reported rather than swallowed.
pub fn resolve_battle(game: &mut Game, encounter: Encounter) -> Option<Encounter> {
    let attack = Attack::Battle { attacker: encounter.mover, defender: encounter.occupant };
    let seed = battle_seed(&game.kingdom, encounter);
    let answer = game.field_policy;
    match engagement::resolve(&mut game.kingdom, attack, encounter.county, answer, seed) {
        Some(_) => None,
        None => Some(encounter),
    }
}

/// A battle's seed, from simulation state both peers already agree on.
///
/// Splitmix64's finaliser over a mix of the turn counter, the two slots and the
/// county. Any deterministic mix would do; what matters is that no term is a
/// clock, an address or an iteration order.
fn battle_seed(kingdom: &Kingdom, e: Encounter) -> u64 {
    let mut z = (kingdom.turn_count as u64)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (e.mover as u64) << 32
        ^ (e.occupant as u64) << 8
        ^ e.county as u64;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// The work a phase does on its first call.
///
/// * **Phase 1** — `docs/kingdom.md` §3.1 says the neutral counties get their tax
///   rates and their fields set once a turn, on the neutral ladder, and realm
///   **0** is how the original addresses them: `AI_SetTaxRates(0)` then
///   `AI_ManageFields(0)` — which is `0x0049DFC6`, the *unowned* counties' pass,
///   and not the AI realms' step 5. See `l2_kingdom::ai_farm`.
/// * **Phase 4** — `AI_RunTurnStep`'s **step 0** for every realm: recount its
///   strength, eliminate it if that comes out zero, and rank. See
///   [`Game::recount_realm`](crate::game::Game::recount_realm), and note that it
///   runs for the human too.
/// * **Phases 3, 5 and 6** originate the game's own move orders, and that is
///   `l2-kingdom`'s to do because it is a rule about units:
///   [`Kingdom::begin_unit_phase`] is the transport re-target, the peasant-mob
///   cursor and `Merchant_AdvanceAll`. **Phase 2 is sieges and originates
///   nothing** — see [`l2_kingdom::units_tick`].
///
/// # The one difference from the original, and the condition it rested on has
/// gone
///
/// `AI_RunTurnStep` interleaves: realm 1 takes step 0, then realm 2 takes step 0,
/// and by the time realm 5 reaches its own step 0 the earlier realms have taken
/// several of their fourteen. So a realm that realm 2's turn destroys is noticed
/// *later in the same phase*. Here all five step 0s happen at the top of the
/// phase instead.
///
/// > This note used to end: *"The two are the same as long as nothing inside
/// > phase 4 takes a county or destroys an army — which is true today, because
/// > both happen in phase 2 — and this is the note to read when that stops
/// > being true."* **It has stopped being true, and this is that reading.**
/// > Movement is not confined to a phase: `Units_Tick` runs on every tick of
/// > the turn (`docs/decisions.md` C35), so an army can reach a castle, take a
/// > county or lose a battle *during phase 4*, after the step 0s have all run.
/// >
/// > What that costs is bounded and is not a divergence in the numbers: a realm
/// > eliminated by a battle inside phase 4 is recounted at the end of the turn
/// > by [`Game::rank_realms`](crate::game::Game::rank_realms) rather than
/// > mid-phase, so it is noticed one phase later than the original notices it.
/// > Nothing between the two reads the elimination. Interleaving the step 0s
/// > properly is the fix, and it belongs with whoever next touches the AI
/// > dispatcher.
fn begin_phase(game: &mut Game, phase: Phase) {
    game.kingdom.begin_unit_phase(phase);
    match phase {
        // **Phase 2 is sieges, and this is where they run.** The variant keeps
        // the name `ArmyMovement` because five files spell it; what it does was
        // corrected in `docs/decisions.md` C35 and it is `Siege_StartPhase`, a
        // cursor over `Siege_BuildTick`, and `Siege_LaunchAssault` for every
        // army whose engines came ready.
        //
        // `engagement::run_siege_phase` has been that pump end to end since it
        // was written and **nothing called it outside its own tests** — a
        // besieging army in a played turn built nothing and never assaulted.
        // This is the call.
        //
        // **The pump is loaded here and run by the tick loop**, one assault at
        // a time, so that each assault can stop and ask the player. Running it
        // to exhaustion in this call — which is what it used to do — is what
        // made a siege the one battle nobody could be asked about.
        Phase::ArmyMovement => {
            let phase = SiegePhase::begin(&mut game.kingdom);
            if let Some(p) = game.turn.as_mut() {
                p.siege = Some(phase);
            }
        }
        Phase::NeutralCounties => {
            game.kingdom.run_ai_tax_rates(0);
            // The unowned counties farm too, by whichever lord held them last.
            // `NoMarket` is the merchant seam: there is no stall yet, so every
            // style's opening shopping cascade is refused and the county farms
            // what it already has. See `l2_kingdom::ai_farm`.
            game.kingdom.run_neutral_farms(&mut l2_kingdom::ai_farm::NoMarket);
        }
        Phase::PlayersTurn => {
            for id in 1..l2_kingdom::realm::MAX_REALMS {
                game.recount_realm(id as u8);
            }
        }
        _ => {}
    }
}

/// One step of every AI realm that has not finished its turn.
///
/// Realms are walked in ascending index order and each takes exactly one step
/// per tick, which is what the original's per-frame dispatcher does. The step
/// to run is the counter's value *before* the increment, so
/// [`ai::pending_step`] is read first and [`ai::run_step`] second.
///
/// > `launched_army` **used to be passed `false`**, on the grounds that "it
/// > asks whether the realm found an idle army to start moving, and there are
/// > no armies". There are armies now, and the argument is the real predicate:
/// > `FUN_004A4E3D(1, realm)`, which is
/// > [`Kingdom::realm_units_moving`](l2_kingdom::Kingdom::realm_units_moving).
/// > It matters because it is half of the finish test — `docs/armies.md` §3.2:
/// > a realm is done only when its `aiStep` has passed `15 + 2 × realmIndex`
/// > **and** none of its armies is still walking. A realm marching on somebody
/// > keeps taking steps.
pub fn drive_ai(kingdom: &mut Kingdom, granted: &mut bool) {
    for id in 1..kingdom.realms.len() {
        if kingdom.realms[id].turn_done() {
            continue;
        }
        let marching = kingdom.realm_units_moving(l2_kingdom::UnitKind::Army, id as u8);
        let step = ai::pending_step(&kingdom.realms[id], id);
        let _finished = ai::run_step(&mut kingdom.realms[id], id, marching);
        if let Some(step) = step {
            run_handler(kingdom, id as u8, step, granted);
        }
    }
}

/// Dispatch one AI handler.
///
/// Four of the fourteen are implemented in `l2-kingdom`; the other ten drive
/// armies, merchants, diplomacy and map tiles, which no crate owns yet. They
/// are skipped rather than stubbed, so nothing here pretends to be a rule.
fn run_handler(kingdom: &mut Kingdom, realm: u8, step: AiStep, granted: &mut bool) {
    match step {
        AiStep::SetTaxRates => {
            kingdom.run_ai_tax_rates(realm);
            // **The grant runs once a turn, not once per realm**, and that is a
            // real difference from the original worth writing down.
            // `AI_SetTaxRates(realm)` grants *that* realm its gold and its
            // counties their free people, herd and grain;
            // `l2_kingdom::ai::grant_resources` instead walks every AI realm
            // and every AI-owned county in one pass. Calling it inside each
            // realm's step 3 would hand every realm four turns' worth of gold.
            // Running it once, at the first step 3 of the turn, puts it where
            // the original puts it and produces the original's totals.
            //
            // On the England turn-one scenario the difficulty is 0 and every grant
            // multiplies by it, so nothing moves either way; the distinction
            // matters the moment somebody starts a harder game.
            if !*granted {
                kingdom.run_ai_grants();
                *granted = true;
            }
        }
        AiStep::ManageFields => {
            kingdom.run_ai_farms(realm, &mut l2_kingdom::ai_farm::NoMarket);
        }
        AiStep::BuildCastles => {
            kingdom.run_ai_castles(realm);
        }
        AiStep::ChooseIndustry => kingdom.run_ai_industry(realm),
        // The letters are dropped rather than shown: a realm-to-realm taunt is
        // not a season message and `l2-game` has no letter screen yet. The
        // realm's timer, stage and voice rotation still advance, which is the
        // whole of the step's effect on the simulation.
        AiStep::Taunt => {
            kingdom.run_ai_taunt(realm);
        }
        AiStep::UpdateTotals => update_totals(kingdom, realm),
        // An empty function in the shipped binary. Named, and it does nothing
        // here for the same reason it does nothing there.
        AiStep::Nothing => {}
        _ => {}
    }
}

/// > `armies` and `total_men` **used to be zero**, "because the unit array does
/// > not exist yet". It does, and they come from it:
/// > [`Units::realm_totals`](l2_kingdom::Units::realm_totals) counts the
/// > realm's armies and their men. They land in realm `+0x2C` and `+0x54`, and
/// > `+0x54` is a score input — so this is the difference between a score that
/// > is the economy's part and a score that is the whole, and every realm's
/// > rank moves the first turn anybody raises an army.
fn update_totals(kingdom: &mut Kingdom, realm: u8) {
    let (armies, total_men) = kingdom.campaign.units.realm_totals(realm);
    ai::update_realm_totals(
        &mut kingdom.realms[realm as usize],
        &kingdom.counties,
        kingdom.county_count,
        realm,
        armies,
        total_men,
    );
}

