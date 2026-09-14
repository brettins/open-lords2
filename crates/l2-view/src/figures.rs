//! Which sprite frame a figure is showing.
//!
//! A battle sprite sheet is not a flat list of pictures. `A2r_swor.pl8` holds
//! 114 frames laid out as **eight facings of twelve poses, then eighteen
//! shared frames**, and the engine computes an index into that layout every
//! tick. This module reproduces the computation.
//!
//! # The layout
//!
//! ```text
//! frame = facing * poses_per_facing + pose        (facing 0..7)
//!
//!   pose 0 ..= 5                 walking, one pose every 4 ticks over 24
//!   pose 6 ..                    striking, from a per-troop cycle table
//!   pose <idle> = N-1            standing
//!   pose 10 ..= 12               drawing a bow (crossbowmen and archers)
//!
//! then, after 8 * poses_per_facing:
//!   +0 ..= +5                    six further shared frames
//!   +6 ..                        dying: 4 half-facings of 3 frames
//! ```
//!
//! # Where the numbers come from
//!
//! `poses_per_facing`, the idle pose and the strike cycles are read from the
//! per-state animation handlers in `Lords2.exe`: `0x00486D83` (walking),
//! `0x00486249` (striking and standing), `0x004872AE` (standing with the
//! fidget), `0x0048804A` (the bow draw) and `0x00487908` (dying). They all compute
//! `figure->frame` (`+0x10`) and hand it to `BattleFigure_Draw`
//! (`0x004BDC31`), which indexes `sheet + frame * 0x10 + 8` — the PL8 frame
//! record. **[V]**
//!
//! # Why the numbers are trustworthy
//!
//! The arithmetic is checked against the shipped art
//! decompiler. For every one of the **36** `a2` sprite files that are not
//! knights — six player colours times six troop types — the frame count is
//!, and the dying handler's base index is
//! exactly `8 * poses_per_facing + 6` in all four of its groups. Getting
//! `poses_per_facing` wrong for any troop breaks both identities at once.
//! `tests/install.rs` asserts both identities over the install.
//!
//! Knights are different and are handled separately: they are drawn on a horse
//! and their frame comes from an 8 x 8 `(body facing, target facing)` table at
//! `0x004D9C30`. That table's sixteen live entries
//! are spaced three apart and top out at 53, which with the walk cycle's
//! maximum of 2 reaches frame 55 — exactly the 56 real frames of
//! `A2r_knig.pl8`. **[V]**

use crate::drawbow;
use l2_sim::Troop;

/// Facings, the eight-way delta table and `Dir_FromDelta` live in `l2-sim`: a
/// facing decides where a figure *walks*, so it is simulation state and not
/// artwork. Re-exported because everything in this module indexes by it.
pub use l2_sim::facing::{facing_from_delta, FACINGS, FACING_DELTA};

/// What a figure is doing, as far as the artwork is concerned — the
/// simulation's [`l2_sim::Motion`], under the name this module has always used
/// for it.
pub use l2_sim::Motion as Anim;

/// Player colours, in the order `Lords2.exe`'s battle asset table lists them
/// (`0x004DA6B8` onward: `a2w_`, `a2r_`, `a2y_`, `a2k_`, `a2p_`, `a2b_`).
/// **[V]** The index is the owning realm's colour byte, campaign unit `+0x02`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Colour {
    White,
    Red,
    Yellow,
    Black,
    Purple,
    Blue,
}

impl Colour {
    pub const ALL: [Colour; 6] =
        [Colour::White, Colour::Red, Colour::Yellow, Colour::Black, Colour::Purple, Colour::Blue];

    pub fn letter(self) -> char {
        match self {
            Colour::White => 'w',
            Colour::Red => 'r',
            Colour::Yellow => 'y',
            Colour::Black => 'k',
            Colour::Purple => 'p',
            Colour::Blue => 'b',
        }
    }
}

/// The seven troop types that have battlefield sprites, in the asset table's
/// order — which is also the `TROOPS*.ENG` column order. **[V]**
pub fn stem(troop: Troop) -> Option<&'static str> {
    Some(match troop {
        Troop::Peasants => "psnt",
        Troop::Crossbowmen => "cros",
        Troop::Macemen => "mace",
        Troop::Swordsmen => "swor",
        Troop::Pikemen => "pike",
        Troop::Archers => "arch",
        Troop::Knights => "knig",
        // Siege engines are drawn from Engine.pl8 and Catarm*.pl8, not from a
        // per-colour troop sheet.
        _ => return None,
    })
}

