#![allow(unused_imports)]
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
pub(super) fn order_none(w: &mut World, cur: usize) {
    // Not even the think timer. The body is `return`.
    w.ai.record(cur, Action::NoThink);
}

/// `UnitOrder_FieldMissile` (`0x0048A9D2`) — category 1, archers and
/// crossbowmen.
///
/// `nearest` is the whole 80-cell field; §2.3's melee units look 8 or 9 cells.
/// The last branch is a **second, independent decision** taken after the first,
/// and it exists only on the cautious side: an AI archer unit that thinks it is
/// winning never backs away.
pub(super) fn field_missile(w: &mut World, cur: usize) {
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
                // Order_DoNothing.
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
///
/// **The cautious branch is where the only inter-unit coordination in the game
/// lives.** A melee unit that is losing men backs two cells away from its
/// attacker *and* publishes that attacker's position; the field missile handler
/// reads the request and shifts two cells toward it. That fire-support call is
/// the only message any unit sends to any other.
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
                    // The march slot is chosen with the **kingdom's season
                    // byte**. In even seasons alternate units take alternate
                    // slots; in odd seasons they all take the same one. This
                    // looks like a free coin flip that happened to be lying
                    // around, and nothing establishes intent.
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
                // "We have run out of archers, so go in."
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
        // Charge radius is tiny, and a worn-down enemy unit of fewer than three
        // figures is invisible to the melee AI entirely.
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

/// `UnitOrder_FieldFoot` — category 2, peasants and pikemen.
pub(super) fn field_foot(w: &mut World, cur: usize) {
    field_foot_or_melee(w, cur, 2, 10, 8, 5, 2, 1);
}

/// `UnitOrder_FieldMelee` — categories 3 **and** 4; both slots hold this one
/// function.
///
/// Note the rally waypoint: `FieldFoot` uses waypoint 2 and `FieldMelee`
/// waypoint 1. `docs/battle-ai.md` §2.3's pseudocode gives both as 2.
pub(super) fn field_melee(w: &mut World, cur: usize) {
    field_foot_or_melee(w, cur, 4, 13, 9, 8, 1, 2);
}

/// `UnitOrder_SiegeAttMissile` (`0x0048D16E`) — siege attacker, category 1.
pub(super) fn siege_att_missile(w: &mut World, cur: usize) {
    if !w.may_think(cur, THINK_INTERVAL, true) {
        w.ai.record(cur, Action::NoThink);
        return;
    }
    w.ai.record(cur, Action::DoNothing);
    w.ai.rot_missile3 = (w.ai.rot_missile3 + 1) % 3;

    if w.ai.approach_score < 3 || w.ai.breach_score == 0 {
        let orders = w.units.get(cur).orders;
        if orders == 0 {
            w.to_field_corner(cur, 1);
        } else if orders % 15 == 0 {
            w.to_castle_approach(cur, 1);
        } else if orders > 10 {
            if w.ai.approach_score < 10 && w.ai.men_total - w.ai.men_knight <= w.ai.men_missile {
                if w.ai.rot_missile16 < 11 {
                    w.to_breach_or_staging(cur);
                } else {
                    w.to_siege_staging(cur);
                }
                w.ai.rot_missile16 = (w.ai.rot_missile16 + 1) % 16;
            } else if !w.shoot_at_unit(cur, 0) {
                w.to_castle_approach(cur, 1);
            }
        }
    } else if w.ai.men_missile < w.ai.men_total {
        if w.ai.tick_missile < 4 || w.ai.rot_missile3 != 0 {
            if w.ai.rot_missile3 == 0 {
                w.ai.tick_missile += 1;
            } else if !w.shoot_at_unit(cur, 0) {
                w.to_nearest_wall_cell(cur);
            }
        } else if w.ai.ramparts_breached < 1 {
            w.ai.rot_missile_objective += 1;
            if w.ai.rot_missile_objective < 6 {
                w.to_castle_objective(cur, 0);
            } else {
                w.to_nearest_wall_cell(cur);
            }
        } else {
            w.to_castle_objective(cur, 0);
        }
    } else {
        w.to_castle_objective(cur, 0);
    }
    w.advance_script(cur);
}

/// `UnitOrder_SiegeAttFoot` (`0x0048D412`) — the longest of the order scripts.
///
/// The ladder is 8, 18, 25, 31, 40, 51, 60, 71, alternating two staging
/// positions, with a jump that sets `orders` to **100** outright when the
/// castle layout flag is set — skipping the rest of the approach script.
pub(super) fn siege_att_foot(w: &mut World, cur: usize) {
    if !w.may_think(cur, THINK_INTERVAL, true) {
        w.ai.record(cur, Action::NoThink);
        return;
    }
    w.ai.record(cur, Action::DoNothing);
    w.ai.rot_foot3 = (w.ai.rot_foot3 + 1) % 3;

    if w.ai.approach_score < 3 || w.ai.breach_score == 0 {
        let orders = w.units.get(cur).orders;
        if orders < 8 {
            w.to_field_corner(cur, 0);
        } else if orders < 0x12 {
            w.to_siege_staging(cur);
        } else if orders < 0x19 {
            if w.field.castle_layout_flag {
                w.units.get_mut(cur).orders = 100;
            }
            w.to_breach_or_staging(cur);
        } else if orders < 0x1f {
            w.to_siege_staging(cur);
        } else if orders < 0x28 {
            w.to_breach_or_staging(cur);
        } else if orders < 0x33 {
            w.to_siege_staging(cur);
        } else if orders < 0x3c {
            w.to_breach_or_staging(cur);
        } else if orders < 0x47 {
            w.to_siege_staging(cur);
        } else {
            w.to_breach_or_staging(cur);
        }
    } else if w.ai.strength_advantage < SIEGE_CHARGE_ADVANTAGE {
        if w.ai.tick_foot < 7 || w.ai.rot_foot3 != 0 {
            if w.ai.rot_foot3 == 0 {
                w.ai.tick_foot += 1;
            } else if w.ai.tick_foot < 6 {
                w.to_siege_staging(cur);
            }
        } else if w.ai.breach_score >= 1 || w.ai.siege_engine_count == 0 {
            w.to_castle_objective(cur, 0);
        }
    } else {
        w.charge_nearest(cur);
    }
    w.advance_script(cur);
}

/// `UnitOrder_SiegeAttMelee` (`0x0048D6FC`) — the same ladder at 15, 25, 30,
/// 41, 50, 61, 70, 81, gated on `orders > 5`, plus a six-phase rotation.
pub(super) fn siege_att_melee(w: &mut World, cur: usize) {
    if !w.may_think(cur, THINK_INTERVAL, true) {
        w.ai.record(cur, Action::NoThink);
        return;
    }
    w.ai.record(cur, Action::DoNothing);
    w.ai.rot_melee6 = (w.ai.rot_melee6 + 1) % 6;

    if w.ai.approach_score < 3 || w.ai.breach_score == 0 {
        let orders = w.units.get(cur).orders;
        if orders > 5 {
            if orders < 0xf {
                w.to_field_corner(cur, 0);
            } else if orders < 0x19 {
                w.to_siege_staging(cur);
            } else if orders < 0x1e {
                if w.field.castle_layout_flag {
                    w.units.get_mut(cur).orders = 100;
                }
                w.to_breach_or_staging(cur);
            } else if orders < 0x29 {
                w.to_siege_staging(cur);
            } else if orders < 0x32 {
                w.to_breach_or_staging(cur);
            } else if orders < 0x3d {
                w.to_siege_staging(cur);
            } else if orders < 0x46 {
                w.to_breach_or_staging(cur);
            } else if orders < 0x51 {
                w.to_siege_staging(cur);
            } else {
                w.to_breach_or_staging(cur);
            }
        }
    } else if w.ai.rot_melee6 == 0 || w.ai.rot_melee6 == 3 {
        if w.ai.breach_score >= 1 || w.ai.siege_engine_count == 0 {
            w.to_castle_objective(cur, 0);
        }
    } else if w.ai.rot_melee6 == 5 {
        w.hold_position(cur);
    }
    w.advance_script(cur);
}

/// `UnitOrder_SiegeAttKnight` (`0x0048D9CE`).
///
/// # It is also **the only thing in the binary that ends a battle by giving up**
///
/// ```c
/// if ((g_aiMenTotal <= g_aiMenKnight) && (g_siegeBreachScore == 0)) {
///     g_battleWithdrawal = 1;
///     DAT_005656f8 = g_battleUnits[g_curBattleUnit].owner;
/// }
/// ```
///
/// — three statements at the top of the think, above the movement ladder and
/// **not** in an `else`, so the unit raises the flag and then goes on issuing
/// its order for the frame. `g_aiMenTotal` and `g_aiMenKnight` are
/// [`Ai::count_men`]'s totals over the AI's own living troops of type 0…6, so
/// `total <= knights` is *"every man I have left is a knight"* — an all-cavalry
/// besieger in front of an unbreached wall, which is a siege it cannot win.
///
/// `Battle_CheckOutcome` tests [`Ai::withdrawal`] **before** either men
/// counter, so this outranks annihilation; and `Battle_ReturnToCampaign` then
/// charges the loser [`Army_WithdrawCasualties`](../../l2_kingdom/battle/fn.withdraw_casualties.html)
/// — half of every troop line — before deciding whether fifty men are left.
///
/// **This clause was missing, and its absence was load-bearing.** Nothing else
/// raises the flag, so with it absent `End::Withdrawal` could not arise in a
/// played game, the whole withdrawal half of the campaign seam was unreachable,
/// and the campaign's own missing `Army_WithdrawCasualties` could not be
/// noticed. `docs/decisions.md` C71.
pub(super) fn siege_att_knight(w: &mut World, cur: usize) {
    if !w.may_think(cur, THINK_INTERVAL, true) {
        w.ai.record(cur, Action::NoThink);
        return;
    }
    w.ai.record(cur, Action::DoNothing);
    if w.ai.men_total <= w.ai.men_knight && w.ai.breach_score == 0 {
        w.ai.withdrawal = Some(w.units.get(cur).side);
    }
    w.ai.rot_knight3 = (w.ai.rot_knight3 + 1) % 3;

    if w.ai.approach_score < 3 || w.ai.breach_score == 0 {
        let orders = w.units.get(cur).orders;
        if orders < 10 {
            w.to_field_corner(cur, 3);
        } else if orders < 0x1f {
            w.to_castle_approach(cur, 3);
        }
    } else if w.ai.rot_knight3 == 0 && (w.ai.breach_score >= 1 || w.ai.siege_engine_count == 0) {
        w.to_castle_objective(cur, 0);
    }
    w.advance_script(cur);
}

/// `UnitOrder_SiegeAttCatapult` (`0x0048DB84`) — **no in-melee gate**, so it
/// keeps thinking while engaged.
///
/// The search radius is 20 cells, which is *exactly* the catapult's firing
/// range in `g_missileStats` (160 eighths of a cell). Two unrelated constants
/// in the binary agreeing is the strongest check available here. **[V]**
///
/// Note it `return`s on the wall-found path *before* the `orders` increment, so
/// a catapult doing its job stops advancing its script.
// Two ladder rungs issue the same order from different conditions, as the
// original does. Collapsing them would lose the correspondence.
#[allow(clippy::if_same_then_else)]
pub(super) fn siege_att_catapult(w: &mut World, cur: usize) {
    if !w.may_think(cur, THINK_INTERVAL, false) {
        w.ai.record(cur, Action::NoThink);
        return;
    }
    w.ai.record(cur, Action::DoNothing);
    let orders = w.units.get(cur).orders;
    if orders > 3 {
        if orders < 0xd {
            w.to_field_corner(cur, 5);
        } else if orders < 0x15 {
            w.to_castle_approach(cur, 5);
        } else if orders % 100 == 0 {
            w.to_castle_approach(cur, 5);
        } else if orders > 0x1e {
            let (first, last) = {
                let u = w.units.get(cur);
                (u.first as usize, u.last as usize)
            };
            for f in first..=last.min(w.figures.len().saturating_sub(1)) {
                if !w.figures[f].is_alive() || w.figures[f].unit as usize != cur {
                    continue;
                }
                let Some(&(x, y)) = w.positions.get(f) else { continue };
                match w.nearest_surface(x as i32, y as i32, 20, 4, false) {
                    Some((cx, cy)) => w.to_cell(cur, cy as usize * DIM + cx as usize),
                    None => w.to_castle_approach(cur, 5),
                }
                return;
            }
        }
    }
    w.advance_script(cur);
}

/// `UnitOrder_SiegeAttTower` (`0x0048DDC7`) — no in-melee gate either.
///
/// Once `approach_score` passes 6 after think 60, or passes 10 at any time, it
/// widens a search from radius **10 to 30** for a surface-4 cell. `L2.eng`
/// 214/2 is the other half of the catapult's sentence: *"Battering rams and
/// siege towers must be moved right up to the castle wall to work."* **[V]**
pub(super) fn siege_att_tower(w: &mut World, cur: usize) {
    if !w.may_think(cur, THINK_INTERVAL, false) {
        w.ai.record(cur, Action::NoThink);
        return;
    }
    w.ai.record(cur, Action::DoNothing);
    let orders = w.units.get(cur).orders;
    if orders < 0xb {
        w.to_field_corner(cur, 3);
    } else if (orders > 0x3c && w.ai.approach_score > 6) || w.ai.approach_score > 10 {
        let (first, last) = {
            let u = w.units.get(cur);
            (u.first as usize, u.last as usize)
        };
        for f in first..=last.min(w.figures.len().saturating_sub(1)) {
            if !w.figures[f].is_alive() || w.figures[f].unit as usize != cur {
                continue;
            }
            let Some(&(x, y)) = w.positions.get(f) else { continue };
            for radius in 10..=30 {
                if let Some((cx, cy)) = w.nearest_surface(x as i32, y as i32, radius, 4, false) {
                    w.to_cell(cur, cy as usize * DIM + cx as usize);
                    return;
                }
            }
            w.to_castle_approach(cur, 3);
            return;
        }
    }
    w.advance_script(cur);
}

/// `UnitOrder_SiegeAttRam` (`0x0048DFBB`) — ignores `orders` entirely and picks
/// between its stored second destination and a castle approach point from three
/// flags. No in-melee gate.
pub(super) fn siege_att_ram(w: &mut World, cur: usize) {
    if !w.may_think(cur, THINK_INTERVAL, false) {
        w.ai.record(cur, Action::NoThink);
        return;
    }
    if !w.field.castle_layout_flag && w.ai.approach_score < 8 {
        w.to_second_target(cur);
    } else if !w.ai.drawbridge_down {
        w.to_castle_approach(cur, 2);
    } else {
        w.to_second_target(cur);
    }
    w.advance_script(cur);
}

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
pub(super) fn siege_def_missile(w: &mut World, cur: usize) {
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
pub(super) fn siege_def_wall_missile_a(w: &mut World, cur: usize) {
    if !w.may_think(cur, THINK_INTERVAL, true) {
        w.ai.record(cur, Action::NoThink);
        return;
    }
    w.ai.record(cur, Action::DoNothing);
    w.advance_script(cur);
}

/// `UnitOrder_SiegeDefWallMissileB` (`0x0048E2C8`) — category 10, the second.
pub(super) fn siege_def_wall_missile_b(w: &mut World, cur: usize) {
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
pub(super) fn siege_def_foot(w: &mut World, cur: usize) {
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
pub(super) fn siege_def_melee(w: &mut World, cur: usize) {
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
pub(super) fn siege_def_knight(w: &mut World, cur: usize) {
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
pub(super) fn siege_def_oil(w: &mut World, cur: usize) {
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

