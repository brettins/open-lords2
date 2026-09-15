//! `Turn_Tick` (`0x0049A010`) dispatches on `g_turnPhase` every frame and
//! `Turn_AdvancePhase` wraps 7 back to 1. `l2-kingdom` models that as
//! [`TurnMachine`](l2_kingdom::phase::TurnMachine) and deliberately stops
//! there: **three** of the seven phases wait on units moving, one waits on
//! sieges, one on the AI,
//! caller does. This module is that caller.
//!
//! > The line above used to say *"four of the seven phases wait on units
//! > moving, and units are not that crate's state"*. Both halves have since
//! > stopped being true. Units **are** `l2-kingdom`'s state — `Campaign` lives
//! > inside `Kingdom` because two lockstep peers have to agree about where an
//! > army stands — and phase 2 waits on sieges
//! > (`docs/decisions.md` C35). What is left on this side of the seam is not
//! > ownership of the units; it is the two things below that are not
//! > rules: what a phase's wait *means* to an application, and who fights a
//! > battle.
//!
//!    The phases only originate the game's own orders — transports in 3, mobs
//!    in 5, merchants in 6 — and then wait for them to stop. Phase 2
//!    originates nothing and waits on sieges, not armies; `docs/kingdom.md`
//!    §3.1 said otherwise and is corrected (`docs/decisions.md` C35).

mod engine;
pub use engine::*;
mod types;
pub use types::*;
mod battle;
pub use battle::*;
mod ai_part;
pub use ai_part::*;
mod tests;
pub use tests::*;

use l2_kingdom::ai::{self, AiStep};
use l2_kingdom::conquest::Attack;
use l2_kingdom::phase::{Phase, PhaseWait};
use l2_kingdom::units_tick::{Contact, Encounter};
use l2_kingdom::victory::Outcome;
use l2_kingdom::{Kingdom, SeasonReport};

use crate::engagement::{self, Answer, BattleReport, SiegePhase};
use crate::game::Game;

pub const MAX_TICKS: u32 = 2048;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnOutcome {
    pub report: SeasonReport,
    pub ticks: u32,
    /// `DAT_0053F0C4` after this turn: whether the game is now over, and how.
    pub outcome: Outcome,
    pub steps: usize,
    pub contacts: Vec<Contact>,
    pub pending_battles: Vec<Encounter>,
    pub battles: Vec<BattleReport>,
}

impl TurnOutcome {
    pub fn captures(&self) -> impl Iterator<Item = (usize, u8)> + '_ {
        self.contacts.iter().filter_map(|c| match c {
            Contact::Castle { unit, county, outcome: l2_kingdom::conquest::Attack::Captured(_) } => {
                Some((*unit, *county))
            }
            _ => None,
        })
    }
}

