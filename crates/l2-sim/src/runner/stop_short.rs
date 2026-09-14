//! The two destination helpers `BattleUnit_Order` (`0x00479E90`) runs on a
//! missile unit's order before it commits the move: `Order_StopShortOfTarget`
//! (`0x00497437`) and `Dest_FindReachableNear` (`0x0048A7D9`). Both leave their
//! answer in `g_foundTileX`/`g_foundTileY`, which the caller reads back into
//! its own `x`/`y`. C231 left them aside with the fire arrow.

use super::*;

/// `Order_StopShortOfTarget` (`0x00497437`) — **a missile unit halts at firing
/// range instead of closing.**
///
/// `range` is the unit's *shortest* missile range in whole cells: the original
/// takes `min(range >> 3)` over the unit's live missile figures, and
/// `g_missileStats` makes that 15 for a bow, 8 for a crossbow and 20 for a
/// catapult ([`crate::missile::MissileStats::range`]). It keeps `range - 3`.
///
/// The two axes are pulled back independently, and **an axis already inside
/// range keeps the unit's own coordinate, not the clicked cell's** — the
/// function seeds `g_foundTileX`/`g_foundTileY` from `(x, y)`, which the one
/// caller passes as the unit's `mapX`/`mapY`. So a bowman told to walk two
/// cells sideways at a target dead ahead does not move at all. `[V]` — 36
/// lines, one call to `Dist_Manhattan` (`0x00404EAD`), whose `g_absDx`/`g_absDy`
/// are the two absolute deltas below.
pub fn stop_short_of_target(from: (i32, i32), target: (i32, i32), range: i32) -> (i32, i32) {
    let keep = range - 3;
    let (abs_dx, abs_dy) = ((from.0 - target.0).abs(), (from.1 - target.1).abs());
    let mut found = from;
    if keep < abs_dx {
        found.0 = if target.0 < from.0 {
            from.0 - (abs_dx - keep)
        } else {
            target.0 - keep
        };
    }
    if keep < abs_dy {
        found.1 = if target.1 < from.1 {
            from.1 - (abs_dy - keep)
        } else {
            target.1 - keep
        };
    }
    found
}

impl BattleRunner {
    /// `Dest_FindReachableNear` (`0x0048A7D9`) — **is there ground near the
    /// pulled-back destination that this unit could stand on?**
    ///
    /// Squares of radius 0…19 around `at`, each scanned whole, for an interior
    /// cell (1…78 on both axes) that `Formation_SlotIsUsable` (`0x0048A672`)
    /// accepts against the **source** cell's surface and elevation. Because
    /// every square contains the last, the answer is just *any* usable interior
    /// cell within Chebyshev 19 of `at`; only the scan order differs.
    ///
    /// > **It cannot move the destination
    /// > away.** Both arms of `BattleUnit_Order` call it as
    /// > `Dest_FindReachableNear(mapX, mapY, g_foundTileX, g_foundTileY, unit)`
    /// > and then `x = g_foundTileX; y = g_foundTileY`
    /// > success arm writes `g_foundTileX = x; g_foundTileY = y` — the
    /// > parameters it was handed. Its failure arm writes neither. So the
    /// > destination is `Order_StopShortOfTarget`'s either way, the `int` it
    /// > returns is never read
    /// > `[V]` — the decompilation of both functions; nothing between them
    /// > touches those two globals. Ported for the ladder's shape and kept
    /// > callable
    ///
    /// The siege arm of `Formation_SlotIsUsable` — a side-0 unit refused an
    /// empty cell of surface under 4 — is still unported, in this caller and in
    /// [`Self::slot_is_usable`]; it could only change the discarded answer.
    pub fn dest_find_reachable_near(&self, from: (i32, i32), at: (i32, i32), unit: usize) -> bool {
        let src = self.field.at(
            from.0.clamp(0, DIM as i32 - 1) as usize,
            from.1.clamp(0, DIM as i32 - 1) as usize,
        );
        let (surface, elevation) = (src.surface, src.elevation);
        for radius in 0..20i32 {
            let mut x0 = at.0 - radius;
            let mut y0 = at.1 - radius;
            let mut rows = radius * 2 + 1;
            // The original clamps the width off the unadjusted span and only
            // then clamps the height, so the two are computed in this order.
            let cols = if x0 < 0 {
                let c = rows + x0;
                x0 = 0;
                c
            } else if x0 + rows > DIM as i32 {
                DIM as i32 - x0
            } else {
                rows
            };
            if y0 < 0 {
                rows += y0;
                y0 = 0;
            } else if y0 + rows > DIM as i32 {
                rows = DIM as i32 - y0;
            }
            for y in y0..y0 + rows {
                for x in x0..x0 + cols {
                    if x > 0
                        && x < DIM as i32 - 1
                        && y > 0
                        && y < DIM as i32 - 1
                        && self.slot_is_usable(unit, x, y, surface, elevation)
                    {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// The destination a missile unit's order actually gets: both helpers in the
    /// order `BattleUnit_Order` runs them, for the two arms that reach them.
    pub(super) fn pull_back_to_range(&self, unit: usize, at: (i32, i32)) -> (i32, i32) {
        let Some(range) = self.unit_missile_range(unit) else {
            return at;
        };
        let from = {
            let u = self.units.get(unit);
            (u.x as i32, u.y as i32)
        };
        let found = stop_short_of_target(from, at, range);
        // The answer is discarded, as the original discards it.
        let _reachable = self.dest_find_reachable_near(from, found, unit);
        found
    }

    /// `min(range >> 3)` over the unit's live missile figures — the original's
    /// `local_20`, seeded at 1000 and left there when the unit has none, which
    /// is the `None` here. Its figure loop also zeroes every figure's
    /// `movStraff`.
    pub(super) fn unit_missile_range(&self, unit: usize) -> Option<i32> {
        self.members(unit)
            .into_iter()
            .filter_map(|m| match WEAPON_CLASS[self.fighters[m].troop.index()] {
                1 => Some(crate::WeaponClass::Bow),
                2 => Some(crate::WeaponClass::Crossbow),
                3 => Some(crate::WeaponClass::Catapult),
                _ => None,
            })
            .map(|c| c.stats().range as i32)
            .min()
    }
}
