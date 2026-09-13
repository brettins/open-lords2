//! **Siege engines** — the four troop types that are not men
//! painters that put them on a battlefield. `docs/battle.md` §13.11,
//! `docs/decisions.md` `C202`.
//!
//! A catapult, a siege tower, a ram and a pot of boiling oil are figures like
//! any other: they sit in `g_battleMen`, they walk, they are drawn by
//! `BattleFigure_Draw` (`0x004BDC31`). What is different is everything about
//! *which* picture.
//!
//! # One sheet for all four
//!
//! `FUN_00480F8B` (`0x00480F8B`) assigns every figure its sheet pointer at the
//! start of a battle. Troop types 0…6 take one of the thirty-six
//! `a2<colour>_<troop>.pl8` files; **troop types 7, 8, 9 and 10 all take
//! `DAT_00553250`, which is slot 8 of the battle asset table — `engine.pl8` —
//! in both banks.** So a siege engine has no side colour
//! branches that say so are the same assignment written eight times. **[V]**
//!
//! A catapult's *arm* is a second sprite from a second pair of files,
//! `catarm1.pl8` and `catarm2.pl8`, slots 9 and 10. **[V]**
//!
//! # The frame map, and it closes
//!
//! Three functions write a siege engine's frame — `FUN_00488436` (reached from
//! `Anim_WalkA2`), `FUN_00488793` (from `Anim_StrikeA2`, `Anim_StandA2`,
//! `Anim_DyingA2`, `Anim_DrawBowA2`) and `FUN_0048895E` (from
//! `Anim_CollapseA2` and from the engine state handlers 12…15). The first two
//! differ only in whether a tower's polar facing is recomputed; the third is
//! the one that changes the picture.
//!
//! | `engine.pl8` | what | from |
//! |---|---|---|
//! | 0 | the ram, every facing | `frame = 0` |
//! | 1 … 4 | the siege tower | `polarDirc >> 1 + 1` |
//! | 5 … 12 | the catapult's carriage | `dirc + 5` |
//! | 13 … 21 | the ram's lower half while it beats on a gate | `FUN_004BEAB9`, `+0x11 + 0x0D` |
//! | 22 … 30 | the ram's upper half | `FUN_004BEAB9`, `+0x11 + 0x16` |
//! | 31 … 34 | a docked tower's stair, over the men | `FUN_004BD759` |
//! | 35 … 40 | a pot of oil, idling | `(animPhase >> 3) + 0x23` |
//! | 42 … 45 | a pot of oil, pouring | `(polarDirc >> 1) + 0x2A` |
//!
//! `Engine.pl8` holds exactly **46** frames
//! 45. `Catarm1.pl8` and `Catarm2.pl8` hold exactly **20** each, and four
//! facings of five arm poses is twenty. Frame 41 is the only one nothing
//! reaches. `tests/install.rs` asserts the counts against the install.
//!
//! # What our simulation has instead of a state byte
//!
//! The original forks on `g_battleMen[].state`; `l2_sim` has
//! [`l2_sim::Motion`], four values, and this module maps onto it. Two of those
//! mappings are inferences and both are named where they are made:
//! [`frame`]'s `Motion::Dying` arm for a spent pot, and
//! [`ram_strips`]'s `Motion::Attacking` for state 14.

use l2_sim::{Motion, Troop};

/// Slot 8 of the battle asset table (`0x004DA550`). One file, four troop
/// types, both banks.
pub const ENGINE_SHEET: &str = "Engine.pl8";

/// Slots 9 and 10. `FUN_004BE7BE` picks the first for `dirc < 4`
/// second otherwise, which is what makes four facings of artwork cover eight.
pub const ARM_SHEETS: [&str; 2] = ["Catarm1.pl8", "Catarm2.pl8"];

/// Poses per facing in one `Catarm` file — `horseFrame = dirc * 5`, wrapped by
/// `-0x14` for the far half. Four facings of five is the file's twenty frames.
pub const ARM_POSES: usize = 5;

/// `g_horseWalkCycle` (`0x004D9C08`), forty bytes, indexed by `animPhase >> 2`.
/// `FUN_0048895E` reuses the horse's curve for the catapult arm: a slow
/// 0 → 1 → 2 → 3 arc over the eighty ticks after the shot. **[V]** from the
/// bytes; entries 28 … 39 hold 4, which the catapult's eighty-tick window
/// never reaches.
pub const ARM_CYCLE: [u8; 40] = [
    0, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 3, 3, 3, 3, 3, 3, 1, 1, 1, 1, 1, 1, 4, 4, 4, 4,
    4, 4, 4, 4, 4, 4, 4, 4,
];

