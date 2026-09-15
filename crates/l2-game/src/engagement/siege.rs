#![allow(unused_imports)]
use super::*;
use super::report::*;
use super::fight::*;
use super::tests::*;
use l2_kingdom::battle::{self, Aftermath, Settlement, Verdict};
use l2_kingdom::conquest::Attack;
use l2_kingdom::kingdom::Kingdom;
use l2_kingdom::unit::TROOP_TYPES;
use l2_sim::runner::{blank_field, BattleRunner, Muster};
use l2_sim::{End, Troop, SIDE_A, SIDE_B};

/// * `g_battleIsSiege` reaches [`battle::return_to_campaign`], where it decides
///   which half of the siege link is cleared, and reaches
///   [`battle::outcome`], where it picks four of the seven `L2.eng` group 82
///   banners.
pub fn resolve_siege(
    kingdom: &mut Kingdom,
    assault: l2_kingdom::siege::Assault,
    answer: Answer,
    seed: u64,
) -> Option<BattleReport> {
    let l2_kingdom::siege::Assault::Battle { attacker, defender, castle_level } = assault else {
        return None;
    };
    let county = kingdom.campaign.units.get(attacker)?.besieging_county;
    resolve_battle(kingdom, attacker, defender, county, Some(castle_level), answer, seed, None)
}

pub fn run_siege_phase(kingdom: &mut Kingdom, answer: Answer, seed: u64) -> Vec<BattleReport> {
    let mut phase = SiegePhase::begin(kingdom);
    let mut reports = Vec::new();
    while let Some(assault) = phase.next(kingdom) {
        if let Some(report) = phase.settle(kingdom, assault, answer, seed) {
            reports.push(report);
        }
    }
    reports
}

impl SiegePhase {
    pub fn begin(kingdom: &mut Kingdom) -> SiegePhase {
        let Kingdom { counties, campaign, .. } = kingdom;
        SiegePhase { cursor: l2_kingdom::siege::start_phase(counties, &mut campaign.units), round: 0 }
    }

    pub fn count(&self) -> u32 {
        self.cursor.count
    }

    pub fn next(&mut self, kingdom: &mut Kingdom) -> Option<l2_kingdom::siege::Assault> {
        if self.round > l2_kingdom::MAX_UNITS {
            return None;
        }
        let army = {
            let Kingdom { campaign, .. } = kingdom;
            l2_kingdom::siege::tick_phase(&mut self.cursor, &mut campaign.units)?
        };
        let Kingdom { counties, campaign, .. } = kingdom;
        Some(l2_kingdom::siege::assault(counties, &mut campaign.units, army))
    }

    pub fn settle(
        &mut self,
        kingdom: &mut Kingdom,
        assault: l2_kingdom::siege::Assault,
        answer: Answer,
        seed: u64,
    ) -> Option<BattleReport> {
        let report = resolve_siege(kingdom, assault, answer, seed.wrapping_add(self.round as u64));
        self.cursor.at += 1;
        self.round += 1;
        report
    }

    pub fn settlement(
        kingdom: &Kingdom,
        assault: l2_kingdom::siege::Assault,
    ) -> Option<(usize, usize, Settlement)> {
        let l2_kingdom::siege::Assault::Battle { attacker, defender, .. } = assault else {
            return None;
        };
        let s = battle::settlement(
            &kingdom.campaign.units,
            attacker,
            defender,
            kingdom.options.fight_humans_only_byte,
        );
        Some((attacker, defender, s))
    }
}

pub(super) fn to_scars(d: l2_sim::CastleDamage) -> l2_kingdom::siege::SiegeScars {
    l2_kingdom::siege::SiegeScars {
        moat_filled: d.moat_filled,
        wall_damage: d.wall_damage,
        breach_score: d.breach_score,
        approach_score: d.approach_score,
        ramparts_breached: d.ramparts_breached,
        gate_open: d.gate_open,
    }
}

pub(super) fn from_scars(s: l2_kingdom::siege::SiegeScars) -> l2_sim::CastleDamage {
    l2_sim::CastleDamage {
        moat_filled: s.moat_filled,
        wall_damage: s.wall_damage,
        breach_score: s.breach_score,
        approach_score: s.approach_score,
        ramparts_breached: s.ramparts_breached,
        gate_open: s.gate_open,
    }
}

