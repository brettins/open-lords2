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

/// **The four surfaces a siege turns on
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

/// **Damage is a count on a surface, not the `0x20` flag.**
///
/// `Missile_Step`'s class-3 gate (`0x00492C8B`, `docs/battle.md` §17.7) is
/// `elevation != 0 && surface == 4 && frame > 2`
/// into the cell's byte `+0`. Ours read `flags & 0x20`, which no surface-4 cell
/// carries — and `UnitOrder_SiegeAttCatapult` (`0x0048DB84`) aims at surface 4
/// (`nearest_surface(.., 4, ..)`, the original's `Siege_FindCellSurface4`,
/// `0x00496566`). Every aimed shot therefore counted nothing.
///
/// Ablation: put `flags & FLAG_WALL != 0` back in front of
/// `siege::shot_damages_wall` and the walk rows here go false.
#[test]
fn a_shot_counts_on_masonry_by_elevation_and_not_on_the_wall_flag() {
    let field = siege::our_castle(2);
    let mut walk = 0;
    for c in &field.cells {
        if c.surface == SURFACE_RAMPART_WALK {
            assert_eq!(c.flags & FLAG_WALL, 0, "the walk carries no 0x20");
            assert!(siege::shot_damages_wall(c), "and a shot still counts on it");
            assert_eq!(c.terrain, 1, "the builder's seed, so fifteen shots bring it down");
            walk += 1;
        }
    }
    assert!(walk > 0, "our_castle(2) has a rampart walk at all");

    let mut c = field.cells[34 * 80 + 40].clone();
    assert_eq!(c.flags & FLAG_WALL, FLAG_WALL, "the south wall at (40, 34)");
    assert!(siege::shot_damages_wall(&c), "the curtain counts too");
    c.flags &= !FLAG_WALL;
    assert!(siege::shot_damages_wall(&c), "and the flag is not what said so");

    // The original's own `elevation != 0`
    // masonry: the bailey, and the rubble a collapse leaves at elevation 0.
    let mut low = field.cells[34 * 80 + 40].clone();
    low.elevation = 0;
    assert!(!siege::shot_damages_wall(&low));
    for s in [SURFACE_BAILEY, siege::SURFACE_COLLAPSED] {
        let mut o = field.cells[34 * 80 + 40].clone();
        o.surface = s;
        assert!(!siege::shot_damages_wall(&o), "surface {s} is not masonry");
    }
    // `Wall_Smash` joins the cell to the bailey, so it stops counting because
    // its surface changed —.
    let mut smashed = siege::our_castle(2);
    siege::smash_walls(&mut smashed, 40, 34, siege::SMASH_RADIUS);
    assert!(!siege::shot_damages_wall(&smashed.cells[34 * 80 + 40]));
}

/// **And in a real siege the cell the catapult aims at is the one that takes
/// the damage.** `UnitOrder_SiegeAttCatapult` sends the engine at a surface-4
/// cell and surface 4 carries no `0x20`, so under the flag gate every shot that
/// arrived where it was aimed counted nothing: the walk's byte `+0` stayed at
/// the builder's seed of 1 for the whole siege. Ablation: restore
/// `flags & FLAG_WALL != 0 && elevation != 0` in `BattleRunner::step_missile`
/// and `walk` here goes to 0 while the curtain's count survives.
#[test]
fn a_catapult_raises_the_cell_byte_of_the_wall_it_is_aimed_at() {
    let mut r = storming_party(2);
    // Masonry only: a ditch's byte is the *fill* counter, seeded at the water
    // id 11 and raised by `BattleMan_StateFillMoat`.
    let damaged = |r: &BattleRunner, s: u8| {
        r.field.cells.iter().filter(|c| c.terrain > 1 && c.surface == s).count()
    };
    let mut walk = 0;
    for _ in 0..60 {
        r.run(500);
        walk = damaged(&r, SURFACE_RAMPART_WALK);
        if walk > 0 {
            break;
        }
    }
    assert!(r.sim.cues.walls_struck() > 0, "a shot was counted against a wall");
    assert!(walk > 0, "and the flag-free walk it was aimed at is what holds the count");
}

// ---------------------------------------------------------------------------
// The headline: a besieger who can win
// ---------------------------------------------------------------------------