/// How long `FUN_0048895E` swings the arm for. `BattleMan_StateEngineFire`
/// (`0x004843BC`) counts `swingTimer` to 100, looses, and wraps at 179 — so
/// `animPhase` runs 1 … 80 after the shot and `>> 2` reaches 20.
pub const ARM_SWING_TICKS: u16 = 80;

/// The pot's idle loop: `animPhase + 1`, wrapped past `0x2F`, then `>> 3`.
/// Six pictures over forty-eight ticks.
const OIL_IDLE_PERIOD: u16 = 0x30;
const OIL_IDLE_BASE: usize = 0x23;
/// The pour: `(polarDirc >> 1) + 0x2A`, four pictures, one per orthogonal.
const OIL_POUR_BASE: usize = 0x2A;

/// The ram's beam counter, `FUN_0048895E`: `animPhase - 1`, clamped back up to
/// `0x8F` when it underflows, then `>> 4`. Nine pictures, counting **down**.
const RAM_PERIOD: u16 = 0x90;
/// `FUN_004BEAB9` draws `+0x11 + 0x16` above the cell and `+0x11 + 0x0D`
/// below it.
const RAM_STRIP_UPPER: usize = 0x16;
const RAM_STRIP_LOWER: usize = 0x0D;

/// **The two extra pixel offsets `BattleFigure_Draw` adds to an engine's
/// body**, after the sprite centring every figure gets: `+0x0C` for a ram,
/// `+8` for a pot of oil, on y only. Nothing for a catapult or a tower. **[V]**
pub fn body_y_nudge(troop: Troop) -> i32 {
    match troop {
        Troop::BatteringRams => 0x0C,
        Troop::Oil => 8,
        _ => 0,
    }
}

/// The `engine.pl8` frame for a siege engine's body, or `None` for a troop
/// that is not one.
///
/// `polar` is figure `+0x168`, the debug panel's `polar dirc` — 0, 2, 4 or 6.
/// `phase` is the figure's own animation counter, the original's `+0x0E`.
///
/// **The oil fork is the one inference here.** `FUN_0047A814` puts a pot that
/// has poured into **state 2**, the corpse state, whose tick runs
/// `Anim_Collapse` → `FUN_0048895E` → the pour frame; a pot that has not
/// poured is in a walking or standing state and gets the idle loop from
/// `FUN_00488436`/`FUN_00488793`. `l2_sim`'s `pour_oil` kills the pot and sets
/// its `polar` to the pour axis in the same statement, so
/// [`Motion::Dying`] is exactly that state here. `[I]` on the mapping, `[V]`
/// on both frame formulas.
pub fn frame(troop: Troop, anim: Motion, facing: u8, polar: u8, phase: u8) -> Option<usize> {
    Some(match troop {
        // `dirc + 5`, in all three handlers.
        Troop::Catapults => 5 + (facing % 8) as usize,
        // `(polarDirc >> 1) + 1`. `FUN_00488436` recomputes the polar facing
        // first; that is `l2_sim::siege::tower_polar`, and simulation state.
        Troop::SiegeTowers => 1 + (polar % 8) as usize / 2,
        // One picture, every facing, every state.
        Troop::BatteringRams => 0,
        Troop::Oil => match anim {
            Motion::Dying => OIL_POUR_BASE + (polar % 8) as usize / 2,
            _ => OIL_IDLE_BASE + (phase as u16 % OIL_IDLE_PERIOD) as usize / 8,
        },
        _ => return None,
    })
}

/// Which of the two `Catarm` files a facing's arm comes from
/// frame in it — `FUN_004BE7BE` (`0x004BE7BE`) picks the sheet on
/// `dirc < 4`, and `FUN_00488436` computes `dirc * 5`, less `0x14` for the far
/// half. **[V]**
pub fn arm_sheet(facing: u8) -> usize {
    usize::from(facing % 8 >= 4)
}

/// **The catapult's arm** — sheet [`arm_sheet`], frame
/// `(dirc % 4) * 5 + g_horseWalkCycle[swing >> 2]`.
///
/// `swing` is ticks since this catapult loosed. In the original that is
/// `swingTimer - 100`, counted by `BattleMan_StateEngineFire` from the shot up
/// to 179 and then wrapped; `FUN_0048895E` holds `animPhase` at zero until the
/// shot, so the arm is still while the crew winds and arcs over afterwards.
///
/// **Our counter is not the original's.** `l2_sim`'s catapult runs through
/// `BattleRunner::fire_tick`, whose `reload_counter` resets to zero **at** the
/// loose and climbs to the weapon's 100-tick interval — the same zero point as
/// the original's `animPhase`, over a shorter window. Clamping at
/// [`ARM_SWING_TICKS`] keeps the arm inside the arc the original draws instead
/// of walking into `g_horseWalkCycle`'s tail, which belongs to a horse.
pub fn arm_frame(facing: u8, swing: u16) -> usize {
    let base = (facing % 8) as usize % 4 * ARM_POSES;
    base + ARM_CYCLE[(swing.min(ARM_SWING_TICKS) >> 2) as usize] as usize
}