/// `A2r_swor.pl8`, and so on. The install's casing is inconsistent, which is
/// why every load goes through the mod overlay's case-insensitive lookup.
pub fn sprite_file(colour: Colour, troop: Troop) -> Option<String> {
    stem(troop).map(|s| format!("A2{}_{}.pl8", colour.letter(), s))
}

/// The horse a knight is drawn on: 48 frames, eight facings of six.
pub const HORSE_FILE: &str = "A2_horse.pl8";
pub const HORSE_POSES: u8 = 6;

/// Poses per facing. **[V]** from the animation handlers, and cross-checked
/// against every shipped sheet — see the module docs.
pub fn poses_per_facing(troop: Troop) -> u8 {
    match troop {
        Troop::Peasants => 10,
        Troop::Crossbowmen => 13,
        Troop::Macemen => 12,
        Troop::Swordsmen => 12,
        Troop::Pikemen => 8,
        Troop::Archers => 13,
        // Knights do not use a stride; the value is what `0x00486249`'s
        // fall-through branch loads, kept for completeness.
        _ => 12,
    }
}

/// **What a pose needs besides the troop, the anim and the facing.**
///
/// Three of the five animation handlers read a figure field that is not the
/// phase, and drawing them from the phase alone was three of the five
/// mismatches the oracle check found. `From<u8>` keeps the plain phase-only
/// call for the handlers that read nothing else.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Pose {
    /// `animPhase` (`+0x0E`) - the walk, strike and dying cadences.
    pub phase: u8,
    /// The figure's own index, `g_curBattleMan`. `Anim_StandA2`
    /// (`0x004872AE`) picks its standing pose out of the low three bits.
    pub index: usize,
    /// `swingTimer` - the missile reload counter. `Anim_DrawBowA2`
    /// (`0x0048804A`) indexes its curve with it. [`crate::drawbow`].
    pub swing: u16,
    /// `role` (`+0x185`) is 2, not 1: this figure is the **defender** of a
    /// melee pair. `Anim_StrikeA2` (`0x00486249`) runs the strike cycle only
    /// under `role == 1` and draws [`defend_pose`] for the other man.
    pub defending: bool,
}

impl From<u8> for Pose {
    fn from(phase: u8) -> Self {
        Pose { phase, ..Pose::default() }
    }
}

/// The pose a live figure is in. One reader of the figure record, so a picture
/// drawn outside the renderer cannot drift from the renderer's.
pub fn pose_of(runner: &l2_sim::runner::BattleRunner, i: usize) -> Pose {
    let f = &runner.fighters[i];
    let sim = &runner.sim.figures[f.sim];
    Pose {
        // **The walk frame is read before the step.** `Anim_WalkA2`
        // (`00480000.c:2754-2756`) computes `dirc * stride + (animPhase >> 2)`
        // and *then* does `animPhase += 1`; `BattleRunner::march` has already
        // stepped it by the time we look, so the drawn pose is one behind —
        // exact, because that step wraps at 24. `Anim_StrikeA2` (`2509`) and
        // `Anim_DyingA2` (`3010`) step first and read after, so they take the
        // counter as it stands. Ours drew the walk a tick ahead of the binary.
        phase: match f.anim {
            l2_sim::Motion::Walking => (f.phase + 23) % 24,
            _ => f.phase,
        },
        index: i,
        swing: sim.reload_counter,
        defending: sim.role == l2_sim::Role::Defending,
    }
}

/// **The defender's pose** - `Anim_StrikeA2` (`0x00486249`), its `local_18`.
///
/// Both men of a melee pair are drawn by `Anim_StrikeA2`; only the one with
/// `role == 1` (`+0x185`) is swinging. The other stands, at a pose that is not
/// the one `Anim_StandA2` gives him: 11 for a swordsman or maceman, 9 for a
/// bowman or a peasant, 7 for a pikeman. **[V]**
fn defend_pose(troop: Troop) -> u8 {

    match troop {
        Troop::Peasants => 9,
        Troop::Crossbowmen | Troop::Archers => 9,
        Troop::Macemen | Troop::Swordsmen => 11,
        Troop::Pikemen => 7,
        _ => 0,
    }
}

/// First **striking** pose. The same for every troop. **[V]**
///
/// **Corrected.** This was `WALK_BASE` and the two bands were the wrong way
/// round here for the same reason `docs/battle.md` §13.5 had them swapped.
/// §14.5 is the correction: `Anim_WalkA2` (`0x00486D83`) gives poses 0…5 and is
/// called from the two states that walk, `Anim_StrikeA2` (`0x00486249`) gives
/// poses from base 6 and is called from the two that hit, and the index space
/// closes only one way round — an archer's N is 13, so 0–5 walk, 6–8 strike,
/// 9 stand and **10–12 the bow draw**, which is where `Anim_DrawBowA2`'s `+ 10`
/// points and where nothing else can.
const STRIKE_BASE: u8 = 6;

