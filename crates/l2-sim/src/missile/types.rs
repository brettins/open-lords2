#![allow(unused_imports)]
use super::*;

use combat::*;
use crate::facing::facing_from_delta;
use crate::figure::Figure;
use crate::troop::Troop;

/// **One missile in flight** — the original's `g_missiles` record
/// (`0x0057A100`, stride `0x4C`), reduced to the fields that decide anything.
///
/// The dropped fields are all presentation or dead: the sprite sheet pointer,
/// the sprite frame and base, the per-cell draw list link, the burnt-surface
/// save slot, and `+0x32`, which the original writes and never reads.
///
/// **`owner` is the free-slot marker**
/// figure: zero means the slot is empty, and [`Missiles::alloc`] finds the
/// lowest such slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Missile {
    /// Realm that fired it. **Zero means a free slot.** The hit test compares
    /// this against a figure's *owner*, not its side
    /// passes through a friendly body without being consumed.
    pub owner: u8,
    /// 1 bow, 2 crossbow, 3 catapult, [`CLASS_DEBRIS`] what a catapult shot
    /// becomes when it strikes a wall.
    pub class: u8,
    /// **The firing figure**, not the target.
    /// record at all — see the module header.
    pub shooter: u16,
    /// Position, in [`SUB_CELL`]ths of a cell.
    pub x: i16,
    pub y: i16,
    /// The impact point, **frozen at launch**. A target that moves is not
    /// followed.
    pub target_x: i16,
    pub target_y: i16,
    /// The cell the missile is over, `(pos + 8) / 32`. The `+ 8` is the
    /// original's: the cell index flips a quarter of a cell late.
    pub cell_x: i16,
    pub cell_y: i16,
    /// Bresenham's remaining counts, in sub-cell units, decremented as the line
    /// is walked. When both reach zero the line is spent and the missile
    /// coasts.
    pub dx: i32,
    pub dy: i32,
    pub err: i32,
    /// 1 when x is the major axis, 2 when y is.
    pub major_axis: u8,
    /// Launch direction, 0…7 — what the missile coasts along once its line is
    /// spent.
    pub dir: u8,
    /// Cell elevation under the shooter. Ground more than one level above this
    /// blocks the shot.
    pub launch_elevation: u8,
    /// Ticks spent blocked. Seeded per shooter
    /// on the same frame.
    pub blocked_ticks: u8,
    /// Sub-steps this tick — [`SUB_STEPS`] normally, 1 for debris.
    pub sub_steps: i8,
    /// Ticks flown, against [`Missile::range_ticks`].
    pub ticks_flown: i16,
    /// The tick budget, which is the range in eighths of a cell.
    pub range_ticks: i16,
    /// Sticky: high ground or a siege engine is in the way.
    pub blocked: bool,
    /// Non-zero **suppresses every impact test**, and the countdown to
    /// retirement. This is what makes one hit per missile structural.
    pub ttl: i16,
    /// Damage, band-scaled and frozen at launch. Elevation, armour and the
    /// engine caps are applied at impact by [`resolve_power`].
    pub power: u16,
    /// Record `+0x3E` — **the surface a fire burnt over**, which a
    /// [`CLASS_FIRE`] record writes back when its countdown reaches 2.
    /// `Missile_UpdateAll`'s class-5 arm, `[V]`. Zero on every other class.
    pub saved_surface: u8,
    /// Record `+0x44`, non-zero — **a fire arrow**, and the original's own
    /// two values, not a flag. `[V]` on both writers.
    ///
    /// * **1** — `BattleMan_FireMissile` (`0x00483337`) sets it for a unit of
    /// side 0 that is not a human's while more than three of a human's
    ///   figures stand in woodland. That arrow lights **any** woodland cell it
    ///   crosses.
    /// * **a cell byte offset** — `BattleMan_StateCloseToAttack`
    ///   (`0x00484BF9`) copies the unit's `targetCell` (`+0x30`) onto every
    ///   arrow it looses. **The player's fire arrow**: `Missile_Step`'s first
    ///   test lights only the one cell `+0x44` names.
    ///
    /// The offset is `(y * 80 + x) * 8` — [`crate::fire::cell_byte_offset`].
    pub fire_arrow: i32,
}

impl Missile {
    pub fn is_live(&self) -> bool {
        self.owner != 0
    }