/// **The ram beating on a gate** — `FUN_004BEAB9` (`0x004BEAB9`), two more
/// frames of `engine.pl8` above and below the carriage, and only while the
/// figure is in **state 14**, `BattleMan_StateRamGate`.
///
/// Returns `((upper frame, dy), (lower frame, dy))`, both `dy` relative to the
/// same cell corner the body is drawn from — the painter restores `g_drawX` /
/// `g_drawY` before it calls this. The x offset is the ordinary
/// `16 - width / 2`; the y offsets are **`-width / 2 - 0x10`** and
/// **`-width / 2 + 0x56`**, and neither carries the `+ 8` every other battle
/// sprite has.
///
/// `FUN_0048895E` counts the beam **down**: `animPhase - 1`, clamped back to
/// `0x8F` on underflow, `>> 4`, so the strip index walks 8 → 0 over 144 ticks
/// and snaps back. `l2_sim`'s `phase` counts up, so it is subtracted here to
/// give the same nine pictures in the same order.
///
/// `[I]` on the state: our nearest thing to state 14 is a ram with
/// [`Motion::Attacking`], which `BattleRunner::strike_castle` sets on exactly
/// the step the original refuses into `BattleMan_StateRamGate`.
pub fn ram_strips(anim: Motion, phase: u8) -> Option<((usize, i32), (usize, i32))> {
    if anim != Motion::Attacking {
        return None;
    }
    let counting_down = (RAM_PERIOD - 1) - (phase as u16 % RAM_PERIOD);
    let k = (counting_down >> 4) as usize;
    Some(((RAM_STRIP_UPPER + k, -0x10), (RAM_STRIP_LOWER + k, 0x56)))
}

/// **The stair a docked siege tower leaves** — `FUN_004BD759` (`0x004BD759`),
/// four frames of `engine.pl8` drawn *after* the men so the tower's top
/// overlaps them.
///
/// `FUN_00491492` writes `DAT_004D9DD0[k + polarDirc * 9]` into the frame byte
/// of every cell of the docked tower's 3 × 3 and sets cell byte `+2` bit
/// `0x80` on the centre. Those four centre codes are `0x49`, `0x4C`, `0x61`
/// and `0x64` — read out of the table at `0x004D9DD0` — and they are exactly
/// the four `FUN_004BD759` answers. **[V]** on both sides.
///
/// Our `Battlefield` carries no byte `+2`, so the `0x80` gate is unavailable;
/// [`crate::scene::dock_overlay`] reads `flags & 1` instead, which
/// `l2_sim::siege::lay_tower_ramp` sets on the same nine cells and nothing
/// else in the crate sets at all. **That gate matters**: the field tileset's
/// hill range covers 64 … 111, so codes 73, 76, 97 and 100 are ordinary hill
/// tiles too
pub const DOCK_OVERLAY: [(u8, usize); 4] = [(0x49, 0x1F), (0x4C, 0x20), (0x61, 0x21), (0x64, 0x22)];

