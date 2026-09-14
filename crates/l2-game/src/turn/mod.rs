//! Ending a turn: driving `l2-kingdom`'s phase machine all the way round.
//!
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
//! # What the spine has to supply
//!
//! Three things,
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
//! at all — the crate exposes the machine and the handlers and had no
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
//! battle — `docs/netcode.md` §5's rule,
//!

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

/// How many `Turn_Tick` calls one whole turn may take before we conclude the
/// machine is not going to come round.
///
/// A real turn takes far fewer — `tests/turn.rs` pins the actual number — and
/// this exists only so a bug in a wait condition is a diagnosable stop rather
/// than a hang in the event loop.
///
/// # Why it is 2048 and was 512
///
/// **512 was set when a unit crossed a tile per tick**, which it no longer
/// does: `Unit_StepOnce`'s sub-tile counter costs 8 ticks a road tile and 32
/// an open one ([`l2_kingdom::units_tick`]). Three of the seven phases wait for
/// a whole class of unit to stop walking, one after another,
/// keeps taking steps while any of its armies is still moving —
/// `docs/armies.md` §3.2. So the bound is four consecutive waits, each as long
/// as the slowest possible leg.
///
/// The slowest leg is 15 move points spent entirely off road: 3 a tile buys 5
/// tiles, 32 ticks each, **160 ticks**. Spending them on roads is *cheaper* in
/// ticks, not dearer — 15 tiles at 8 — so 160 is the maximum and
/// `4 × 160 + the fixed phases ≈ 680`. 2048 is threefold headroom on that.
///
/// **The cost of raising it is real and is the reason for the arithmetic**: a
/// wedged turn now takes 33 seconds of wall clock to report itself
/// instead of 8. That is the trade — a bound that a legitimate turn can cross
/// is worse than a slow diagnosis, because it stops the turn *and* looks like
/// the bug it is not.
pub const MAX_TICKS: u32 = 2048;

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
    /// turn where nobody had anywhere to go —
    /// necessarily zero while four phases did nothing.
    pub steps: usize,
    /// Everything the unit sweep reported, in the order it happened. Battles,
    /// captures and blocked moves.
    pub contacts: Vec<Contact>,
    /// Battles that were raised and **not fought** — normally empty, because
    /// [`resolve_battle`] fights them. An entry here means
    /// [`crate::engagement::resolve`] declined the pair, which it does when
/// either slot is no longer a unit. Reported.
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
            Contact::Castle { unit, county, outcome: l2_kingdom::conquest::Attack::Captured(_) } => {
                Some((*unit, *county))
            }
            _ => None,
        })
    }
}

