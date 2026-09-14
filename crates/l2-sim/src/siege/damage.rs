#![allow(unused_imports)]
use super::*;
use super::castle::*;
use super::drawbridge::*;
use super::moat::*;
use super::siegetower::*;
use crate::terrain::{Battlefield, Cell, DIM};

/// A figure hits the wall — `BattleMan_Step`'s state 6 and state 14.
///
/// `standing_on` is the **surface the attacker is standing on**, not the
/// surface it is hitting, and that is the whole of how the original chooses
/// between its two accumulators. `is_ram` is `troop == BatteringRams`; state 14
/// is reachable by nothing else, which `docs/battle.md` §14.3 establishes from
/// `Cell_TryEnterEngine` returning 6 only for `troopType == 9`.
pub fn strike_wall(state: &mut SiegeState, standing_on: u8, is_ram: bool) -> WallBlow {
    let hits = if is_ram { RAM_HITS_PER_FRAME } else { WALL_HITS_PER_MAN };
    if standing_on == SURFACE_BAILEY {
        state.rampart_hits += hits;
        if state.rampart_hits >= RAMPART_HITS {
            state.rampart_hits = 0;
            state.ramparts_breached += 1;
            return WallBlow::RampartBreached;
        }
        return WallBlow::Absorbed;
    }
    if state.gate_breached {
        // The counter never resets and the breach never un-happens, so a man
        // still swinging at an open gate achieves nothing. The original keeps
        // adding to a total nothing reads again; the effect is the same.
        state.gate_hits += hits;
        return WallBlow::Absorbed;
    }
    state.gate_hits += hits;
    if state.gate_hits >= GATE_HITS {
        state.gate_breached = true;
        return WallBlow::GateBreached;
    }
    WallBlow::Absorbed
}

/// **Open the wall** — `Wall_Smash`, `FUN_0049694F` (`0x0049694F`), which both
/// wall-attack states call with a radius of **4** the moment either
/// accumulator crosses its threshold.
///
/// ```c
/// Sound_PlayFile("bathit2.wav");
/// for every cell of the (2*r+1) square around (x, y), clipped at the field edge:
///     if (flags & 0x20) { flags &= ~0x20; surface = 5; frame[+0x280] += 0x10; }
///     if (flags & 0x40) { flags  =  0;    surface = 5; frame       += 0x28; }
/// Path_BuildTerrainTemplate(); Path_BuildStepCost();
/// ```
///
/// # This is the routine that makes an assault possible
///
/// A breach is **nine cells wide**, not one. Our `strike_castle` used to open
/// the single cell the attacker had walked into, and a single-cell gap in a
/// castle wall is a funnel that a few dozen figures cannot clear: they arrive,
/// the first one blocks it, the pathfinder marks a friendly-occupied cell 998,
/// and the rest stand outside for as long as you care to run the simulation.
/// Fought at scale that is exactly the observation the branch started from —
/// *848 men outside, two garrison figures alive, 400,000 frames*.
///
/// # Three details that are the original's and not obvious
///
/// * **The elevation is not touched.** A smashed wall stays at the height the
///   builder gave it, which is [`WALL_ELEVATION`] — one — and
///   `movement::can_step_elevation` allows exactly that. It is
///   [`collapse_wall`], the catapult's routine, that flattens a cell.
/// * **A wall and a drawbridge both become [`SURFACE_BAILEY`]**, so the hole
///   joins the courtyard region.
///   That is what puts a besieger who is standing in it onto the *rampart*
/// accumulator, so a breach spreads at 5,000 hits.
///   20,000.
/// * **The wall's graphic bump lands one row south** — `frame[+0x280] += 0x10`
///   — while the drawbridge's lands on the cell itself. Reproduced, because
/// the two are not the same offset in the original and
///   believe that is an accident. [`Cell`] carries no `flags2`, so the two
///   `|= 1` writes are dropped; nothing in this engine reads that byte.
///
/// Returns how many cells were opened, which is 0 when the square held no wall
/// at all.
pub fn smash_walls(field: &mut Battlefield, x: i32, y: i32, radius: i32) -> usize {
    let mut opened = 0;
    for cy in (y - radius).max(0)..=(y + radius).min(DIM as i32 - 1) {
        for cx in (x - radius).max(0)..=(x + radius).min(DIM as i32 - 1) {
            let c = cy as usize * DIM + cx as usize;
            let flags = field.cells[c].flags;
            if flags & FLAG_WALL != 0 {
                field.cells[c].flags &= !FLAG_WALL;
                field.cells[c].surface = SURFACE_BAILEY;
                // The frame bump is on the cell one row **south**, which is
                // where the original writes it.
                if cy + 1 < DIM as i32 {
                    let s = (cy as usize + 1) * DIM + cx as usize;
                    field.cells[s].gfx = field.cells[s].gfx.wrapping_add(0x10);
                }
                opened += 1;
            }
            if field.cells[c].flags & FLAG_DRAWBRIDGE != 0 {
                field.cells[c].flags = 0;
                field.cells[c].surface = SURFACE_BAILEY;
                field.cells[c].gfx = field.cells[c].gfx.wrapping_add(0x28);
                opened += 1;
            }
        }
    }
    opened
}