/// The `engine.pl8` frame a docked tower's centre cell draws, if it is one.
pub fn dock_overlay_frame(gfx: u8) -> Option<usize> {
    DOCK_OVERLAY.iter().find(|(code, _)| *code == gfx).map(|(_, f)| *f)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The frame count of each shipped file, so the arithmetic can be checked
    /// with no install. `tests/install.rs` asserts these against the files.
    pub(crate) const ENGINE_FRAMES: usize = 46;
    pub(crate) const ARM_FRAMES: usize = 20;

    #[test]
    fn every_engine_frame_lands_inside_the_forty_six() {
        for troop in [Troop::Catapults, Troop::SiegeTowers, Troop::BatteringRams, Troop::Oil] {
            for anim in [Motion::Idle, Motion::Walking, Motion::Attacking, Motion::Dying] {
                for facing in 0..8u8 {
                    for polar in [0u8, 2, 4, 6] {
                        for phase in 0..=255u8 {
                            let f = frame(troop, anim, facing, polar, phase).unwrap();
                            assert!(f < ENGINE_FRAMES, "{troop:?} {anim:?} -> {f}");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn a_man_has_no_engine_frame() {
        for t in [Troop::Peasants, Troop::Archers, Troop::Knights, Troop::Pikemen] {
            assert_eq!(frame(t, Motion::Idle, 0, 0, 0), None);
        }
    }

    /// The carriage occupies 5 … 12, one per facing
    /// per orthogonal — the two blocks the frame map closes with.
    #[test]
    fn the_carriage_is_eight_facings_and_the_tower_is_four() {
        let carriage: Vec<_> =
            (0..8u8).map(|d| frame(Troop::Catapults, Motion::Walking, d, 0, 0).unwrap()).collect();
        assert_eq!(carriage, (5..=12).collect::<Vec<_>>());
        let tower: Vec<_> = [0u8, 2, 4, 6]
            .iter()
            .map(|&p| frame(Troop::SiegeTowers, Motion::Walking, 0, p, 0).unwrap())
            .collect();
        assert_eq!(tower, vec![1, 2, 3, 4]);
    }

    /// Idling is six pictures and pouring is four, and they do not overlap.
    #[test]
    fn a_pot_idles_in_six_pictures_and_pours_in_four() {
        let idle: std::collections::BTreeSet<_> =
            (0..=255u8).map(|p| frame(Troop::Oil, Motion::Idle, 0, 0, p).unwrap()).collect();
        assert_eq!(idle.iter().copied().collect::<Vec<_>>(), vec![35, 36, 37, 38, 39, 40]);
        let pour: Vec<_> = [0u8, 2, 4, 6]
            .iter()
            .map(|&p| frame(Troop::Oil, Motion::Dying, 0, p, 0).unwrap())
            .collect();
        assert_eq!(pour, vec![42, 43, 44, 45]);
        assert!(pour.iter().all(|f| !idle.contains(f)));
    }

    /// Four facings of five poses, twice — every arm frame is inside one
    /// `Catarm` file
    #[test]
    fn the_arm_is_four_facings_of_five_poses_in_each_of_two_files() {
        for facing in 0..8u8 {
            for swing in 0..=200u16 {
                let f = arm_frame(facing, swing);
                assert!(f < ARM_FRAMES, "facing {facing} swing {swing} -> {f}");
            }
        }
        assert_eq!(arm_sheet(0), 0);
        assert_eq!(arm_sheet(3), 0);
        assert_eq!(arm_sheet(4), 1);
        assert_eq!(arm_sheet(7), 1);
        // The far half wraps onto the same four bases: `dirc * 5 - 0x14`.
        for d in 0..4u8 {
            assert_eq!(arm_frame(d, 0), arm_frame(d + 4, 0));
        }
    }

    /// The arm is still at the shot and arcs over afterwards.
    #[test]
    fn the_arm_holds_at_rest_until_the_shot_and_then_swings() {
        assert_eq!(arm_frame(0, 0) % ARM_POSES, 0, "at rest when the counter is zero");
        let poses: Vec<_> = (0..=ARM_SWING_TICKS).map(|s| arm_frame(0, s) % ARM_POSES).collect();
        assert_eq!(*poses.last().unwrap(), 3, "the arm ends over");
        assert!(poses.windows(2).all(|w| w[1] >= w[0]), "the arc never runs backwards: {poses:?}");
    }

    /// Nine pictures, walking down, and both strips stay inside the sheet.
    #[test]
    fn a_ram_beating_a_gate_walks_nine_pictures_downward() {
        assert!(ram_strips(Motion::Walking, 0).is_none(), "only while it is beating");
        assert!(ram_strips(Motion::Idle, 0).is_none());
        let mut uppers = Vec::new();
        for phase in 0..RAM_PERIOD as u8 {
            let ((u, du), (l, dl)) = ram_strips(Motion::Attacking, phase).unwrap();
            assert!(u < ENGINE_FRAMES && l < ENGINE_FRAMES);
            assert_eq!(u - l, RAM_STRIP_UPPER - RAM_STRIP_LOWER);
            assert_eq!((du, dl), (-0x10, 0x56));
            uppers.push(u);
        }
        assert_eq!(uppers[0], 0x16 + 8, "the count starts at the top");
        assert_eq!(*uppers.last().unwrap(), 0x16, "and ends at the bottom");
        assert!(uppers.windows(2).all(|w| w[1] <= w[0]), "it counts down");
    }

    /// The four codes are the centres of `DAT_004D9DD0`'s four rows, and they
    /// map onto four consecutive frames.
    #[test]
    fn the_dock_overlay_is_four_codes_and_four_frames() {
        assert_eq!(dock_overlay_frame(0x49), Some(0x1F));
        assert_eq!(dock_overlay_frame(0x64), Some(0x22));
        assert_eq!(dock_overlay_frame(0x4A), None, "a neighbour of the centre draws nothing");
        assert_eq!(dock_overlay_frame(0), None);
        for (_, f) in DOCK_OVERLAY {
            assert!(f < ENGINE_FRAMES);
        }
    }
}
