#![allow(unused_imports)]
use super::*;
use super::battle_runner::*;
use super::fight_session::*;
use super::*;
use super::report::*;
use super::siege::*;
use super::tests::*;
use l2_kingdom::battle::{self, Aftermath, Settlement, Verdict};
use l2_kingdom::conquest::Attack;
use l2_kingdom::kingdom::Kingdom;
use l2_kingdom::unit::TROOP_TYPES;
use l2_sim::runner::{blank_field, BattleRunner, Muster};
use l2_sim::{End, Troop, SIDE_A, SIDE_B};

/// **Resolve a battle the campaign has just produced, end to end.**
///
/// `attack` is what [`l2_kingdom::conquest::attack_county`] returned; anything
/// but [`Attack::Battle`] yields `None`, because nothing was fought.
///
/// `answer` is consulted only when [`battle::settlement`] returns
/// [`Settlement::Prompt`] — the other two settlements never reach a screen and
/// the player is not asked. `seed` seeds the battle AI's generator and must be
/// derived from simulation state, never from a clock: two lockstep peers fight
/// the same battle or they are not playing the same game.
///
/// `g_optFightHumansOnly` comes from `kingdom.options`. It was a parameter here
/// while the save and the battle seam were being written in parallel branches —
/// adding a field to [`l2_kingdom::kingdom::Options`] means adding it to the
/// save and to the lockstep checksum, and neither branch could do that to the
/// other's file. Both have landed, so it lives where it belongs.
pub fn resolve(
    kingdom: &mut Kingdom,
    attack: Attack,
    county: u8,
    answer: Answer,
    seed: u64,
) -> Option<BattleReport> {
    let Attack::Battle { attacker, defender } = attack else { return None };
    resolve_battle(kingdom, attacker, defender, county, None, answer, seed, None)
}

/// **Settle a battle the player has already watched.**
///
/// [`resolve`] runs the whole battle between two statements; this takes one that
/// a [`crate::battlefield::LiveBattle`] has been stepping a tick at a time and
/// does the rest — the write-back, the verdict and the aftermath — from exactly
/// where it stopped. Everything after the fight is the same code, which is the
/// point: a played battle and a headless one differ in *who supplied the ticks*
/// and in nothing else.
pub fn resolve_fought(
    kingdom: &mut Kingdom,
    attacker: usize,
    defender: usize,
    county: u8,
    castle_level: Option<u8>,
    seed: u64,
    runner: BattleRunner,
) -> Option<BattleReport> {
    resolve_battle(
        kingdom,
        attacker,
        defender,
        county,
        castle_level,
        Answer::TakeTheField,
        seed,
        Some(runner),
    )
}

/// `FUN_0047F474` for one side: zero all eleven counts, refill the seven the
/// campaign carries from the surviving figures, and rebuild the total by
/// summing.
fn write_back(kingdom: &mut Kingdom, id: usize, survivors: [u32; 11]) {
    let Some(u) = kingdom.campaign.units.get_mut(id) else { return };
    for (slot, left) in u.troops.iter_mut().zip(survivors.iter()) {
        *slot = *left as i32;
    }
    // The band cannot be told from the line it was folded into, so it is
// released. See this module's header.
    u.mercenaries = None;
    u.men = u.troops.iter().sum();
}