/// **What a catapult shot counts against** — the class-3 gate of `Missile_Step`
/// (`0x00492C8B`), `docs/battle.md` §17.7: `elevation != 0 && surface == 4 &&
/// frame > 2`. **Damage is a count, not a flag**: the hit is added to the
/// cell's own byte `+0` and the cell collapses when that byte passes
/// [`crate::missile::WALL_DAMAGE_MAX`]. [`FLAG_WALL`] is the *mover's* byte —
/// `Cell_TryEnter` (`0x00490A44`) returns 5 for it — and says nothing about how
/// damaged a cell is; [`smash_walls`] clears it and leaves the count alone,
///
///
/// `[V]` on the original's three clauses. `[I]` on accepting [`SURFACE_WALL`]
/// beside [`SURFACE_RAMPART_WALK`]: the original's raster paints **one** masonry
/// surface 4 and tells walk from curtain by the frame, while
/// `Battlefield_BuildCastle`'s structure code 8 gives us a separate surface 8
/// and no frames at all — so `frame > 2` has no counterpart here and both
/// halves of that surface are masonry. `Siege_FindCellSurface4` (`0x00496566`),
/// which is how a catapult finds the wall, hunts surface 4.
///
/// The elevation *split* stays with the caller: at
/// [`crate::missile::WALL_TOO_HIGH`] and above the shot still lands and is not
/// counted.
pub fn shot_damages_wall(cell: &Cell) -> bool {
    cell.elevation != 0 && (cell.surface == SURFACE_RAMPART_WALK || cell.surface == SURFACE_WALL)
}

/// The radius `BattleMan_StateAttackWall` and `BattleMan_StateRamGate` both
/// pass to [`smash_walls`]. Four, so the square is 9 × 9. `[V]` — the literal
/// at all three call sites.
pub const SMASH_RADIUS: i32 = 4;

/// **A catapult brings a wall cell down** — `Wall_Collapse`, `FUN_0047DFE0`
/// (`0x0047DFE0`).
///
/// ```c
/// surface = 9; flags = 2; elevation = 0; flags2 = (flags2 & 0xE3) | 4;
/// for each of the four orthogonal neighbours still at surface 5:
///     g_siegeBreachScore++;  FUN_0048EE46(nb);  DAT_0056D648++;
/// ```
///
/// It is **not** [`smash_walls`] and leaves a different mark: surface
/// [`SURFACE_COLLAPSED`], the elevation
/// flattened to [`BREACH_ELEVATION`], and `flags` set to 2 outright.
/// having a bit cleared — so a collapsed cell carries neither [`FLAG_WALL`] nor
/// [`FLAG_DRAWBRIDGE`] and is passable.
///
/// Returns the number of neighbours billed, which is both the breach score
/// gained and the wall damage the county pays for. `docs/bugs.md` B69: those
/// two are the *same* count, written in one statement per neighbour.
pub fn collapse_wall(field: &mut Battlefield, state: &mut SiegeState, cell: usize) -> i32 {
    {
        let c = &mut field.cells[cell];
        c.surface = SURFACE_COLLAPSED;
        c.flags = 2;
        c.elevation = BREACH_ELEVATION;
    }
    let mut billed = 0;
    for n in orthogonal_neighbours(cell) {
        if field.cells[n].surface == SURFACE_BAILEY {
            billed += 1;
        }
    }
    state.wall_damage = state.wall_damage.saturating_add(billed as u16);
    billed
}

