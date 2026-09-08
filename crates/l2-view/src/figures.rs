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
//!   pose 0 ..= 5                 attacking, one pose every 4 ticks
//!   pose 6 ..                    walking, from a per-troop cycle table
//!   pose <idle>                  standing
//!
//! then, after 8 * poses_per_facing:
//!   +0 ..= +5                    six further shared frames
//!   +6 ..                        dying: 4 half-facings of 3 frames
//! ```
//!
//! # Where the numbers come from
//!
//! `poses_per_facing`, the idle pose and the walk cycles are read from the
//! per-state animation handlers in `Lords2.exe`: `0x00486249` (idle/walk),
//! `0x00486D83` (attacking) and `0x00487908` (dying). They all compute
//! `figure->frame` (`+0x10`) and hand it to `BattleFigure_Draw`
//! (`0x004BDC31`), which indexes `sheet + frame * 0x10 + 8` — the PL8 frame
//! record. **[V]**
//!
//! # Why the numbers are trustworthy
//!
//! The arithmetic is checked against the shipped art, not just read out of a
//! decompiler. For every one of the **36** `a2` sprite files that are not
//! knights — six player colours times six troop types — the frame count is
//! exactly `8 * poses_per_facing + 18`, and the dying handler's base index is
//! exactly `8 * poses_per_facing + 6` in all four of its groups. Getting
//! `poses_per_facing` wrong for any troop breaks both identities at once.
//! `tests/install.rs` asserts both identities over the install.
//!
//! Knights are different and are handled separately: they are drawn on a horse
//! and their frame comes from an 8 x 8 `(body facing, target facing)` table at
//! `0x004D9C30` rather than from a stride. That table's sixteen live entries
//! are spaced three apart and top out at 53, which with the walk cycle's
//! maximum of 2 reaches frame 55 — exactly the 56 real frames of
//! `A2r_knig.pl8`. **[V]**

use l2_sim::Troop;

/// The eight facings, in the original's numbering. Confirmed independently by
/// `docs/battle.md` §2.1 and by the sub-cell offset table below, whose signs
/// only make sense under this order.
pub const FACINGS: usize = 8;

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

/// The standing pose. **[V]** `0x00486249`.
fn idle_pose(troop: Troop) -> u8 {
    match troop {
        Troop::Peasants => 9,
        Troop::Crossbowmen | Troop::Archers => 9,
        Troop::Macemen | Troop::Swordsmen => 11,
        Troop::Pikemen => 7,
        _ => 0,
    }
}

/// First walking pose. The same for every troop. **[V]**
const WALK_BASE: u8 = 6;

/// Ten-entry walk cycles at `0x004D9A00`, `0x004D9A28` and `0x004D9A50`,
/// stepped once every four ticks over a forty-tick loop. **[V]**
const WALK_HEAVY: [u8; 10] = [0, 1, 2, 3, 4, 4, 3, 2, 1, 0];
const WALK_LIGHT: [u8; 10] = [0, 0, 1, 1, 2, 2, 1, 1, 0, 0];
const WALK_PIKE: [u8; 10] = [0, 0, 1, 1, 1, 1, 1, 0, 0, 0];

fn walk_cycle(troop: Troop) -> &'static [u8; 10] {
    match troop {
        Troop::Macemen | Troop::Swordsmen => &WALK_HEAVY,
        Troop::Pikemen => &WALK_PIKE,
        _ => &WALK_LIGHT,
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

/// What a figure is doing, as far as the artwork is concerned. These are the
/// three animation handlers that were read; the original has more (firing,
/// siege engines, siege walls) and they are not modelled here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anim {
    Idle,
    Walking,
    Attacking,
    Dying,
}

