#![allow(unused_imports)]
use super::*;
use super::field::*;
use super::siege_def::*;
use super::*;
use super::world::*;
use super::tests::*;
use crate::figure::{Figure, State};
use crate::troop::Troop;
use crate::unit::{chebyshev, pct_of, Units, MAX_UNITS, REFORM_INTERVAL};
use l2_net::Pcg32;

/// `UnitOrder_SiegeAttMissile` (`0x0048D16E`) — siege attacker, category 1.
pub(crate) fn siege_att_missile(w: &mut World, cur: usize) {
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
pub(crate) fn siege_att_foot(w: &mut World, cur: usize) {
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
pub(crate) fn siege_att_melee(w: &mut World, cur: usize) {
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
pub(crate) fn siege_att_knight(w: &mut World, cur: usize) {
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
pub(crate) fn siege_att_catapult(w: &mut World, cur: usize) {
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
pub(crate) fn siege_att_tower(w: &mut World, cur: usize) {
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
pub(crate) fn siege_att_ram(w: &mut World, cur: usize) {
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

