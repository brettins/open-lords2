//! **The bow draw** — `Anim_DrawBowA2` (`0x0048804A`).
//!
//! ```text
//! frame = |dirc| * 13 + curve[min(swingTimer, 0x4F)] + 10
//! ```
//!
//! Three things in that line are not what we had. The stride is **13 for every
//! troop** — the handler does not read a per-troop `poses_per_facing` the way
//! `Anim_StandA2` and `Anim_StrikeA2` do,
//! an archer (N = 13) can be drawn by it at all. The pose is not `phase / 4`:
//! it is a **table lookup on `swingTimer`**, the missile reload counter. And
//! the two tables are 0x50 bytes long, so the whole reload up to eighty ticks
//! is a drawn pose, not a four-tick cadence.
//!
//! The curves are read out of `Lords2.exe` itself (`tools/oracle/tables.ps1`'s
//! method: virtual address through the PE section headers, bytes off disk),
//! not out of a decompiler listing. **[V]**

use l2_sim::Troop;

/// `g_drawBowArcher` (`0x004D9AC8`), 0x50 bytes, file offset `0xD7CC8`.
///
/// It runs **down**: the archer is at full draw (pose 12) for the first
/// twenty-four ticks after `swingTimer` resets, eases to 11, and is back to the
/// nocked pose 10 by tick 30 — a follow-through, then a long wait.
const ARCHER: [u8; 0x50] = [
    2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 1, 1, 1, 1, 1, 1, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

/// `g_drawBowCrossbow` (`0x004D9B18`), 0x50 bytes, file offset `0xD7D18`.
///
/// The other shape: a crossbow is **cranked**, 10 -> 11 -> 12 over the first
/// twelve ticks, held at 12 for fifty, and let down again at the end.
const CROSSBOW: [u8; 0x50] = [
    0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
    2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
    2, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

/// The stride `Anim_DrawBowA2` uses for every troop, and the base it adds.
pub const STRIDE: usize = 13;
pub const BASE: usize = 10;

/// The drawn pose, 0 … 2 above [`BASE`]. Troops the handler has no curve for —
/// every troop but the crossbowman (1) and the archer (5), knights included —
/// get 0, so they are drawn at `facing * 13 + 10`. That is the binary's own
/// answer and it lands outside their sheets; nothing calls the handler for
/// them, because `BattleMan_FireMissile` only reaches it with a target.
pub fn pose(troop: Troop, swing: u16) -> usize {
    let t = (swing as usize).min(0x4F);
    match troop {
        Troop::Crossbowmen => CROSSBOW[t] as usize,
        Troop::Archers => ARCHER[t] as usize,
        _ => 0,
    }
}

/// `frame = |dirc| * 13 + curve[min(swingTimer, 0x4F)] + 10`.
pub fn frame(troop: Troop, facing: u8, swing: u16) -> usize {
    (facing % 8) as usize * STRIDE + BASE + pose(troop, swing)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Both curves cover all three poses, so 10, 11 and 12 all reach the
    /// screen over one reload.
    ///
    /// **Ablation** — the old `phase / 4`, clamped to 2, reached pose 12 after
    /// eight ticks and never came off it; and with the draw restarted every
    /// tick (the state it was in) it never left pose 10 at all.
    #[test]
    fn every_bow_pose_is_reached_over_a_reload() {
        for troop in [Troop::Archers, Troop::Crossbowmen] {
            let seen: std::collections::HashSet<usize> =
                (0..80u16).map(|t| pose(troop, t)).collect();
            assert_eq!(seen.len(), 3, "{troop:?} draws {seen:?}");
        }
    }

    /// The two weapons are drawn in opposite directions.
    #[test]
    fn the_archer_eases_down_and_the_crossbow_cranks_up() {
        assert_eq!((pose(Troop::Archers, 0), pose(Troop::Archers, 40)), (2, 0));
        assert_eq!((pose(Troop::Crossbowmen, 0), pose(Troop::Crossbowmen, 40)), (0, 2));
    }

    /// The clamp is the handler's own `if (0x4F < local_c) local_c = 0x4F`.
    #[test]
    fn swing_past_the_table_clamps_to_its_last_entry() {
        for troop in [Troop::Archers, Troop::Crossbowmen] {
            assert_eq!(pose(troop, 0x4F), pose(troop, 60_000));
        }
    }

    /// Stride 13, not the troop's own — a pikeman drawn by this handler lands
    /// past his sheet,
    #[test]
    fn the_stride_is_thirteen_for_every_troop() {
        assert_eq!(frame(Troop::Pikemen, 3, 0), 3 * 13 + 10);
        assert_eq!(frame(Troop::Knights, 1, 20), 13 + 10);
    }
}
