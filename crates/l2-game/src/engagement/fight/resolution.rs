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

/// `Battle_WriteBackCasualties` (`0x0047F474`) for one side: `menTotal`,
/// `mercMen` and the eleven counts zeroed; each figure's men go to `menTotal`
/// and to its troop slot, or to `mercMen` when its `mercenary` flag is set.
/// Band id and `mercTroop` are not touched.
pub(crate) fn write_back(kingdom: &mut Kingdom, id: usize, survivors: [u32; 11], band: u32) {
    let Some(u) = kingdom.campaign.units.get_mut(id) else { return };
    for (slot, left) in u.troops.iter_mut().zip(survivors.iter()) {
        *slot = *left as i32;
    }
    // `mercMen = (char)men + mercMen` — a byte, `00470000.c:6637`.
    if let Some(m) = u.mercenaries.as_mut() {
        m.men = band as u8;
    }
    // `menTotal += men` for every figure, band or not — `00470000.c:6630`.
    u.men = u.troops.iter().sum::<i32>() + band as i32;
}


