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
pub(crate) fn write_back(kingdom: &mut Kingdom, id: usize, survivors: [u32; 11]) {
    let Some(u) = kingdom.campaign.units.get_mut(id) else { return };
    for (slot, left) in u.troops.iter_mut().zip(survivors.iter()) {
        *slot = *left as i32;
    }
    u.mercenaries = None;
    u.men = u.troops.iter().sum();
}


