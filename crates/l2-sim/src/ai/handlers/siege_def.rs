#![allow(unused_imports)]
use super::*;
use super::field::*;
use super::siege_att::*;
use super::*;
use super::world::*;
use super::tests::*;
use crate::figure::{Figure, State};
use crate::troop::Troop;
use crate::unit::{chebyshev, pct_of, Units, MAX_UNITS, REFORM_INTERVAL};
use l2_net::Pcg32;

/// The opening every "real" siege defender shares: above the sortie threshold
/// it lowers the drawbridge. Returns true when the sortie is on.
///
/// The threshold is 260, so a garrison sallies out only when it believes it is
/// 3.6× the attacker's strength. **[V]** on the number; the drawbridge routine
/// demonstrably paints 7 × 4 cells and bumps both progress counters — whether
/// that is a drawbridge, a sally port or a siege ramp is **[I]**.
fn sortie(w: &mut World, _cur: usize) -> bool {
    if w.ai.strength_advantage > SORTIE_THRESHOLD {
        if !w.ai.drawbridge_down {
            w.ai.drawbridge_down = true;
            w.ai.approach_score += 4;
            w.ai.breach_score += 4;
        }
        true
    } else {
        false
    }
}

/// `UnitOrder_SiegeDefMissile` (`0x0048E097`) — nine wall slots, every other
/// think, while fewer than three attackers stand on the wall.
///
/// **Its sortie branch does nothing at all**, and that is faithful, not a stub:
/// the original discards the `Enemy_NearestUnit` result and then calls
/// `Order_HalfwayToUnit` on the unit's *own* index, so both axis separations
/// are zero and it returns without writing anything. It reads as a missing
/// assignment. Reproduced, because "fixing" it would change observable
/// behaviour — and it is unreachable in practice anyway, behind a 260 % gate.
pub(crate) fn siege_def_missile(w: &mut World, cur: usize) {
    if !w.may_think(cur, THINK_INTERVAL, true) {
        w.ai.record(cur, Action::NoThink);
        return;
    }
    w.ai.record(cur, Action::DoNothing);
    if sortie(w, cur) {
        let _discarded = w.nearest_enemy_unit(cur, 80, 1);
        w.halfway_to_unit(cur, cur);
    } else if !w.field.castle_layout_flag || w.ai.approach_score == 0 {
        if w.ai.attackers_on_wall < 3 {
            if w.units.get(cur).orders % 2 == 0 && w.units.get(cur).firing == 0 {
                let slot = w.ai.wall_slot_cursor;
                w.to_wall_slot(cur, 1, slot);
                w.ai.wall_slot_cursor = if slot >= 8 { 0 } else { slot + 1 };
            }
        } else {
            w.to_castle_objective(cur, 0);
        }
    } else {
        w.to_castle_objective(cur, 1);
    }
    w.advance_script(cur);
}

/// `UnitOrder_SiegeDefWallMissileA` (`0x0048E234`) — category 9, the first
/// missile unit a garrison raises.
///
/// It does **nothing but count `orders` up**, so that unit holds whatever wall
/// slot it was deployed on for the entire battle. Not a stub: the original body
/// is the think gate and the increment, and nothing else.
pub(crate) fn siege_def_wall_missile_a(w: &mut World, cur: usize) {
    if !w.may_think(cur, THINK_INTERVAL, true) {
        w.ai.record(cur, Action::NoThink);
        return;
    }
    w.ai.record(cur, Action::DoNothing);
    w.advance_script(cur);
}

/// `UnitOrder_SiegeDefWallMissileB` (`0x0048E2C8`) — category 10, the second.
pub(crate) fn siege_def_wall_missile_b(w: &mut World, cur: usize) {
    if !w.may_think(cur, THINK_INTERVAL, true) {
        w.ai.record(cur, Action::NoThink);
        return;
    }
    w.ai.record(cur, Action::DoNothing);
    if !w.field.castle_layout_flag || w.ai.approach_score < 4 {
        if w.ai.attackers_on_wall < 3 {
            w.to_wall_near_target(cur, true);
        } else {
            w.to_wall_below_keep(cur);
        }
    } else {
        w.to_wall_below_keep(cur);
    }
    w.advance_script(cur);
}

/// `UnitOrder_SiegeDefFoot` (`0x0048E39C`) — thinks every **100** frames and
/// has **no** in-melee gate, so it keeps re-deciding while fighting.
///
/// Its `Enemy_NearestUnit(unit, 10, 0)` result is discarded, the same missing
/// assignment as in `SiegeDefMissile`. Reproduced for the same reason.
pub(crate) fn siege_def_foot(w: &mut World, cur: usize) {
    if !w.may_think(cur, THINK_INTERVAL_FAST, false) {
        w.ai.record(cur, Action::NoThink);
        return;
    }
    w.ai.record(cur, Action::DoNothing);
    if w.live_attacker(cur).is_none() {
        let _discarded = w.nearest_enemy_unit(cur, 10, 0);
    }
    if sortie(w, cur) {
        w.charge_nearest(cur);
    } else {
        w.to_castle_objective(cur, 1);
    }
    w.advance_script(cur);
}

