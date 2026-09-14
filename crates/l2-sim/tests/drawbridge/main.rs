//! **The garrison's drawbridge, and the two accumulators that bill the
//! repair.**
//!
//! ```text
//! cargo test -p l2-sim --test drawbridge
//! ```
//!
//! Three things the crate could not do before, all of them reachable from a
//! battle a player is watching:
//!
//! * `FUN_00496B9F` — **lower the drawbridge**. The battlefield's third button
//!   called a once-per-battle latch and nothing else; this is the routine.
//! * `FUN_0047DD86` — **fill a moat cell in**, which is the only writer of
//!   `DAT_0057A0D8` in the whole binary.
//! * the pair `DAT_0057A0D8` / `DAT_0056D648`, which is everything
//!   `Siege_RecordCastleDamage` (`0x004784CA`) bills a castle's repair from.
//!
//! Needs no game install. The castle is [`l2_sim::siege::our_castle`] — **ours,
//! not the original's**, and its own documentation says why. What is *not* ours
//! is every number asserted below: the 7 × 4 patch, the twenty-eight frames,
//! the four score arms and the two accumulators are all read out of
//! `Lords2.exe`.

mod drawbridge;
pub use drawbridge::*;
mod moat;
pub use moat::*;

use l2_sim::siege;
use l2_sim::terrain::{id, DIM};
use l2_sim::{BattleRunner, Muster, Troop};

/// A besieger against a garrison at a given castle level, both sides driven by
/// the AI so that nothing here depends on an order being issued.
fn siege_battle(level: u8, seed: u64) -> BattleRunner {
    let attacker = [
        (Troop::Peasants, 240u32),
        (Troop::Swordsmen, 80),
        (Troop::Catapults, 2),
        (Troop::BatteringRams, 1),
    ];
    let defender = [(Troop::Archers, 300u32), (Troop::Swordsmen, 40), (Troop::Oil, 3)];
    BattleRunner::deploy_siege(
        siege::our_castle(level),
        seed,
        Muster { troops: &attacker, owner: 1, human: false },
        Muster { troops: &defender, owner: 2, human: false },
        level,
    )
}

// ---------------------------------------------------------------------------
// The drawbridge — `FUN_00496B9F`
// ---------------------------------------------------------------------------