/// **The standing pose** - `Anim_StandA2` (`0x004872AE`), its `local_c`.
///
/// Not `N-1`, and not per-troop: the figure's **own index** `& 7`, with 6 and 7
/// folded back to 1 and 2, so a rank of men stand in six different postures out
/// of the walk band instead of one. **[V]**
fn stand_pose(index: usize) -> usize {
    match index & 7 {
        6 => 1,
        7 => 2,
        n => n,
    }
}

/// Ten-entry **strike** cycles — `g_strikeCycleMace` (`0x004D9A00`),
/// `g_strikeCycleBow` (`0x004D9A28`) and `g_strikeCyclePike` (`0x004D9A50`),
/// stepped once every four ticks over a forty-tick loop. **[V]**
const STRIKE_HEAVY: [u8; 10] = [0, 1, 2, 3, 4, 4, 3, 2, 1, 0];
const STRIKE_LIGHT: [u8; 10] = [0, 0, 1, 1, 2, 2, 1, 1, 0, 0];
const STRIKE_PIKE: [u8; 10] = [0, 0, 1, 1, 1, 1, 1, 0, 0, 0];

fn strike_cycle(troop: Troop) -> &'static [u8; 10] {
    match troop {
        Troop::Macemen | Troop::Swordsmen => &STRIKE_HEAVY,
        Troop::Pikemen => &STRIKE_PIKE,
        _ => &STRIKE_LIGHT,
    }
}

/// The 8 x 8 `(body facing, target facing)` table at `0x004D9C30` that gives a
/// knight's frame. Zero means "this pair has no artwork"; the engine rotates
/// the body facing until it finds one. **[V]**
const KNIGHT_FRAMES: [[u8; 8]; 8] = [
    [0, 0, 0, 0, 0, 0, 0, 0],
    [11, 0, 14, 17, 0, 0, 0, 8],
    [0, 0, 0, 0, 0, 0, 0, 0],
    [0, 20, 23, 0, 26, 29, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 32, 35, 0, 38, 53],
    [0, 0, 0, 0, 0, 0, 0, 0],
    [50, 53, 0, 0, 0, 44, 47, 0],
];

/// Pick the frame index for a figure.
///
/// `facing` is 0..7 and `phase` is the figure's own animation counter — the
/// original keeps one per figure at `+0x0E`, seeded differently per figure so
/// that identical men do not march in lockstep.
pub fn frame(troop: Troop, anim: Anim, facing: u8, pose: impl Into<Pose>) -> usize {
    let pose = pose.into();
    let phase = pose.phase;
    let facing = (facing % FACINGS as u8) as usize;

    // `Anim_DrawBowA2` (`0x0048804A`) has no knight arm and no per-troop
    // stride: it is one formula for every `troopType < 7`. [`crate::drawbow`].
    if anim == Anim::Shooting {
        return drawbow::frame(troop, facing as u8, pose.swing);
    }

    if troop == Troop::Knights {
        // **Only `Anim_StrikeA2` has the knight arm with the table.**
        // `00480000.c:2565`: a knight's body facing snaps to whichever of the
        // eight `DAT_004D9C30` rows has artwork for the facing it wants,
        // searching outward from its current one, and the strike cycle rides
        // on top — for the defender too, whose phase is merely frozen.
        //
        // `Anim_WalkA2` (`00480000.c:2896`) and `Anim_StandA2` (`2873`) have
        // their own, much shorter knight arm: the **rider** frame is the bare
        // facing 0 … 7 and the cadence goes on the horse sheet
        // ([`horse_frame`]). Ours put walk and stand on the strike formula, so
        // a riding knight's body flickered through the swing.
        //
        // Both of those arms read `dirc`, not `facingDrawn`: `2896-2900` is
        // `frame = dirc & 7` written *over* the fidget's frame. The caller
        // hands `facing` in — `l2_view::scene::render` picks `dirc` for a
        // standing knight for this reason.
        let base = knight_base(facing, facing) as usize;
        return match anim {
            Anim::Attacking => base + strike_cycle(troop)[((phase % 40) / 4) as usize] as usize,
            Anim::Walking | Anim::Idle => facing,
            // Knights have no separate dying block in this table.
            Anim::Dying => base,
            // Handled above: `Anim_DrawBowA2` has no knight arm either.
            Anim::Shooting => drawbow::frame(troop, facing as u8, pose.swing),
        };
    }

    let stride = poses_per_facing(troop) as usize;
    match anim {
        // `Anim_StandA2` `0x004872AE`: a per-figure pose out of the walk band,
        // and the fidget turns the drawn facing - `Fighter::facing_drawn`.
        Anim::Idle => facing * stride + stand_pose(pose.index),
        // `Anim_WalkA2` `0x00486D83`: six poses, one every four ticks, over a
        // 24-tick loop. Read from `dirc` (`+0x18`).
        Anim::Walking => facing * stride + ((phase % 24) / 4) as usize,
        // `Anim_StrikeA2` `0x00486249`: base 6 plus the per-troop strike cycle,
        // stepped every fourth tick of a forty-tick loop. Read from `dirc2`
        // (`+0x19`).
        // ...and only for the man whose `role` is 1. The defender of the pair
        // is drawn by the same handler, standing.
        Anim::Attacking if pose.defending => facing * stride + defend_pose(troop) as usize,
        Anim::Attacking => {
            let step = strike_cycle(troop)[((phase % 40) / 4) as usize];
            facing * stride + STRIKE_BASE as usize + step as usize
        }
        // Handled above: the bow draw does not use the troop's stride.
        Anim::Shooting => drawbow::frame(troop, facing as u8, pose.swing),
        // `0x00487908`: base + (facing & 6) / 2 * 3 + phase / 32, phase 0..95.
        Anim::Dying => {
            let base = FACINGS * stride + 6;
            base + (facing & 6) / 2 * 3 + ((phase % 96) / 32) as usize
        }
    }
}

