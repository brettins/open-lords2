

mod outcome;
mod deploy;
mod ai_runs;
mod missiles;

mod cues;

use super::*;

fn small_battle() -> BattleRunner {
    BattleRunner::deploy(
        blank_field(),
        &[(Troop::Swordsmen, 6), (Troop::Archers, 4)],
        &[(Troop::Pikemen, 6), (Troop::Peasants, 4)],
    )
}

// --- the outcome test, `FUN_00477DFC` --------------------------------