/// `UnitOrder_SiegeDefMelee` (`0x0048E4CC`) — thinks every 100 frames.
///
/// Reserves a defence post, moves onto an attacking unit that has reached the
/// wall, and otherwise rotates four inner slots every fifteenth think —
/// abandoning all of it for the castle objective once four attackers are up.
pub(crate) fn siege_def_melee(w: &mut World, cur: usize) {
    if !w.may_think(cur, THINK_INTERVAL_FAST, true) {
        w.ai.record(cur, Action::NoThink);
        return;
    }
    w.ai.record(cur, Action::DoNothing);
    let attacker = w.units.get(cur).last_attacker as usize;
    // 0 = no live attacker, 1 = attacker on low ground, 2 = attacker on the
    // wall. The threshold is the attacker unit's own cell surface.
    let mut attacker_state = 0u8;
    if w.live_attacker(cur).is_some() {
        let (ax, ay) = w.unit_pos(attacker);
        attacker_state = if w.field.surface_at(ax as i32, ay as i32).unwrap_or(0) < 4 {
            1
        } else {
            2
        };
    }
    let post = w.claim_defence_post(cur);

    if sortie(w, cur) {
        w.charge_nearest(cur);
    } else if attacker_state == 2 {
        w.onto_unit(cur, attacker);
    } else if !w.field.castle_layout_flag || w.ai.approach_score == 0 {
        if w.ai.attackers_on_wall >= 6 {
            w.to_castle_objective(cur, 1);
        } else if w.ai.attackers_on_wall >= 4 {
            w.to_wall_slot(cur, 2, 0);
        } else if post != 0 && attacker_state == 1 {
            w.to_surface5_near(cur, post);
        } else if w.units.get(cur).orders % 15 == 0 {
            if post == 0 {
                let slot = w.ai.inner_slot_cursor;
                w.to_wall_slot(cur, 0, slot);
                w.ai.inner_slot_cursor = if slot >= 3 { 0 } else { slot + 1 };
            } else {
                w.to_cell(cur, post);
            }
        }
    } else {
        w.to_castle_objective(cur, 1);
    }
    w.advance_script(cur);
}

/// `UnitOrder_SiegeDefKnight` (`0x0048E774`) — the most passive of the
/// defenders: wall-slot group 2 every twentieth think until a single attacker
/// reaches the wall.
pub(crate) fn siege_def_knight(w: &mut World, cur: usize) {
    if !w.may_think(cur, THINK_INTERVAL, true) {
        w.ai.record(cur, Action::NoThink);
        return;
    }
    w.ai.record(cur, Action::DoNothing);
    if sortie(w, cur) {
        w.charge_nearest(cur);
    } else if !w.field.castle_layout_flag || w.ai.approach_score == 0 {
        if w.ai.attackers_on_wall < 1 {
            if w.units.get(cur).orders % 20 == 0 {
                w.to_wall_slot(cur, 2, 0);
            }
        } else {
            w.to_castle_objective(cur, 1);
        }
    } else {
        w.to_castle_objective(cur, 1);
    }
    w.advance_script(cur);
}

/// `UnitOrder_SiegeDefOil` (`0x0048E8B8`) — the most specific handler in the
/// set. It moves only if its unit is at elevation ≥ 2, and then only to the
/// densest cluster of enemies within six cells (needing three) or four (needing
/// two, on the rampart proper).
// Three of the fall-back branches end in the same order, as the original does.
#[allow(clippy::if_same_then_else)]
pub(crate) fn siege_def_oil(w: &mut World, cur: usize) {
    if !w.may_think(cur, THINK_INTERVAL, true) {
        w.ai.record(cur, Action::NoThink);
        return;
    }
    w.ai.record(cur, Action::DoNothing);
    if let Some((x, y)) = w.oil_find_pour_target(cur) {
        w.set_target(cur, x, y);
        w.ai.record(cur, Action::PourOil);
    } else if w.ai.attackers_on_wall >= 2 {
        w.to_wall_below_keep(cur);
    } else if w.field.castle_layout_flag && w.ai.approach_score != 0 {
        w.to_wall_below_keep(cur);
    } else if !w.oil_is_on_high_wall(cur) {
        if w.units.get(cur).orders % 3 == 0 && w.units.get(cur).hit_memory != 0 {
            w.to_wall_below_keep(cur);
        } else {
            w.to_wall_near_target(cur, false);
        }
    }
    w.advance_script(cur);
}

// ---------------------------------------------------------------------------
// The three dispatch tables
// ---------------------------------------------------------------------------


