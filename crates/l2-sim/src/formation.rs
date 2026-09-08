//! Unit geometry: how many figures a unit holds, and where they stand.
//!
//! Two different things use this. **Raising** an army walks the eleven troop
//! types and splits each into units of at most [`MAX_FIGURES_PER_UNIT`] figures
//! (`Battle_RaiseSide`, `0x0047FEA7`), laying each unit's figures out around its
//! deployment slot with [`offset_x`] / [`offset_y`] (`BattleUnit_Create`,
//! `0x00480662`). **Reforming** — the every-500-frame formation tidy-up of
//! `docs/battle-ai.md` §5 — rebuilds the same rectangle around the unit's
//! *destination* instead ([`Rect`], `Formation_ComputeRect`, `0x0048A1C9`).
//!
//! # The tables
//!
//! All four are read out of `Lords2.exe` rather than out of a listing. **[V]**
//! `g_troopBattleStats` (`0x004D96D0`, 11 rows of 5 ints) supplies three of
//! them and `tools/oracle/tables.ps1` prints it:
//!
//! ```text
//!               maxFigures footprint    rowMax weaponClass moveDelay
//! Peasants              12         1         6           0         2
//! Crossbowmen            8         1         6           2         2
//! ...
//! Oil                    1         2         1           0         5
//! ```
//!
//! The fourth, [`TYPE_PRIORITY`], is `g_formationTypePriority` (`0x004D98C8`),
//! read the same way.
//!
//! **They are also *checked* against those bytes**, which they were not until
//! recently: `crates/l2-sim/tests/oracle.rs` maps both addresses through the PE
//! section headers and compares all 66 entries. Until then the only guard was a
//! unit test that recited the same literals a second time, under a name that
//! said "oracle" — `docs/decisions.md` C12's shape, three years of transcribed
//! numbers behind it, and the script that could have caught it printing to a
//! console nobody read.
//!
//! # Determinism
//!
//! Integer arithmetic and fixed-size tables only. Every division here is the
//! original's, including the ones that truncate toward zero on a negative
//! numerator, which C and Rust agree about.

use crate::figure::Side;
use crate::troop::Troop;

/// Figures one unit may hold, by [`Troop::index`]. **[V]** column 0 of
/// `g_troopBattleStats`.
///
/// This is the number the previous driver did not have, and its absence is why
/// deployment packed four figures per marker slot: the original packs a *unit*
/// per slot, and this is how big a unit gets before the next one starts.
pub const MAX_FIGURES_PER_UNIT: [u16; 11] = [12, 8, 8, 8, 10, 12, 6, 2, 2, 2, 1];

/// Cells one figure occupies on a side, by [`Troop::index`]. **[V]** column 1.
/// Infantry are 1, siege engines 3, oil 2.
pub const FOOTPRINT: [i32; 11] = [1, 1, 1, 1, 1, 1, 1, 3, 3, 3, 2];

/// Figures per formation row, by [`Troop::index`]. **[V]** column 2.
pub const ROW_MAX: [i32; 11] = [6, 6, 4, 4, 5, 6, 6, 2, 2, 2, 1];

/// Which troop type dictates a mixed unit's geometry: the highest priority
/// wins. **[V]** `g_formationTypePriority` (`0x004D98C8`) — siege engines
/// outrank everyone, then knights, pikemen, crossbowmen, archers, swordsmen,
/// macemen, peasants.
pub const TYPE_PRIORITY: [i32; 11] = [0, 4, 1, 2, 5, 3, 6, 10, 8, 9, 7];

/// Weapon class, by [`Troop::index`]. **[V]** column 3 of `g_troopBattleStats`:
/// 1 bow, 2 crossbow, 3 catapult, 0 none.
///
/// `Formation_SendFigure` branches on `0 < class < 3` (a figure that closes to
/// shoot) and on `class > 2` (a catapult that aims), so the raw number matters
/// rather than just "has a missile weapon".
pub const WEAPON_CLASS: [u8; 11] = [0, 2, 0, 0, 0, 1, 0, 3, 0, 0, 0];

