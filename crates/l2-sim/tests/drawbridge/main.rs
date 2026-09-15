//! * `FUN_00496B9F` — **lower the drawbridge**. The battlefield's third button
//!   called a once-per-battle latch and nothing else; this is the routine.
//!
//! * `FUN_0047DD86` — **fill a moat cell in**, which is the only writer of
//!   `DAT_0057A0D8` in the whole binary.
//!
//! * the pair `DAT_0057A0D8` / `DAT_0056D648`, which is everything
//!   `Siege_RecordCastleDamage` (`0x004784CA`) bills a castle's repair from.

mod drawbridge;
pub use drawbridge::*;
mod moat;
pub use moat::*;

use l2_sim::siege;
use l2_sim::terrain::{id, DIM};
use l2_sim::{BattleRunner, Muster, Troop};

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

