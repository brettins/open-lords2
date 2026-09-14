//! **How a game of Lords of the Realm II ends.**
//!
//! Four functions in the original, and the chain is short enough to state whole:
//!
//! 1. `FUN_0049B42B` ([`recount_strength`]) rebuilds realm `+0x04` as
//!    `3 * counties + 1 * armies`. A realm that comes out zero has just been
//!    eliminated and a message says so.
//! 2. `Score_RankRealms` (`0x0049AA0E`, [`rank_and_crown`]) scores and ranks the
//!    survivors, counts how many of them are **not** the local player, and — when
//!    the ranking table's first and last live entries are the same realm — crowns
//!    whoever is left.
//! 3. `Msg_DrawWindow`'s category-`0x0E` arm ([`outcome_of`]) turns the message
//!    being displayed into `DAT_0053F0C4`: **10 won, 11 lost**.
//! 4. `FUN_00497879` ([`crate::victory::Outcome`]'s consumer, which lives in the
//!    game crate) advances the campaign counter on a win and enters screen
//!    `0x1C`.
//!
//! # The victory condition, and it is not what it reads like
//!
//! `docs/plan.md` revision 4 records it as *"`Score_RankRealms` fires group 225
//! when the trailing realm equals the leader"* and warns that this "reads
//! strange". Read against the code it is not strange at all — but it is also not
//! what the sentence suggests. The comparison is
//!
//! ```c
//! if (g_rankTrailer == g_rankLeader) { ... }
//! ```
//!
//! and `g_rankLeader` / `g_rankTrailer` are **realm indices**, not scores: the
//! first and the last non-zero entry of the sorted ranking table, from which
//! every eliminated realm has just been struck out. Two realm *indices* are equal
//! exactly when the table has one live entry. So the condition is the ordinary
//! one — **one realm left standing** — written as a scan of a table
//! count. It is not "the bottom-ranked realm caught the leader on points".
//!
//! What *is* strange is everything around it, and all three are reproduced here:
//!
//! * **The empty case passes too.** With nobody in play, leader and trailer are
//! both `0`, `0 == 0`, and the original crowns `g_realms[0]` — the array slot.
//!   See [`Ranking::sole_survivor`].
//! * **The last realm standing being an AI does not end the game the first
//!   time.** It sends group 195, *"Just call me king."*, and sets the one-shot
//!   guard `+0xED`. `Score_RankRealms` runs many times a turn, so the *next* call
//!   finds the guard set, takes the other branch, and sends the human group 225
//!   *"Victory!"* — the human wins a game in which the human is dead. In practice
//!   the human's own group 224 has already set the outcome to 11 by then.
//! * **The mainline human victory is not this function at all.** It is
//!   [`outcome_of`]'s last branch: whenever a category-`0x0E` message is
//!   displayed and [`Ranking::opponents_remaining`] is zero, `Msg_DrawWindow`
//!   enqueues group 225 at the human. Killing the last AI raises group 194 for
//!   *that AI*; displaying 194 with no opponents left is what wins the game.
//!
//! # The human is treated differently, three times over
//!
//! * `AI_RunTurnStep` (`0x0049A581`) runs **step 0 for every realm**, including
//!   the human: the `isHuman` test guards the fourteen *handlers*, not the
//! initialisation above them. So the human's strength is recounted, and the
//!   human's elimination detected, on the human's own turn. See
//!   [`crate::ai::begin_realm_turn`], whose caller must therefore not skip
//!   humans.
//! * The message [`recount_strength`] sends is chosen on **"is this the local
//!   player"**, not on "is this a human": the local player gets group 224
//!   *"Defeat!"*, an AI gets group 194 *"Foiled again."*, and a **second human in
//!   a network game gets neither**.
//! * `voice_rotation` is advanced for all three of those cases, the local
//!   player's included, even though the human has no recorded voice lines.
//!
//! # What does *not* end a game
//!
//! Nothing here reads a turn limit, a score threshold or a date. The only ending
//! is elimination — every other realm's, or your own.

mod strength_part;
pub use strength_part::*;
mod ranking;
pub use ranking::*;
mod outcome;
pub use outcome::*;
mod tests_part;
pub use tests_part::*;

use crate::county::{County, MAX_COUNTIES};
use crate::realm::{Realm, MAX_REALMS};
use crate::tables::Tables;
use crate::unit::{UnitKind, Units};
use l2_net::{Quirk, Quirks};

/// `L2.eng` group 194 — *"Foiled again."*, an **AI** realm has been eliminated.
pub const MSG_AI_ELIMINATED: u16 = 194;
/// `L2.eng` group 195 — *"Just call me king."*, an AI is the last realm standing.
pub const MSG_AI_CROWNED: u16 = 195;
/// `L2.eng` group 224 — *"Defeat!"*, **you** have been eliminated.
pub const MSG_DEFEAT: u16 = 224;
/// `L2.eng` group 225 — *"Victory!"*.
pub const MSG_VICTORY: u16 = 225;

/// The message category the ending messages travel in. `Msg_DrawWindow`'s
/// `g_messageCategory == 0x0E` arm is the only place `DAT_0053F0C4` is written
/// during play.
pub const CATEGORY_ENDING: u8 = 0x0E;

