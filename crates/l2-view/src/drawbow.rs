//! **The bow draw** — `Anim_DrawBowA2` (`0x0048804A`).
//!
//! The curves are read out of `Lords2.exe` itself (`tools/oracle/tables.ps1`'s
//! method: virtual address through the PE section headers, bytes off disk),
//! not out of a decompiler listing. **[V]**

use l2_sim::Troop;

/// `g_drawBowArcher` (`0x004D9AC8`), 0x50 bytes, file offset `0xD7CC8`.
const ARCHER: [u8; 0x50] = [
    2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 1, 1, 1, 1, 1, 1, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

/// `g_drawBowCrossbow` (`0x004D9B18`), 0x50 bytes, file offset `0xD7D18`.
const CROSSBOW: [u8; 0x50] = [
    0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
    2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
    2, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

pub const STRIDE: usize = 13;
pub const BASE: usize = 10;

pub fn pose(troop: Troop, swing: u16) -> usize {
    let t = (swing as usize).min(0x4F);
    match troop {
        Troop::Crossbowmen => CROSSBOW[t] as usize,
        Troop::Archers => ARCHER[t] as usize,
        _ => 0,
    }
}

pub fn frame(troop: Troop, facing: u8, swing: u16) -> usize {
    (facing % 8) as usize * STRIDE + BASE + pose(troop, swing)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_bow_pose_is_reached_over_a_reload() {
        for troop in [Troop::Archers, Troop::Crossbowmen] {
            let seen: std::collections::HashSet<usize> =
                (0..80u16).map(|t| pose(troop, t)).collect();
            assert_eq!(seen.len(), 3, "{troop:?} draws {seen:?}");
        }
    }

    #[test]
    fn the_archer_eases_down_and_the_crossbow_cranks_up() {
        assert_eq!((pose(Troop::Archers, 0), pose(Troop::Archers, 40)), (2, 0));
        assert_eq!((pose(Troop::Crossbowmen, 0), pose(Troop::Crossbowmen, 40)), (0, 2));
    }

    #[test]
    fn swing_past_the_table_clamps_to_its_last_entry() {
        for troop in [Troop::Archers, Troop::Crossbowmen] {
            assert_eq!(pose(troop, 0x4F), pose(troop, 60_000));
        }
    }

    #[test]
    fn the_stride_is_thirteen_for_every_troop() {
        assert_eq!(frame(Troop::Pikemen, 3, 0), 3 * 13 + 10);
        assert_eq!(frame(Troop::Knights, 1, 20), 13 + 10);
    }
}