/// Pick the frame index for a figure.
///
/// `facing` is 0..7 and `phase` is the figure's own animation counter — the
/// original keeps one per figure at `+0x0E`, seeded differently per figure so
/// that identical men do not march in lockstep.
pub fn frame(troop: Troop, anim: Anim, facing: u8, phase: u8) -> usize {
    let facing = (facing % FACINGS as u8) as usize;

    if troop == Troop::Knights {
        // A knight's body facing snaps to whichever of the eight rows has
        // artwork for the facing it wants, searching outward from its current
        // one exactly as `0x00486249` does.
        let base = knight_base(facing, facing);
        let step = walk_cycle(troop)[((phase % 40) / 4) as usize];
        return match anim {
            Anim::Walking | Anim::Idle | Anim::Attacking => base as usize + step as usize,
            // Knights have no separate dying block in this table.
            Anim::Dying => base as usize,
        };
    }

    let stride = poses_per_facing(troop) as usize;
    match anim {
        Anim::Idle => facing * stride + idle_pose(troop) as usize,
        Anim::Walking => {
            let step = walk_cycle(troop)[((phase % 40) / 4) as usize];
            facing * stride + WALK_BASE as usize + step as usize
        }
        // `0x00486D83`: six poses, one every four ticks, over a 24-tick loop.
        Anim::Attacking => facing * stride + ((phase % 24) / 4) as usize,
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
pub fn horse_frame(facing: u8, phase: u8) -> usize {
    let facing = (facing % FACINGS as u8) as usize;
    facing * HORSE_POSES as usize + (((phase % 40) / 4) as usize).min(HORSE_POSES as usize - 1)
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

    const SPRITE_TROOPS: [Troop; 6] = [
        Troop::Peasants,
        Troop::Crossbowmen,
        Troop::Macemen,
        Troop::Swordsmen,
        Troop::Pikemen,
        Troop::Archers,
    ];

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

    #[test]
    fn the_walk_offset_trails_the_cell_being_entered() {
        // Walking north: drawn below the destination, closing on it.
        assert_eq!(walk_offset(0, 1), (0, 30));
        assert_eq!(walk_offset(0, 16), (0, 0));
        // Walking east: drawn to the west of it.
        assert_eq!(walk_offset(2, 1), (-30, 0));
        // Every facing's offset points opposite its movement.
        for f in 0..8u8 {
            let (dx, dy) = FACING_DELTA[f as usize];
            let (ox, oy) = walk_offset(f, 2);
            assert_eq!(ox.signum(), -dx.signum(), "facing {f} x");
            assert_eq!(oy.signum(), -dy.signum(), "facing {f} y");
        }
        assert_eq!(walk_offset(3, 0), (0, 0), "not walking means no offset");
    }

    #[test]
    fn every_animation_frame_lands_inside_its_sheet() {
        for troop in SPRITE_TROOPS {
            let total = FACINGS * poses_per_facing(troop) as usize + 18;
            for facing in 0..8u8 {
                for phase in 0..=120u8 {
                    for anim in [Anim::Idle, Anim::Walking, Anim::Attacking, Anim::Dying] {
                        let f = frame(troop, anim, facing, phase);
                        assert!(
                            f < total,
                            "{troop:?} {anim:?} facing {facing} phase {phase} -> {f} of {total}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn walking_and_attacking_stay_inside_their_own_pose_bands() {
        for troop in SPRITE_TROOPS {
            let stride = poses_per_facing(troop) as usize;
            for facing in 0..8usize {
                for phase in 0..=120u8 {
                    let w = frame(troop, Anim::Walking, facing as u8, phase) - facing * stride;
                    assert!((6..stride).contains(&w), "{troop:?} walk pose {w}");
                    let a = frame(troop, Anim::Attacking, facing as u8, phase) - facing * stride;
                    assert!(a < 6, "{troop:?} attack pose {a}");
                }
            }
        }
    }

    #[test]
    fn dying_uses_four_half_facings_of_three_frames() {
        let troop = Troop::Swordsmen;
        let base = FACINGS * poses_per_facing(troop) as usize + 6;
        let mut seen = std::collections::HashSet::new();
        for facing in 0..8u8 {
            for phase in 0..96u8 {
                seen.insert(frame(troop, Anim::Dying, facing, phase));
            }
        }
        assert_eq!(seen.len(), 12, "4 half-facings x 3 frames");
        assert_eq!(*seen.iter().min().unwrap(), base);
        assert_eq!(*seen.iter().max().unwrap(), base + 11);
        // Facings that share a half-facing share their death frames.
        assert_eq!(
            frame(troop, Anim::Dying, 2, 0),
            frame(troop, Anim::Dying, 3, 0),
            "2 and 3 are the same half-facing"
        );
    }

    #[test]
    fn a_knight_always_finds_artwork_and_never_leaves_its_sheet() {
        for facing in 0..8u8 {
            for phase in 0..=120u8 {
                let f = frame(Troop::Knights, Anim::Walking, facing, phase);
                assert!(f >= 8, "facing {facing} fell through to the unused low frames");
                assert!(f < 56, "facing {facing} phase {phase} -> {f}, past the 56 real frames");
            }
        }
    }

    #[test]
    fn sprite_file_names_follow_the_asset_table() {
        assert_eq!(sprite_file(Colour::Red, Troop::Swordsmen).unwrap(), "A2r_swor.pl8");
        assert_eq!(sprite_file(Colour::Blue, Troop::Peasants).unwrap(), "A2b_psnt.pl8");
        assert_eq!(sprite_file(Colour::Black, Troop::Knights).unwrap(), "A2k_knig.pl8");
        assert!(sprite_file(Colour::Red, Troop::Catapults).is_none());
    }
}