/// `DAT_0053F0C4` — how the game ended, or that it has not.
///
/// The two live values are the original's own: **10 won, 11 lost**. Everything
/// that reads it (`FUN_00497879`, `FUN_0042E060`, `FUN_00476768`, the screen
/// `0x1C` painter) tests exactly those two numbers, so they are the
/// discriminants
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum Outcome {
    /// `0`. `Game_NewGame` and `FUN_004975D3` both clear it to this.
    #[default]
    InPlay = 0,
    Won = 10,
    Lost = 11,
}

impl Outcome {
    /// The byte the original stores.
    pub fn value(self) -> u8 {
        self as u8
    }

    pub fn from_value(v: u8) -> Option<Outcome> {
        match v {
            0 => Some(Outcome::InPlay),
            10 => Some(Outcome::Won),
            11 => Some(Outcome::Lost),
            _ => None,
        }
    }

    /// Whether the game is over. The two consumers that gate on it —
    /// `FUN_00476768` and `FUN_0042E060` — both write `(x == 0xb) || (x == 10)`.
    pub fn is_over(self) -> bool {
        self != Outcome::InPlay
    }
}

/// One thing the ending chain did, in the order it did it.
///
/// These are the `Msg_Enqueue` calls of `FUN_0049B42B` and `Score_RankRealms`,
/// kept as data because the message *queue* is not this crate's — and because
/// [`outcome_of`] reads two fields of a queued message (`group` and `from`) and
/// nothing else, so a caller with a real queue and a caller with a `Vec` reach
/// the same outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ending {
    /// `L2.eng` group: one of the four constants above.
    pub group: u16,
    /// `Msg_Enqueue`'s `from`, which `Msg_DrawWindow` reads back as
    /// `DAT_00553EE0`. **This, compared against the local player, is what tells
    /// a loss from someone else's loss** — see [`outcome_of`].
    pub from: u8,
    /// `Msg_Enqueue`'s `to`. Carried for faithfulness; nothing here branches on
    /// it.
    pub to: u8,
    /// The message category. Group 195 travels as category 1 and every other
    /// ending message as [`CATEGORY_ENDING`]
    /// cannot itself set an outcome.
    pub category: u8,
    /// `Msg_Enqueue`'s `+0x0C` — **which of the group's strings the window
    /// draws**, as `variant + 1` past the label at index 0.
    ///
    /// It matters here and it was missing: groups 194 and 195 hold **seventeen**
    /// strings apiece — a label and sixteen lord-flavoured laments — and the
    /// original picks one with [`voice_variant`], `lord * 4 + rotation - 4`. An
    /// `Ending` with no variant draws the Knight's first line for every lord in
    /// the game, for ever. Groups 224 and 225 pass 0 and have three and two
    /// strings.
    pub variant: u8,
}

/// `(voiceRotation - 4) + lord * 4` — the variant every lord-flavoured message
/// in the game is enqueued with, and the reason `Realm` carries a rotation at
/// `+0x159` at all.
///
/// Lords are 1..=4 and the rotation 0..=3, so this is 0..=15 in four contiguous
/// blocks of four: **each lord has four things to say and says them in turn.**
/// It is not clamped here for the same reason nothing clamps it there — a lord
/// byte outside 1..=4 is a realm
pub fn voice_variant(realm: &Realm) -> u8 {
    (realm.lord as i32 * 4 + realm.voice_rotation as i32 - 4).clamp(0, 15) as u8
}

impl Ending {
    /// Whether `Msg_DrawWindow` would take its `DAT_0053F0C4` arm for this
    /// message.
    pub fn sets_outcome(self) -> bool {
        self.category == CATEGORY_ENDING
    }
}

/// What `Score_RankRealms` leaves behind besides the ranks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Ranking {
    /// `g_rankLeader` (`0x00553D24`) — the first live entry of the sorted table,
    /// 0 when nobody is in play.
    pub leader: u8,
    /// `g_rankTrailer` — the last live entry.
    pub trailer: u8,
    /// `DAT_0056D5D8` — how many realms are in play that are **not** the local
    /// player. [`outcome_of`]'s last branch is the whole reason this is counted.
    pub opponents_remaining: u8,
    /// How many realms are in play at all, local player included. Not a global
    /// of the original's; it is here so a test can tell "one left" from "none
    /// left" without re-walking the array.
    pub realms_in_play: u8,
}

impl Ranking {
    /// The original's test, verbatim: `g_rankTrailer == g_rankLeader`.
    ///
    /// **True when nobody is in play**, because both are then 0 — and the
    /// original goes on to crown `g_realms[0]`, the slot that is never a realm.
    /// Keep the test and let [`rank_and_crown`] deal with the slot.
    pub fn sole_survivor(self) -> bool {
        self.trailer == self.leader
    }
}

/// What displaying one message does to `DAT_0053F0C4`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutcomeStep {
    /// The outcome is set to this. `Msg_DrawWindow` writes `DAT_0053F0C4 = 0`
    /// first and then may overwrite it, so [`Outcome::InPlay`] is a real answer
    /// and not "nothing happened".
    Set(Outcome),
    /// Nobody is left to fight. The original **enqueues group 225 at the local
    /// player** and leaves the outcome at zero for this message; the victory
    /// arrives when that message is displayed in its turn.
    EnqueueVictory,
}

/// The message `Msg_DrawWindow` enqueues for [`OutcomeStep::EnqueueVictory`].
pub fn victory_message(local_player: u8) -> Ending {
    Ending { group: MSG_VICTORY, from: 0, to: local_player, category: CATEGORY_ENDING, variant: 0 }
}

