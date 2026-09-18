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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TurnStep {
    Done(Box<TurnOutcome>),
    /// **The turn is suspended, and the player is being asked.** *"A Battle is
    /// to be fought. Will you take the field?"* — `L2.eng` group 80, screen
    /// `0x12`.
    Ask(Question),
    /// *"The Battle is decided."* — `L2.eng` group 81, screen `0x13`.
    Report(Box<BattleReport>),
    /// The original's turn is spread over frames by construction: `Turn_Tick`
    /// is called once per frame from the main loop, `Units_Tick` right after
    /// it, and three of the seven phases do nothing but **wait for the units
    /// they started to stop moving**. Those waits are not bookkeeping — they
    /// are the pacing, and they are what makes a march something a player can
    /// watch. `docs/decisions.md` C35 established where the mover is dispatched
    /// from; this is the other half of the same fact, which is that the
    /// dispatch happens *many times*.
    Running,
    Stuck,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Question {
    pub attacker: usize,
    pub defender: usize,
    pub county: u8,
/// A siege assault. `L2.eng` group 80 index 7 is
    /// *"The Siege commences."* where a field battle draws index 0.
    pub is_siege: bool,
    pub attacker_owner: u8,
    pub defender_owner: u8,
    pub attacker_men: i32,
    pub defender_men: i32,
    pub attacker_roster: crate::engagement::Roster,
    pub defender_roster: crate::engagement::Roster,
    /// **Whose choice it is** — `g_battleChoiceOwner`, which picks between
    /// `L2.eng` group 80's indices 1, 2 and 3 and decides whether the two thumb
    /// widgets are drawn at all.
    ///
    /// **`[I]` on which of two humans holds the choice**, which cannot arise
    /// until there are two: the attacker is taken to hold it, because the
    /// attacker is the side that pressed. The three strings and the "no buttons
    /// for a bystander" rule are `[V]`.
    pub choice_owner: u8,
    pub castle_level: Option<u8>,
}

impl Question {
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

#[cfg(test)]
pub(crate) fn suspend_on(game: &mut Game, question: Question) {
    game.turn =
        Some(TurnProgress { idle: true, question: Some(question), ..TurnProgress::default() });
}

pub fn pending_question(game: &Game) -> Option<Question> {
    game.turn.as_ref().and_then(|p| p.question)
}

pub fn pending_report(game: &Game) -> Option<&BattleReport> {
    game.turn.as_ref().and_then(|p| p.unseen.as_ref())
}

pub fn turn_in_flight(game: &Game) -> bool {
    game.turn.is_some()
}

pub fn players_turn_ended(game: &Game) -> bool {
    game.turn.as_ref().is_some_and(|p| !p.idle)
}

/// `Screen_DrawMenuBar` (`0x00419C78`) draws one banner per realm under
///
/// `Turn_End` (`0x0043AC23`) writes it for the local player the instant End
/// Turn is clicked, `FUN_004479E9` writes it for a remote seat,
/// `AI_RunTurnStep` parks an AI realm on it when that realm is finished, and
/// `Turn_BeginPlayersTurn` (`0x0049B6D3`) clears every living realm back to 0.
///
/// `Turn_End` also raises `DAT_0056D6A0`, which is half of
/// `Screen_DrawMenuBar`'s own repaint guard — the button forces the bar to
/// repaint so that the shield goes.
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Resume {
    Start,
    Answer(Answer),
    Dismiss,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum Stage {
    #[default]
    Begin,
    Phase,
    Tail,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Tail {
    pub(super) contacts: Vec<Contact>,
    pub(super) report: Option<SeasonReport>,
    pub(super) battle: Option<Encounter>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TurnProgress {
    pub(super) idle: bool,
    pub(super) ticks: u32,
    pub(super) steps: usize,
    pub(super) contacts: Vec<Contact>,
    pub(super) pending_battles: Vec<Encounter>,
    pub(super) granted: bool,
    pub(super) stage: Stage,
    pub(super) siege: Option<SiegePhase>,
    pub(super) question: Option<Question>,
    pub(super) pending_assault: Option<l2_kingdom::siege::Assault>,
    pub(super) tail: Option<Tail>,
    pub(super) reports: Vec<BattleReport>,
    pub(super) unseen: Option<BattleReport>,
}