/// How many rows a unit of `figures` figures of `troop` forms up in.
///
/// `BattleUnit_Create`: `(figures - 1) / rowMax + 1`. Note the name: this is
/// the value the original passes to [`offset_x`] and [`offset_y`] as `rows`,
/// and those two treat it as the **column height** — figure `i` sits in column
/// `i / rows`, row `i % rows`. So a full twelve-figure peasant unit with a
/// `rowMax` of 6 forms two rows of six, not six rows of two.
pub fn rows_for(troop: Troop, figures: usize) -> i32 {
    let row_max = ROW_MAX[troop.index()];
    (figures.max(1) as i32 - 1) / row_max + 1
}

/// `Formation_OffsetX` (`0x00481878`): `(i / rows) * footprint`.
///
/// The original is a five-way ladder rather than a division, and it **caps at
/// five**: `rows` of six or more behaves as five. Reproduced, because a unit
/// of more than 25 figures is reachable (oil holds one figure per unit, but a
/// mixed 80-figure army is not) and the cap changes the shape.
pub fn offset_x(footprint: i32, rows: i32, i: i32) -> i32 {
    (i / rows.clamp(1, 5)) * footprint
}

/// `Formation_OffsetY` (`0x00481906`): `(i % rows) * footprint`, **negated for
/// side 0** so the two armies form up facing each other.
///
/// `rows < 2` gives a flat line, and the same five-row cap applies.
pub fn offset_y(footprint: i32, rows: i32, i: i32, side: Side) -> i32 {
    if rows < 2 {
        return 0;
    }
    let sign = if side == 0 { -1 } else { 1 };
    (i % rows.clamp(1, 5)) * sign * footprint
}

/// A unit's formation rectangle: where its top-left slot sits and how wide it
/// is. Slot `i` is at `(origin.0 + (i % cols) * footprint, origin.1 + (i / cols) * footprint)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub origin_x: i32,
    pub origin_y: i32,
    pub cols: i32,
    pub footprint: i32,
}

impl Rect {
    /// Cell of the `i`-th slot.
    pub fn slot(&self, i: usize) -> (i32, i32) {
        let i = i as i32;
        (
            (i % self.cols) * self.footprint + self.origin_x,
            (i / self.cols) * self.footprint + self.origin_y,
        )
    }
}

