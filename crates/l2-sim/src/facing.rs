//! Which way a figure is pointing, and the step that produced it.
//!
//! This is simulation state, not decoration. `Dir_FromDelta` is what the
//! original's mover calls to turn a target into the next cell to try
//! (`BattleMan_Step`, `docs/battle.md` §7), what the melee search uses to face
//! an opponent, and what a missile's direction byte is set from. The renderer
//! then *reads* the facing to pick a sprite row — but it never decides one.
//!
//! Kept in `l2-sim` for that reason: a facing decides where a figure walks, and
//! two lockstep peers that disagree about a facing disagree about the battle.
//!
//! # Determinism
//!
//! Pure integer arithmetic over a fixed table, no allocation, no ordering
//! dependence (`docs/netcode.md`).

/// The eight facings, in the original's numbering. Confirmed independently by
/// `docs/battle.md` §2.1 and by the renderer's sub-cell offset table, whose
/// signs only make sense under this order.
pub const FACINGS: usize = 8;

/// The eight facing deltas, in facing order. Used to turn a step into a facing
/// and back.
pub const FACING_DELTA: [(i32, i32); 8] =
    [(0, -1), (1, -1), (1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1)];

/// The facing that best matches a movement delta. `None` when the delta is
/// zero, which the original represents as facing 8, "same cell".
pub fn facing_from_delta(dx: i32, dy: i32) -> Option<u8> {
    if dx == 0 && dy == 0 {
        return None;
    }
    let sx = dx.signum();
    let sy = dy.signum();
    // Reduce to one of the eight compass directions by dominance: an axis is
    // dropped when it is less than half the other, which is the same partition
    // the original's `Dir_FromDelta` branch structure produces.
    let (ux, uy) = if dx.abs() > 2 * dy.abs() {
        (sx, 0)
    } else if dy.abs() > 2 * dx.abs() {
        (0, sy)
    } else {
        (sx, sy)
    };
    FACING_DELTA.iter().position(|&d| d == (ux, uy)).map(|i| i as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn facings_and_deltas_are_inverse() {
        for (i, (dx, dy)) in FACING_DELTA.iter().enumerate() {
            assert_eq!(facing_from_delta(*dx, *dy), Some(i as u8), "facing {i}");
        }
        assert_eq!(facing_from_delta(0, 0), None);
        // Long runs still resolve to the nearest compass point.
        assert_eq!(facing_from_delta(10, 1), Some(2), "mostly east");
        assert_eq!(facing_from_delta(10, 9), Some(3), "south-east");
        assert_eq!(facing_from_delta(-1, -10), Some(0), "mostly north");
    }
}
