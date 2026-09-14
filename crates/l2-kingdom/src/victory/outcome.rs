#![allow(unused_imports)]
use super::*;
use super::strength_part::*;
use super::ranking::*;
use super::tests_part::*;
use crate::county::{County, MAX_COUNTIES};
use crate::realm::{Realm, MAX_REALMS};
use crate::tables::Tables;
use crate::unit::{UnitKind, Units};
use l2_net::{Quirk, Quirks};

/// `Msg_DrawWindow`'s category-`0x0E` arm, which is the only writer of
/// `DAT_0053F0C4` during play.
///
/// The original writes the same four-way ladder twice, once in each of the
/// animated and unanimated paths, and they are identical:
///
/// ```c
/// DAT_0053f0c4 = 0;
/// if ((group == 0xe1) || (0 < DAT_0056d5d8)) {
///   if ((group == 0xe1) || (g_localPlayer != DAT_00553ee0)) {
///     if (group == 0xe1) { DAT_0053f0c4 = 10; }
///   } else { DAT_0053f0c4 = 0xb; }
/// } else { Msg_Enqueue(0, g_localPlayer, 0xe1, 0, '\x0e', 0, 0, 0); }
/// ```
///
/// Flattened, and this is the whole of "who won":
///
/// | message | opponents left | result |
/// |---|---|---|
/// | 225 *Victory!* | any | **won** |
/// | anything else | some | **lost** if it is about me, otherwise nothing |
/// | anything else | none | enqueue 225 — and the game is won when it shows |
///
/// The third row is the ordinary way a person wins: the last AI's elimination
/// raises group 194 *about that AI*, and displaying 194 with nobody left is what
/// produces the victory. The second row is the ordinary way a person loses:
/// group 224 with `from == me`.
pub fn outcome_of(
    msg: Ending,
    local_player: u8,
    ranking: Ranking,
    quirks: Quirks,
) -> OutcomeStep {
    if msg.group == MSG_VICTORY {
        return OutcomeStep::Set(Outcome::Won);
    }
    // **Switchable** — [`Quirk::MutualDestructionIsAWin`], `docs/bugs.md` B53.
    // The original tests "no opponents left" *before* "is this message about
    // me", so a local player eliminated on the same pass as the last opponent
    // is handed a victory. The fixed path asks whose defeat this is first; the
    // two tests are otherwise unchanged and in the same function.
    let mine_first = !quirks.reproduces(Quirk::MutualDestructionIsAWin);
    if mine_first && msg.from == local_player {
        return OutcomeStep::Set(Outcome::Lost);
    }
    if ranking.opponents_remaining == 0 {
        return OutcomeStep::EnqueueVictory;
    }
    if msg.from == local_player {
        OutcomeStep::Set(Outcome::Lost)
    } else {
        OutcomeStep::Set(Outcome::InPlay)
    }
}

