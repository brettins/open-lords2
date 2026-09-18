#![allow(unused_imports)]
use super::*;
use super::siege_att::*;
use super::siege_def::*;
use super::*;
use super::world::*;
use super::tests::*;
use crate::figure::{Figure, State};
use crate::troop::Troop;
use crate::unit::{chebyshev, pct_of, Units, MAX_UNITS, REFORM_INTERVAL};
use l2_net::Pcg32;

/// `UnitOrder_None` (`0x0048A9C7`): a bare `return`, filling **seven** of the
/// twenty-five slots.
///
/// Which seven is not arbitrary, and it is the strongest cross-check in the
/// whole document: the siege **defender**'s catapult, tower and ram slots are
/// empty, and `g_raiseOrderSiege` independently says a garrison never holds
/// those three troop types. Two unrelated structures agreeing. **[V]**
pub(crate) fn order_none(w: &mut World, cur: usize) {
    w.ai.record(cur, Action::NoThink);
}

/// `UnitOrder_FieldMissile` (`0x0048A9D2`) — category 1, archers and
/// crossbowmen.
pub(crate) fn field_missile(w: &mut World, cur: usize) {
    if !w.may_think(cur, THINK_INTERVAL, true) {
        w.ai.record(cur, Action::NoThink);
        return;
    }
    w.ai.record(cur, Action::DoNothing);
    let attacker = w.units.get(cur).last_attacker as usize;
    let nearest = w.nearest_enemy_unit(cur, 80, 1);

    if w.ai.strength_advantage > AGGRESSION_THRESHOLD {
        if w.units.get(cur).hit_memory != 0 && w.units.owner_of(nearest) != 0 {
            w.shoot_at_unit(cur, nearest);
        } else {
            let orders = w.units.get(cur).orders;
            if orders < 10 {
            } else if orders < 0x12 {
                w.halfway_to_unit(cur, nearest);
            } else if orders < 0x15 {
                w.hold_position(cur);
            } else if w.units.owner_of(nearest) != 0 {
                w.halfway_to_unit(cur, nearest);
            }
        }
    } else {
        if w.ai.commit_counter == 0 {
            if w.units.get(cur).hit_memory != 0 && w.units.owner_of(attacker) != 0 {
                w.shoot_at_unit(cur, attacker);
            } else if w.ai.rally_request {
                w.step_toward_rally_point(cur);
            } else if w.units.get(cur).orders % 4 == 0 {
                w.to_rally_waypoint(cur, 0);
            }
        } else {
            w.halfway_to_unit(cur, nearest);
        }
        let u = *w.units.get(cur);
        if u.times_hit > 10
            && w.units.owner_of(attacker) != 0
            && u.firing == 0
            && !u.withdrawing
        {
            w.step_away_from_unit(cur, attacker);
        }
    }
    w.advance_script(cur);
}

/// The shape `UnitOrder_FieldFoot` (`0x0048ACD2`) and `UnitOrder_FieldMelee`
/// (`0x0048B02B`) share. They differ only in four constants.
#[allow(clippy::too_many_arguments)]
fn field_foot_or_melee(
    w: &mut World,
    cur: usize,
    silent_until: i16,
    march_until: i16,
    charge_radius: i32,
    rally_every: i16,
    rally_waypoint: usize,
    withdrawal_limit: u8,
) {
    if !w.may_think(cur, THINK_INTERVAL, true) {
        w.ai.record(cur, Action::NoThink);
        return;
    }
    w.ai.record(cur, Action::DoNothing);
    let attacker = w.units.get(cur).last_attacker as usize;
    let has_attacker =
        w.units.get(cur).hit_memory != 0 && w.units.owner_of(attacker) != 0;

    if w.ai.strength_advantage > AGGRESSION_THRESHOLD {
        if has_attacker {
            w.onto_unit(cur, attacker);
        } else {
            let orders = w.units.get(cur).orders;
            if orders < march_until {
                if orders >= silent_until {
                    let slot = if w.ai.season & 1 == 0 {
                        (cur & 1) + w.ai.approach_lane
                    } else {
                        w.ai.approach_lane
                    };
                    w.to_enemy_end(cur, slot);
                }
            } else {
                w.charge_nearest(cur);
            }
        }
    } else {
        if has_attacker {
            w.ai.engagement_count += 1;
            if w.ai.engagement_count > w.ai.engagement_budget {
                w.ai.commit_counter += 3;
            } else if w.ai.men_missile < w.ai.men_total / 8 {
                w.ai.commit_counter += 20;
            } else {
                w.ai.rally_request = true;
                let (ax, ay) = w.unit_pos(attacker);
                w.ai.rally_x = ax as i32;
                w.ai.rally_y = ay as i32;
                w.step_away_from_unit(cur, attacker);
            }
        } else if w.ai.commit_counter != 0 {
            w.ai.commit_counter -= 1;
        }
        if w.ai.commit_counter < 1 {
            w.units.get_mut(cur).halted = false;
        }
        let nearest = w.nearest_enemy_unit(cur, charge_radius, 3);
        if w.ai.commit_counter != 0 || nearest != 0 {
            w.charge_nearest(cur);
        } else if w.units.get(cur).orders % rally_every == 0
            && w.units.get(cur).withdrawals < withdrawal_limit
        {
            w.to_rally_waypoint(cur, rally_waypoint);
        }
    }
    w.advance_script(cur);
}

pub(crate) fn field_foot(w: &mut World, cur: usize) {
    field_foot_or_melee(w, cur, 2, 10, 8, 5, 2, 1);
}

pub(crate) fn field_melee(w: &mut World, cur: usize) {
    field_foot_or_melee(w, cur, 4, 13, 9, 8, 1, 2);
}