/// `Formation_ComputeRect` (`0x0048A1C9`): centre the rectangle on `target` and
/// clamp it to the 80 × 80 map.
///
/// Two details are the original's and look like mistakes until they are checked
/// against the disassembly:
///
/// * the **width** collapses to 1 for a single-figure unit even when its
///   footprint is 3, so a lone catapult forms up on one cell;
/// * the two axes clamp **differently**. `x` is pulled back so the right edge
///   lands on 79, while `y` is pulled back by `bottom - 79` where `bottom` is
///   `origin + depth` rather than `origin + depth - 1` — one cell more
///   generous. Reproduced rather than symmetrised.
///
/// The original also forces `cols = 2` when unit byte `+0x09` is 1. That byte is
/// not modelled here (nothing we have read writes it), so that branch is absent.
pub fn compute_rect(target: (i16, i16), figures: usize, footprint: i32, cols: i32) -> Rect {
    let figures = figures as i32;
    let cols = cols.min(figures).max(1);
    let mut width = cols * footprint;
    if figures == 1 {
        width = 1;
    }
    let depth = ((figures - 1) / cols) * footprint;

    let mut origin_x = target.0 as i32 - (width - 1) / 2;
    let right = origin_x + width - 1;
    let mut origin_y = target.1 as i32 - (depth - 1) / 2;
    let bottom = origin_y + depth;

    if origin_x < 0 {
        origin_x = 0;
    } else if right > 0x4F {
        origin_x -= origin_x + width - 0x50;
    }
    if origin_y < 0 {
        origin_y = 0;
    } else if bottom > 0x4F {
        origin_y -= bottom - 0x4F;
    }
    Rect { origin_x, origin_y, cols, footprint }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figure::{SIDE_A, SIDE_B};

    /// The shape of the three `g_troopBattleStats` columns this module carries:
    /// which troops share a unit size, which occupy more than one cell, and
    /// which carry a weapon.
    ///
    /// # This is not the oracle, and it used to say it was
    ///
    /// It was called `the_unit_size_and_footprint_tables_match_the_oracle_reading`
    /// and its doc comment cited "the prose `docs/battle.md` §8.2a records from
    /// the oracle run". It opens nothing. Every literal below was copied out of
    /// the same paragraph the constants were copied from, so a transcription
    /// error would have had to be made twice on the same afternoon to be caught
    /// — and `assert_eq!(MAX_FIGURES_PER_UNIT[Peasants], 12)` compares a
    /// constant to a second spelling of itself in any case.
    ///
    /// **The oracle is `crates/l2-sim/tests/oracle.rs`**, which reads all 55
    /// entries out of `Lords2.exe` at `0x004D96D0` through the PE section
    /// headers. What is left here is the part that is worth stating in prose:
    /// the *relations* between the rows, which are what a reader needs and what
    /// a reordering of `Troop` would break while the byte-for-byte comparison
    /// still passed.
    #[test]
    fn the_unit_size_and_footprint_tables_have_the_shape_the_module_documents() {
        assert_eq!(MAX_FIGURES_PER_UNIT[Troop::Peasants.index()], 12);
        assert_eq!(MAX_FIGURES_PER_UNIT[Troop::Archers.index()], 12);
        assert_eq!(MAX_FIGURES_PER_UNIT[Troop::Crossbowmen.index()], 8);
        assert_eq!(MAX_FIGURES_PER_UNIT[Troop::Macemen.index()], 8);
        assert_eq!(MAX_FIGURES_PER_UNIT[Troop::Swordsmen.index()], 8);
        assert_eq!(MAX_FIGURES_PER_UNIT[Troop::Pikemen.index()], 10);
        assert_eq!(MAX_FIGURES_PER_UNIT[Troop::Knights.index()], 6);
        assert_eq!(MAX_FIGURES_PER_UNIT[Troop::Oil.index()], 1);
        for t in [Troop::Catapults, Troop::SiegeTowers, Troop::BatteringRams] {
            assert_eq!(MAX_FIGURES_PER_UNIT[t.index()], 2);
            assert_eq!(FOOTPRINT[t.index()], 3);
        }
        assert_eq!(FOOTPRINT[Troop::Oil.index()], 2);
        for t in [
            Troop::Peasants,
            Troop::Crossbowmen,
            Troop::Macemen,
            Troop::Swordsmen,
            Troop::Pikemen,
            Troop::Archers,
            Troop::Knights,
        ] {
            assert_eq!(FOOTPRINT[t.index()], 1, "{t:?} is one cell");
        }
        // The weapon classes that pin the row order of the whole table.
        assert_eq!(WEAPON_CLASS[Troop::Crossbowmen.index()], 2);
        assert_eq!(WEAPON_CLASS[Troop::Archers.index()], 1);
        assert_eq!(WEAPON_CLASS[Troop::Catapults.index()], 3);
        assert_eq!(WEAPON_CLASS[Troop::Swordsmen.index()], 0);

        // Relations rather than values: exactly three troop types carry a
        // weapon, exactly four take more than one cell, and no unit is empty.
        assert_eq!(WEAPON_CLASS.iter().filter(|&&c| c != 0).count(), 3);
        assert_eq!(FOOTPRINT.iter().filter(|&&f| f > 1).count(), 4);
        assert!(MAX_FIGURES_PER_UNIT.iter().all(|&n| n >= 1));
        assert!(ROW_MAX.iter().zip(MAX_FIGURES_PER_UNIT).all(|(&r, n)| r as u16 <= n));
    }

    /// Siege engines dictate a mixed unit's shape, and peasants never do.
    #[test]
    fn the_formation_priority_puts_siege_engines_first_and_peasants_last() {
        let best = |ts: &[Troop]| -> Troop {
            *ts.iter().max_by_key(|t| TYPE_PRIORITY[t.index()]).unwrap()
        };
        assert_eq!(best(&[Troop::Peasants, Troop::Knights]), Troop::Knights);
        assert_eq!(best(&[Troop::Knights, Troop::Catapults]), Troop::Catapults);
        assert_eq!(best(&[Troop::Peasants, Troop::Archers]), Troop::Archers);
        assert_eq!(TYPE_PRIORITY[Troop::Peasants.index()], 0);
        let mut sorted = TYPE_PRIORITY;
        sorted.sort_unstable();
        assert_eq!(sorted, [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10], "a permutation, no ties");
    }

    /// A full peasant unit is two rows of six, and the two sides' rows grow
    /// away from each other.
    #[test]
    fn a_full_unit_forms_up_in_rows_that_face_the_enemy() {
        let rows = rows_for(Troop::Peasants, 12);
        assert_eq!(rows, 2);
        let fp = FOOTPRINT[Troop::Peasants.index()];
        let cells4: Vec<(i32, i32)> = (0..12)
            .map(|i| (offset_x(fp, rows, i), offset_y(fp, rows, i, SIDE_B)))
            .collect();
        assert_eq!(cells4[0], (0, 0));
        assert_eq!(cells4[1], (0, 1), "second figure is behind the first");
        assert_eq!(cells4[2], (1, 0), "third starts the next column");
        assert_eq!(cells4[11], (5, 1), "six columns of two");
        // Side 0's second rank grows the other way, so the two bodies of men
        // face rather than trail each other.
        assert_eq!(offset_y(fp, rows, 1, SIDE_A), -1);
        assert_eq!(offset_y(fp, rows, 1, SIDE_B), 1);
        // One row is a flat line whichever side it is.
        assert_eq!(offset_y(fp, 1, 3, SIDE_A), 0);
    }

    /// The five-row cap is in the original's ladder, not an accident of ours.
    #[test]
    fn more_than_five_rows_behaves_as_five() {
        assert_eq!(offset_x(1, 9, 5), offset_x(1, 5, 5));
        assert_eq!(offset_y(1, 9, 7, SIDE_B), offset_y(1, 5, 7, SIDE_B));
        assert_eq!(offset_x(1, 6, 12), 2, "twelve into columns of five");
    }

    #[test]
    fn the_rectangle_is_centred_on_the_destination() {
        // Six figures, two columns: 2 wide, 2 deep.
        let r = compute_rect((40, 40), 6, 1, 2);
        assert_eq!(r.cols, 2);
        assert_eq!(r.slot(0), (r.origin_x, r.origin_y));
        assert_eq!(r.slot(1), (r.origin_x + 1, r.origin_y));
        assert_eq!(r.slot(2), (r.origin_x, r.origin_y + 1));
        assert!((r.origin_x - 40).abs() <= 1 && (r.origin_y - 40).abs() <= 1);
    }

    /// The clamp, and the asymmetry between the axes that is the original's.
    #[test]
    fn the_rectangle_is_pulled_back_inside_the_map() {
        let r = compute_rect((79, 79), 12, 1, 6);
        assert!(r.origin_x >= 0 && r.origin_y >= 0);
        for i in 0..12 {
            let (x, y) = r.slot(i);
            assert!((0..80).contains(&x), "slot {i} x {x}");
            assert!((0..80).contains(&y), "slot {i} y {y}");
        }
        // x lands the right edge exactly on 79; y stops one short, because the
        // original measures the bottom from origin + depth rather than
        // origin + depth - 1.
        assert_eq!(r.origin_x + r.cols * r.footprint - 1, 79);
        let depth = (12 - 1) / r.cols * r.footprint;
        assert_eq!(r.origin_y + depth, 79);
    }

    /// A lone catapult occupies one cell rather than its three-cell footprint.
    #[test]
    fn a_single_figure_unit_is_one_cell_wide_whatever_its_footprint() {
        let r = compute_rect((40, 40), 1, 3, 2);
        assert_eq!(r.origin_x, 40, "width 1, so the origin is the target");
        assert_eq!(r.slot(0), (40, 40));
    }
}