/// A knight's frame base for a `(body, target)` facing pair, rotating the body
/// facing outward until the table has artwork. **[V]** `0x00486249`.
fn knight_base(body: usize, target: usize) -> u8 {
    let (mut up, mut down) = (body, body);
    for _ in 0..FACINGS {
        if KNIGHT_FRAMES[up][target] != 0 {
            return KNIGHT_FRAMES[up][target];
        }
        up = (up + 1) % FACINGS;
        down = (down + FACINGS - 1) % FACINGS;
        if KNIGHT_FRAMES[up][target] != 0 {
            return KNIGHT_FRAMES[up][target];
        }
        if KNIGHT_FRAMES[down][target] != 0 {
            return KNIGHT_FRAMES[down][target];
        }
    }
    0
}

/// The horse frame under a knight: eight facings of six poses.
///
/// `horseFrame = |dirc| * 6 + (animPhase >> 2)` in `Anim_WalkA2`
/// (`0x00486D83`, `00480000.c:2762`), over that handler's 24-tick loop — so
/// the six poses are exactly covered. `Anim_StandA2` (`0x004872AE`) and
/// `Anim_StrikeA2` write `|dirc| * 6` with no phase term: a standing or
/// swinging knight sits a still horse. **[V]**
pub fn horse_frame(facing: u8, anim: Anim, phase: u8) -> usize {
    let facing = (facing % FACINGS as u8) as usize;
    let step = match anim {
        Anim::Walking => ((phase % 24) / 4) as usize,
        _ => 0,
    };
    facing * HORSE_POSES as usize + step
}

/// Where a figure is drawn while it is between cells.
///
/// `BattleFigure_Draw` adds `g_walkOffset32[facing][walking]` to the cell's
/// screen position, where `walking` is the figure's sub-cell progress. The
/// table is at `0x004E4030`; every entry is `(±(32 - 2*step), ±(32 - 2*step))`
/// for the axes the facing moves along, so the whole 8 x 17 table collapses to
/// this. **[V]** — reproduced arithmetically and asserted against the bytes
/// read out of `Lords2.exe` in `tools/view/`.
///
/// The sign is what independently confirms the facing numbering: facing 0 is
/// north, and its offset is *positive* y, i.e. the figure is drawn trailing
/// south of the cell it is walking into.
pub fn walk_offset(facing: u8, walking: u8) -> (i32, i32) {
    if walking == 0 || walking > 16 {
        return (0, 0);
    }
    let d = 32 - 2 * walking as i32;
    let (sx, sy) = match facing % 8 {
        0 => (0, 1),
        1 => (-1, 1),
        2 => (-1, 0),
        3 => (-1, -1),
        4 => (0, -1),
        5 => (1, -1),
        6 => (1, 0),
        _ => (1, 1),
    };
    (sx * d, sy * d)
}

#[cfg(test)]
#[path = "figures_tests.rs"]
mod tests;
