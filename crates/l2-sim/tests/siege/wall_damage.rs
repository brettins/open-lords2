#![allow(unused_imports)]
use super::*;
use super::handlers::*;
use super::breaches_and_assaults::*;
use super::outcomes::*;
use super::player_tactics::*;
use l2_sim::ai::{handler_for, TABLE_FIELD, TABLE_SIEGE_ATT, TABLE_SIEGE_DEF};
use l2_sim::runner::{ASSAULT_REPEATS_BELOW_LEVEL, ASSAULT_REPEAT_SCORE};
use l2_sim::siege::{
    self, SiegeState, FLAG_KEEP, FLAG_WALL, SURFACE_BAILEY, SURFACE_RAMPART_WALK, SURFACE_WALL,
};
use l2_sim::{BattleRunner, End, Muster, Troop, SIDE_A, SIDE_B};

/// **The four surfaces a siege turns on, and the two that were the wrong way
/// round.**
///
/// This test used to assert `SURFACE_BREACH == 4` and `SURFACE_RAMPART == 5`
/// and it was wrong about the first: nothing in `Lords2.exe` writes surface 4
/// outside the castle build's flood classifier, and a breach leaves **5**
/// (`Wall_Smash`) or **9** (`Wall_Collapse`). `docs/decisions.md`
/// `C99`.
#[test]
fn a_smashed_wall_joins_the_bailey_and_a_collapsed_one_does_not() {
    assert_eq!(SURFACE_WALL, 8, "an intact wall, the cell that carries 0x20");
    assert_eq!(SURFACE_BAILEY, 5, "the bailey — and what Wall_Smash leaves");
    assert_eq!(SURFACE_RAMPART_WALK, 4, "what Siege_FindCellSurface4 hunts");
    assert_eq!(siege::SURFACE_COLLAPSED, 9, "what Wall_Collapse leaves");

    // The accumulator is chosen by where the attacker *stands*, and 5 is the
    // one that picks the rampart's 5,000.
    let mut s = SiegeState::castle(2);
    for _ in 0..siege::RAMPART_HITS - 1 {
        siege::strike_wall(&mut s, SURFACE_BAILEY, false);
    }
    assert_eq!(
        siege::strike_wall(&mut s, SURFACE_BAILEY, false),
        siege::WallBlow::RampartBreached
    );
    // Standing on the rampart walk is *not* standing on a 5, so it feeds the
    // gate — which is the whole content of the correction.
    let mut g = SiegeState::castle(2);
    siege::strike_wall(&mut g, SURFACE_RAMPART_WALK, false);
    assert_eq!(g.gate_hits, 1);
    assert_eq!(g.rampart_hits, 0);
}

/// **`Wall_Smash` opens a nine-cell-wide hole, and `Wall_Collapse` opens one
/// cell and leaves a different mark.** They are two routines, not one.
#[test]
fn a_breach_is_nine_cells_wide_and_a_collapse_is_one() {
    let mut field = siege::our_castle(2);
    let wall_before = field.cells.iter().filter(|c| c.flags & FLAG_WALL != 0).count();

    // The south wall of `our_castle(2)` runs along y = 34 through x = 40.
    let opened = siege::smash_walls(&mut field, 40, 34, siege::SMASH_RADIUS);
    assert_eq!(opened, 9, "a 9 x 9 centred on one wall cell meets nine of them");
    let wall_after = field.cells.iter().filter(|c| c.flags & FLAG_WALL != 0).count();
    assert_eq!(wall_before - wall_after, 9);
    for x in 36..=44u32 {
        let c = &field.cells[34 * 80 + x as usize];
        assert_eq!(c.flags & FLAG_WALL, 0, "({x}, 34) is open");
        assert_eq!(c.surface, SURFACE_BAILEY, "and joins the bailey");
        assert_eq!(
            c.elevation,
            siege::WALL_ELEVATION,
            "and keeps the wall's own height — Wall_Smash does not flatten"
        );
    }

    // A collapse is one cell, at ground level, on surface 9, and it bills the
    // orthogonal neighbours still at 5.
    let mut field = siege::our_castle(2);
    let mut s = SiegeState::castle(2);
    let cell = 34 * 80 + 40;
    let billed = siege::collapse_wall(&mut field, &mut s, cell);
    assert_eq!(field.cells[cell].surface, siege::SURFACE_COLLAPSED);
    assert_eq!(field.cells[cell].elevation, siege::BREACH_ELEVATION);
    assert_eq!(field.cells[cell].flags & FLAG_WALL, 0);
    assert_eq!(field.cells[cell - 80].flags & FLAG_WALL, 0, "north of it is the bailey");
    assert_eq!(billed, 1, "one neighbour — the bailey cell inside it");
    assert_eq!(s.wall_damage, 1, "and the bill is the same count");
    assert_eq!(
        field.cells[cell + 1].flags & FLAG_WALL,
        FLAG_WALL,
        "the wall either side of it still stands"
    );
}

// ---------------------------------------------------------------------------
// The headline: a besieger who can win
// ---------------------------------------------------------------------------

