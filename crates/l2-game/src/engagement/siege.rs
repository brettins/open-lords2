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

/// **Resolve a siege assault** — [`l2_kingdom::siege::assault`]'s
/// [`Assault::Battle`](l2_kingdom::siege::Assault::Battle), fought or
/// auto-resolved.
///
/// The only differences from [`resolve`] are the three the original makes, and
/// each of them is one argument:
///
/// * the **castle level** goes to [`battle::auto_resolve`], which multiplies
///   the *defender's* strength by [`battle::CASTLE_STRENGTH_PERCENT`]. The
///   table has been in that function since the seam landed with nothing to
///   pass it; this is what passes it.
/// * `Army_PrepareForBattle` fills the four battle-only troop slots — the
/// besieger's engines and the garrison's oil — and they go into the muster
///   because the original zeroes them
///   again the instant the battle ends.
/// * `g_battleIsSiege` reaches [`battle::return_to_campaign`], where it decides
///   which half of the siege link is cleared, and reaches
///   [`battle::outcome`], where it picks four of the seven `L2.eng` group 82
///   banners.
///
/// **The `county` is the besieged one**, taken from the besieger's own
/// `besieging_county`.
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

/// **Turn phase 2, end to end** — `Turn_Tick`'s `g_turnPhase == 2` arm.
///
/// ```c
/// if (step == 1) Siege_StartPhase();
/// if (step % 100 == 2) {
///     if (Siege_TickPhase() == 0) Turn_AdvancePhase();
///     else                        Siege_LaunchAssault(g_siegeCursor);
/// }
/// ```
///
/// **Phase 2 is sieges and nothing else.** It is called *"army movement"* in
/// `docs/kingdom.md` §3.1: `Siege_StartPhase`,
/// a cursor, and the assaults it yields. Armies move in phase 4, under whoever
/// is driving that realm.
///
/// The cursor is deliberately not advanced past an army that assaulted: the
/// assault clears the siege link one way or another, so the next
/// [`l2_kingdom::siege::build_tick`] on that slot reports nothing and the
/// cursor moves on by itself. Reproduced, and it is what makes the loop
/// terminate.
///
/// `answer` decides what a human does when asked to take the field, and `seed`
/// must come from simulation state — a battle is lockstep state like any other.
/// Returns one report per assault fought, in cursor order.
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
    /// `Siege_StartPhase`: break the pairs that no longer agree and seed the
    /// cursor.
    pub fn begin(kingdom: &mut Kingdom) -> SiegePhase {
        let Kingdom { counties, campaign, .. } = kingdom;
        SiegePhase { cursor: l2_kingdom::siege::start_phase(counties, &mut campaign.units), round: 0 }
    }

    /// How many live besieging armies the phase started with — `g_siegeCount`.
    pub fn count(&self) -> u32 {
        self.cursor.count
    }

    /// The next assault, or `None` once the cursor has run off the end.
    ///
    /// The cursor only ever advances, so a loop over this cannot spin: at worst
    /// it looks at every unit slot once and yields at most one assault a slot.
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

    /// Settle what [`SiegePhase::next`] handed back, and advance the cursor
    /// past it.
    ///
/// message `0x119`, the siege is lifted, and
    /// `siege::assault` has already done it — so this answers `None`.
    ///
    /// Whatever happened, the slot must not be looked at again with the same
    /// state: the original relies on the link being gone, and an assault that
    /// was refused has had it broken too. That is the `cursor.at += 1`, and it
    /// is what makes the phase terminate.
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

    /// Whether the assault `next` handed back is one a human is in — the
    /// question [`l2_kingdom::battle::settlement`] answers, asked before the
/// battle.
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

/// **The one place the two simulations' castle records meet.**
///
/// `l2_sim::CastleDamage` and `l2_kingdom::siege::SiegeScars` are the same six
/// county fields written twice, in two crates that must not see each other —
/// `docs/plan.md`'s one-way rule. This module is the only crate that depends on
/// both, so this is the only place the conversion can live, and it is a
/// field-for-field copy so that a reader can check it at a glance.
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

/// The other direction — [`l2_kingdom::siege::scars_for_assault`]'s answer, put
/// back into a battle that is opening on the same castle.
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