    /// `Missile_SetupLine` (`0x00493CB9`) — `|dx|`, `|dy|`, `2 * min − max`,
    /// and the octant snap.
    ///
    /// The snap moves the *direction* only, never the position: a near-vertical
    /// diagonal is drawn as vertical. It matters here because `dir` is what the
    /// missile coasts along once its line is spent.
    pub(crate) fn setup_line(&mut self) {
        let dx = (self.target_x - self.x).unsigned_abs() as i32;
        let dy = (self.target_y - self.y).unsigned_abs() as i32;
        self.dx = dx;
        self.dy = dy;
        // `2 * min − max`, and **0 when the two are equal** — the perfect
        // diagonal, where either major axis walks the same line.
        match dx.cmp(&dy) {
            core::cmp::Ordering::Greater => {
                self.major_axis = 1;
                self.err = 2 * dy - dx;
            }
            core::cmp::Ordering::Less => {
                self.major_axis = 2;
                self.err = 2 * dx - dy;
            }
            core::cmp::Ordering::Equal => {
                self.major_axis = 1;
                self.err = 0;
            }
        }
        // **The octant snap is already done.** `Missile_SetupLine` pulls a
        // diagonal `dir` to the nearer cardinal when one axis is under half the
        // other — and that is the same partition
        // [`crate::facing::facing_from_delta`] applies, which is where `dir`
        // came from. Applying it twice would change nothing; applying it here
        // as well would only invite somebody to change one copy.
    }

    /// `Missile_StepError` (`0x00493B61`) — one Bresenham error update, over the
    /// **remaining** counts and one decrement of
    /// the major axis.
    ///
    /// Using the remaining counts is the original's
    /// shrink together, so the slope it re-derives each step is the same slope.
    /// **[I]** on whether the decrement precedes or follows the error update —
    /// the path is identical either way, and nothing else reads the counts.
    fn step_error(&mut self) {
        let (major, minor) =
            if self.major_axis == 2 { (self.dy, self.dx) } else { (self.dx, self.dy) };
        if self.err < 0 {
            self.err += 2 * minor;
        } else {
            self.err += 2 * (minor - major);
        }
        if self.major_axis == 2 {
            self.dy -= 1;
        } else {
            self.dx -= 1;
        }
    }

    /// One unit toward the frozen impact point, on one axis.
    fn step_toward_x(&mut self) {
        match self.x.cmp(&self.target_x) {
            core::cmp::Ordering::Less => self.x += 1,
            core::cmp::Ordering::Greater => self.x -= 1,
            core::cmp::Ordering::Equal => {}
        }
    }

    fn step_toward_y(&mut self) {
        match self.y.cmp(&self.target_y) {
            core::cmp::Ordering::Less => self.y += 1,
            core::cmp::Ordering::Greater => self.y -= 1,
            core::cmp::Ordering::Equal => {}
        }
    }

    /// **The overshoot.** `FUN_00494265`: once the line is spent the missile
    /// carries on one unit a sub-step along its launch direction, until range,
    /// the map edge or somebody else stops it.
    ///
    /// This is the behaviour that makes a miss dangerous, and it is why an
    /// arrow that passes its target keeps travelling instead of vanishing.
    pub fn step_octant(&mut self) {
        let (dx, dy) = crate::facing::FACING_DELTA[(self.dir & 7) as usize];
        self.x += dx as i16;
        self.y += dy as i16;
    }

    /// `Missile_OffMap` — the battlefield bound again, `0 … 79` in either axis.
    pub(crate) fn off_map(&self) -> bool {
        self.cell_x < 0
            || self.cell_y < 0
            || self.cell_x >= crate::terrain::DIM as i16
            || self.cell_y >= crate::terrain::DIM as i16
    }

    /// Recompute the cell from the position. The `+ 8` is the original's, and
    /// it means the cell index changes a quarter of a cell after the geometric
    /// boundary.
    pub(crate) fn resync_cell(&mut self) {
        self.cell_x = (self.x + 8) / SUB_CELL;
        self.cell_y = (self.y + 8) / SUB_CELL;
    }

    /// One sub-step of the line, or of the coast past its end.
    pub(crate) fn sub_step(&mut self) {
        if self.dx + self.dy < 1 {
            self.step_octant();
        } else {
            self.step_error();
            if self.major_axis == 2 {
                self.step_toward_y();
                if self.err >= 0 {
                    self.dx -= 1;
                    self.step_toward_x();
                }
            } else {
                self.step_toward_x();
                if self.err >= 0 {
                    self.dy -= 1;
                    self.step_toward_y();
                }
            }
        }
        self.resync_cell();
    }
}

