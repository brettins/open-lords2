#![allow(unused_imports)]
use super::*;
use super::moat::*;
use l2_sim::siege;
use l2_sim::terrain::{id, DIM};
use l2_sim::{BattleRunner, Muster, Troop};

/// **The garrison lowers its own drawbridge**, and what that costs it.
///
/// `FUN_00496B9F` sets exactly the globals the twenty-thousandth ram hit sets:
/// `_DAT_00569588`, and four on each of the two progress scores. So opening
/// your own gate to sally hands the besieger's AI the same signal it would have
/// got from breaking the gate down itself — which is the mechanical reason the
/// shipped `Readme.txt` says *"drawbridges can not be closed once they have
/// been opened."*
#[test]
fn lowering_the_drawbridge_opens_a_way_through_and_tells_the_besieger_so() {
    let mut r = siege_battle(3, 7);
    assert!(r.has_drawbridge(), "a stone castle has one");
    assert!(!r.siege.gate_breached, "and it is shut to begin with");
    let (breach, approach) = (r.ai.breach_score, r.ai.approach_score);

    let bridge_cells =
        r.field.cells.iter().filter(|c| c.flags & siege::FLAG_DRAWBRIDGE != 0).count();
    assert!(bridge_cells > 0);

    assert!(r.lower_drawbridge(), "the bridge comes down");

    assert!(r.siege.drawbridge_down, "DAT_0052AF9C, the latch");
    assert!(r.siege.gate_breached, "_DAT_00569588, the gate is open");
    assert_eq!(r.ai.breach_score - breach, siege::GATE_BREACH_SCORE);
    assert_eq!(r.ai.approach_score - approach, siege::GATE_BREACH_SCORE);
    assert_eq!(
        r.field.cells.iter().filter(|c| c.flags & siege::FLAG_DRAWBRIDGE != 0).count(),
        0,
        "every cell the patch covered had its flags cleared",
    );

    // Once, and once only.
    assert!(!r.lower_drawbridge(), "a drawbridge cannot be raised again");
}

/// **The patch is 7 × 4 and the frames are the ones in the executable**, laid
/// down in the patch's own row-major order.
///
/// A table exercised at one input is `docs/decisions.md` C26's shape, so this
/// reads all twenty-eight cells back off the field. The **shape** is what makes
/// the *reading* checkable: fifteen cells carry
/// the filler frame 197 and the other thirteen trace a diagonal in frames
/// 192…198 into the corner nearest the gate. Twenty-eight `i32`s misread as
/// bytes, or bytes misread as `i32`s, would have no shape at all — they would
/// be uniform or they would be noise.
#[test]
fn the_drawbridge_patch_is_seven_by_four_and_drawn_with_the_executables_frames() {
    let mut r = siege_battle(4, 11);
    let anchor = r
        .field
        .cells
        .iter()
        .position(|c| c.flags & siege::FLAG_DRAWBRIDGE != 0)
        .expect("a royal castle has a drawbridge");
    assert!(r.lower_drawbridge());

    let (ax, ay) = (anchor % DIM, anchor / DIM);
    for row in 0..siege::DRAWBRIDGE_ROWS {
        for col in 0..siege::DRAWBRIDGE_COLS {
            let cell = &r.field.cells[(ay + row) * DIM + ax + col];
            assert_eq!(
                cell.gfx,
                siege::DRAWBRIDGE_FRAMES[row * siege::DRAWBRIDGE_COLS + col],
                "frame at row {row} column {col}",
            );
            assert_eq!(cell.surface, siege::SURFACE_GROUND, "row {row} column {col}");
            assert_eq!(cell.flags, 0, "row {row} column {col}");
        }
    }
    assert_eq!((siege::DRAWBRIDGE_ROWS, siege::DRAWBRIDGE_COLS), (7, 4));
    let filler = siege::DRAWBRIDGE_FRAMES.iter().filter(|&&f| f == 197).count();
    assert_eq!(filler, 15, "the filler frame");
    assert_eq!(
        siege::DRAWBRIDGE_FRAMES.iter().filter(|&&f| f != 197).count(),
        13,
        "and thirteen cells that are the bridge itself",
    );
    assert!(
        siege::DRAWBRIDGE_FRAMES.iter().all(|&f| (192..=198).contains(&f)),
        "every frame is in the castle tileset's 192…198 run",
    );
}

/// **A palisade has nothing to lower**, and the button is not spent trying.
///
/// The original's latch is set *inside* the search's `if`, so a castle with no
/// `0x40` cell leaves `DAT_0052AF9C` clear. It matters because the level-3
/// guard and the search are two separate tests — the first is the button's and
/// the second is the routine's — and only the first is what the shipped
/// `Readme.txt` describes.
#[test]
fn a_castle_with_no_drawbridge_does_not_spend_the_latch() {
    for level in 0..3u8 {
        let mut r = siege_battle(level, 3);
        assert!(!r.has_drawbridge(), "level {level} has no drawbridge");
        assert!(!r.lower_drawbridge(), "level {level}");
        assert!(!r.siege.drawbridge_down, "level {level}: the latch is untouched");
        assert!(!r.siege.gate_breached, "level {level}: and the gate is still shut");
    }
}

/// The bridge is a **way through**.
///
/// `Cell_TryEnter` refuses a `0x40` cell to **both** sides — it is the one
/// castle flag that is not a side test — so a raised drawbridge is a shut gate
/// to the garrison as well. Lowering it leaves plain ground behind, and
/// `strike_castle` then has nothing to catch, which is what lets a sallying
/// garrison walk out.
#[test]
fn the_lowered_bridge_is_a_way_through_where_the_raised_one_was_a_wall() {
    let mut r = siege_battle(3, 21);
    let bridge: Vec<usize> = r
        .field
        .cells
        .iter()
        .enumerate()
        .filter(|(_, c)| c.flags & siege::FLAG_DRAWBRIDGE != 0)
        .map(|(i, _)| i)
        .collect();
    assert!(!bridge.is_empty(), "a stone castle has a drawbridge");
    assert!(r.lower_drawbridge());
    for cell in bridge {
        let c = r.field.cells[cell];
        assert!(!c.impassable(), "cell {cell} is still blocked");
        assert_eq!(c.flags & (siege::FLAG_WALL | siege::FLAG_DRAWBRIDGE), 0);
        assert_eq!(c.surface, siege::SURFACE_GROUND);
    }
}

// ---------------------------------------------------------------------------
// The two accumulators
// ---------------------------------------------------------------------------

