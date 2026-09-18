#![allow(unused_imports)]
use super::*;
use super::campaign::*;
use crate::county::{County, MAX_COUNTIES};
use crate::math::pct;
use crate::realm::{Realm, MAX_REALMS};
use crate::tables::Tables;
use crate::unit::{ArmyNames, Unit, Units, TROOP_TYPES};

/// `FUN_004A6A30` — which of the three ways this battle is settled.
///
/// Note what is **not** here: no distance from the view, no army-size cutoff,
/// no separate "quick battle" toggle. The mid-battle *"Autocalc battle?"*
/// button is a fourth entry and re-runs [`auto_resolve`] from the counts as
/// they stand; `[V]` — one function, three callers,
/// all reading the same two expressions.
pub fn settlement(units: &Units, a: usize, b: usize, fight_humans_only_byte: u8) -> Settlement {
    let human = |id: usize| units.get(id).is_some_and(|u| u.owner_is_human);
    let (a_human, b_human) = (human(a), human(b));
    if !a_human && !b_human {
        return Settlement::Silently;
    }
    if fight_humans_only_byte == 0 && !(a_human && b_human) {
        return Settlement::Reported;
    }
    Settlement::Prompt
}


/// >
/// > `g_battleLoser` (`0x0057C924`) holds the **winner**. Three independent
/// > sites agree against the name: the autocalc assigns it the side whose
/// > troops it then *keeps*; `Battle_ReturnToCampaign`'s
/// > `g_battleLoser == g_battleArmyA` branch hands the county to A and destroys
/// > B; and `FUN_00478419` maps `g_localPlayer == g_units[g_battleLoser].owner`
/// > to `L2.eng` group 82's *"won"* string. Implemented on the name, you
/// > destroy the winner and hand the county to the corpse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Verdict {
    pub attacker: usize,
    pub defender: usize,
    pub attacker_won: bool,
    pub survival_percent: Option<i32>,
}

impl Verdict {
    pub fn a_won(attacker: usize, defender: usize) -> Verdict {
        Verdict { attacker, defender, attacker_won: true, survival_percent: None }
    }

    pub fn b_won(attacker: usize, defender: usize) -> Verdict {
        Verdict { attacker, defender, attacker_won: false, survival_percent: None }
    }

    pub fn winner(&self) -> usize {
        if self.attacker_won {
            self.attacker
        } else {
            self.defender
        }
    }

    pub fn loser(&self) -> usize {
        if self.attacker_won {
            self.defender
        } else {
            self.attacker
        }
    }
}

/// `FUN_004AAD07` (`0x004AAD07`) — **the autocalc**, the whole of a battle
/// nobody watches.
pub fn auto_resolve(
    units: &mut Units,
    attacker: usize,
    defender: usize,
    castle_level: Option<u8>,
) -> Option<Verdict> {
    let strength_a = units.get(attacker)?.strength_score();
    let mut strength_b = units.get(defender)?.strength_score();
    if let Some(level) = castle_level {
        let bonus = CASTLE_STRENGTH_PERCENT
            [(level as usize).min(CASTLE_STRENGTH_PERCENT.len() - 1)];
        strength_b = pct(strength_b, bonus);
    }

    let attacker_won = strength_a >= strength_b;
    let (bigger, smaller) =
        if attacker_won { (strength_a, strength_b) } else { (strength_b, strength_a) };
    let ratio = if smaller == 0 { 0 } else { bigger * 100 / smaller };
    let percent = survival_percent(ratio);

    let mut verdict = if attacker_won {
        Verdict::a_won(attacker, defender)
    } else {
        Verdict::b_won(attacker, defender)
    };
    verdict.survival_percent = Some(percent);

    if let Some(w) = units.get_mut(verdict.winner()) {
        apply_survival(w, percent);
    }
    if let Some(l) = units.get_mut(verdict.loser()) {
        l.troops = [0; TROOP_TYPES];
        l.mercenaries = None;
        l.men = 0;
    }
    Some(verdict)
}

/// Keep `percent` of every count, then rebuild the total from what is left —
/// the original recomputes `+0x168` by summing it, so the
/// total and the counts cannot drift apart however the rounding falls.
fn apply_survival(unit: &mut Unit, percent: i32) {
    for t in 0..TROOP_TYPES {
        unit.troops[t] = pct(unit.troops[t], percent);
    }
    if let Some(band) = unit.mercenaries.as_mut() {
        band.men = pct(band.men as i32, percent).clamp(0, u8::MAX as i32) as u8;
    }
    unit.men = unit.troops.iter().sum::<i32>() + unit.mercenary_men();
}


/// `FUN_00478419` — which banner this battle draws for `local_player`.
///
/// `winner_owner` is the realm of the side that held the field. A local player
/// who owns neither army gets [`Outcome::Bystander`]; the original reaches that
/// pair by a different route (`DAT_005533D0 == 0` never brings screen `0x2B`
/// up at all) and the pair exists for it. `[D]` on the seventh, `[V]` on the
/// six.
pub fn outcome(
    verdict: Verdict,
    is_siege: bool,
    local_player: u8,
    winner_owner: u8,
    loser_owner: u8,
) -> Outcome {
    if local_player != winner_owner && local_player != loser_owner {
        return Outcome::Bystander;
    }
    let local_won = local_player == winner_owner;
    match (is_siege, local_won, verdict.attacker_won) {
        (false, true, _) => Outcome::Won,
        (false, false, _) => Outcome::Lost,
        (true, true, true) => Outcome::SiegeWon,
        (true, true, false) => Outcome::SiegeLifted,
        (true, false, true) => Outcome::CastleLost,
        (true, false, false) => Outcome::SiegeLost,
    }
}


