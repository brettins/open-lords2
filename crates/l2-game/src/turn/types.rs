#![allow(unused_imports)]
use super::*;
use super::engine::*;
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
/// wait condition.
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
/// A siege assault. `L2.eng` group 80 index 7 is
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
    /// The besieged castle's level, or `None` for a field battle.
    ///
    /// Carried on the question because [`take_the_field`] needs it to raise the
    /// battlefield — `Battle_Start` picks `Battlefield_BuildCastle` over
    /// `Battlefield_BuildRandom` on exactly this —
    /// thing that survives from the assault to the answer.
    pub castle_level: Option<u8>,
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

/// **Put a question on the table without playing a turn to get one.**
///
/// [`TurnProgress`]'s fields are private to this module on purpose — a
/// suspended turn is a state only this machine may create —
/// about what happens *while* one is on the table would otherwise have to march
/// two armies together first. `cfg(test)` and crate-internal: it cannot be a
/// route in a running game.
#[cfg(test)]
pub(crate) fn suspend_on(game: &mut Game, question: Question) {
    game.turn =
        Some(TurnProgress { idle: true, question: Some(question), ..TurnProgress::default() });
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
///
/// It is also true while a battle raised on an ordinary frame is suspended
/// ([`TurnProgress::idle`]), and that is deliberate on both counts: the map must
/// not run its unit sweep while a question is on the table — `Units_Tick`'s own
/// latch — and End Turn must not be reachable underneath the prompt.
pub fn turn_in_flight(game: &Game) -> bool {
    game.turn.is_some()
}

/// **Whether the person has ended his turn and it is being run** — a turn in
/// flight that is not a battle suspended on an ordinary frame.
///
/// [`turn_in_flight`] is true for both, and for the turn timer the difference
/// is the whole question: `Turn_Tick` goes on counting under a battle prompt
/// raised while the person was playing, because `g_battlePhase` is still 0 and
/// nothing has called `Turn_End`. See [`crate::turn_clock`].
pub fn players_turn_ended(game: &Game) -> bool {
    game.turn.as_ref().is_some_and(|p| !p.idle)
}

/// **Whether a realm has finished its turn, as the menu bar's shield row asks
/// it.**
///
/// `Screen_DrawMenuBar` (`0x00419C78`) draws one banner per realm under
///
/// ```c
/// if ((g_realms[i].strength != 0) && (g_realms[i].aiStep < 999)) { ...draw...; slot++; }
/// ```
///
/// and `aiStep == 999` is the original's *"this realm's turn is over"*:
/// `Turn_End` (`0x0043AC23`) writes it for the local player the instant End
/// Turn is clicked, `FUN_004479E9` writes it for a remote seat,
/// `AI_RunTurnStep` parks an AI realm on it when that realm is finished, and
/// `Turn_BeginPlayersTurn` (`0x0049B6D3`) clears every living realm back to 0.
/// `Turn_End` also raises `DAT_0056D6A0`, which is half of
/// `Screen_DrawMenuBar`'s own repaint guard — the button forces the bar to
/// repaint so that the shield goes.
///
/// # Why this is not `ai_step < 999`
///
/// **Our turn is shaped differently, and a literal port inverts the row.** The
/// original's phase 4 *is* the interactive phase: the person sits inside it with
/// his counter parked at 1 while the AI realms step behind him. Ours parks the
/// person on the map between turns, with the phase machine at phase 1, and runs
/// the whole of phases 1 … 7 inside one press of End Turn — so when the person
/// is playing, every realm's counter is already at or past 999 from the turn
/// before. `ai_step < 999` would draw **no shields at all** exactly when the
/// original draws all of them.
///
/// So the two halves are mapped separately, and each mapping is exact:
///
/// * **The local player's counter is [`turn_in_flight`].** `Turn_End` writes
///   999 on the click and nothing clears it until the next turn begins, which is
///   the interval `game.turn` is `Some` for. The End Turn caption in
///   `screens::map` already reads the same flag this way, for
///   `Screen_DrawEndTurn`'s `aiStep < 999`.
/// * **An AI realm's counter is its own `ai_step`, inside a turn.** Outside one,
///   nobody has finished: the map between turns is the original's phase 4 before
///   anybody has pressed anything.
///
/// Not [`Realm::turn_done`](l2_kingdom::Realm::turn_done) either: that answers
/// *"may phase 4 stop waiting on this realm?"*, and it says yes for any human
/// at any time, which would take the person's own shield off the bar for good.
pub fn realm_turn_ended(game: &Game, realm: usize) -> bool {
    if !turn_in_flight(game) {
        return false;
    }
    if realm == game.player as usize {
        return true;
    }
    match game.kingdom.realms.get(realm) {
        Some(r) => r.ai_step >= l2_kingdom::AI_STEP_DONE,
        None => true,
    }
}

/// What the caller is handing back to a suspended turn.
///
/// A plain `Option<Answer>` could not express the difference between *"carry
/// on, I have seen the result"* and *"carry on, I have nothing to say"*, and
/// the two are different: the first consumes a report and the second would hand
/// the same report back for ever.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Resume {
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
/// report, so the stage is stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum Stage {
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
pub(super) struct Tail {
    pub(super) contacts: Vec<Contact>,
    pub(super) report: Option<SeasonReport>,
    /// The battle the sweep raised, still unresolved.
    pub(super) battle: Option<Encounter>,
}

/// **A turn, mid-flight.** Everything [`end_turn`]'s loop used to hold in
/// locals, hoisted so the loop can be left and re-entered.
///
/// It lives on the [`Game`] because
/// a half-run turn is not something a caller may drop: the kingdom is in a state
/// no rule describes — two armies on one tile, a season report computed and not
/// yet delivered —
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TurnProgress {
/// A battle raised by [`tick_units_only`] on an
    /// ordinary frame suspends the campaign in exactly the same fields — a
    /// [`Question`] for screen `0x12`, an `unseen` report for `0x13` — because
    /// the two screens and their three doors are the same ones. What it must
    /// never do is enter the phase machine: the player was watching his army
    /// walk, not ending his season, and winding a turn on because he answered a
    /// battle would advance the calendar behind his back.
    ///
    /// So [`advance`] drops the progress
    /// and there is nothing left to show. See [`tick_units_only`].
    pub(super) idle: bool,
    pub(super) ticks: u32,
    pub(super) steps: usize,
    pub(super) contacts: Vec<Contact>,
    pub(super) pending_battles: Vec<Encounter>,
    /// The AI's resource grant runs once a turn. See `run_handler`.
    pub(super) granted: bool,
    pub(super) stage: Stage,
    /// Phase 2's assault pump while it is loaded.
    pub(super) siege: Option<SiegePhase>,
    /// The question on the table, if the turn is suspended.
    pub(super) question: Option<Question>,
    /// The assault the question is about, already launched and not yet settled.
    /// `None` when the question is about a field battle.
    pub(super) pending_assault: Option<l2_kingdom::siege::Assault>,
    pub(super) tail: Option<Tail>,
    /// Battles settled this turn, oldest first — what [`TurnOutcome::battles`]
    /// is built from.
    pub(super) reports: Vec<BattleReport>,
    /// The one the player has not been shown yet. Held separately because a
    /// screen has to see it *before* the turn ends, and because dropping it on
    /// the floor would be the difference between a battle happening and a
    /// battle being noticed.
    pub(super) unseen: Option<BattleReport>,
}

