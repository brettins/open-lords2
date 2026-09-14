//! The campaign map: a **scrolling viewport** into the isometric lattice, at
//! one of the original's two zooms.
//!
//! `docs/screens.md` is the decompilation this file is built from; the numbers
//! below are its §1. The short version, because it is the thing this file
//! previously got wrong:
//!
//! > `Map_RenderIso` (`0x0040526E`) does not walk the lattice. It walks a
//! > **window** into it. The start row, the start column, the column count and
//! > the tile pitch are all globals, and `Map_SetZoom` (`0x00451FCC`) sets them
//! > together. **Neither zoom shows the whole map** — the lattice is 65 columns
//! > wide and the far view shows 40 of them.
//!
//! An earlier revision of this file drew all 4,096 tiles at zoom-2 size on one
//! screen and called it the campaign map. That is a view the original does not
//! have, and it reads as a minimap because that is effectively what it is.
//!
//! # The two zooms
//!
//! | | tile art | frame | pitch | visible | viewport |
//! |---|---|---|---|---|---|
//! | [`NEAR`] | `Base1?.pl8` … | 58 × 30 | 60 × 15 | 8 cols × 30 rows | 480 × 450 |
//! | [`FAR`] | `Base2?.pl8` … | 10 × 6 | 12 × 3 | 40 cols × 128 rows | 480 × 384 |
//!
//! `Map_SetZoom` has a third case (26 × 14 tiles) and it is dead code:
//! `g_mapZoom` has three writers in the whole binary and none of them can make
//! it 1. See `docs/screens.md` §2.2 —
//! map tiles either.
//!
//! # Why there is one blitter here and five in the original
//!
//! `Map_DrawTile` dispatches to five unrolled blitters — whole tile, top half,
//! bottom half, left half, right half — and the halves exist so that a tile at
//! the edge of the viewport writes nothing outside it. Reading their write
//! offsets out shows the two columns they drop at the vertical seam land at
//! `viewX − 2 … viewX − 1` on the left and at **478 … 479** on the right, which
//! is under the panel. So a plain blit clipped to `x ∈ [viewX, 478)`,
//! `y ∈ [24, bottom)` writes exactly the same pixels. That derivation is
//! `docs/screens.md` §1.4 and it is asserted, not assumed, in the tests below.
//!
//! # One measured difference from the original
//!
//! The original's tile blitters are straight copies: a diamond pixel holding
//! palette index 0 is written, and comes out black. Ours skips it, because
//! `DecodedFrame::opaque` is `index != 0` and cannot tell "index 0 inside the
//! diamond" from "outside the diamond", where the original writes nothing at
//! all. Measured over the ten campaign tile banks: **4,600 of 436,176 diamond
//! body pixels are index 0**, 4,425 of them in `Base1a.pl8`. Those pixels show
//! the tile behind instead of black. Fixing it needs the frame's shape and
//! overhang, which `DecodedFrame` does not carry.

mod view;
pub use view::*;
mod assets;
pub use assets::*;

mod terrain;
pub use terrain::*;
mod render;
pub use render::*;

use l2_formats::maps::{MapSlot, Plane, LATTICE_H, LATTICE_W, PLANE_DIM};

use crate::canvas::{Canvas, Clip, Tags};
use crate::sheet::Sheet;

/// Height of the menu bar, and the top of the map viewport. `Screen_DrawMenuBar`
/// fills 640 × 24, and `Map_DrawPathMarker` clips the map to
/// `Clip_Vertical(0x18, …)`.
pub const TOP_BAR_H: i32 = 24;

/// Left edge of the right-hand panel, and the map's right clip.
/// `Map_DrawPathMarker` calls `Clip_Horizontal(g_mapViewX, 0x1DE)`; the panel's
/// `Misc_cty` frames are 162 wide and are drawn at 478, and 478 + 162 = 640.
pub const PANEL_X: i32 = 478;
pub const PANEL_W: i32 = 162;

/// The bits of the plane-1 byte that select a tile bank: `bank = (b & 0x1c) >> 2`.
/// `Map_ResolvePick` (`0x0046D5FE`) tests `(tile.bank & 0x1c) == 4`, and
/// `FUN_004063C1` switches on the same mask.
pub const BANK_MASK: u8 = 0x1c;

/// The palette `Screen_DrawCampaign` installs: `Palette_Set(g_paletteBase01)`,
/// and `0x005691F0` is entry 0 of the startup preload table, `Base01.256`.
pub const PALETTE: &str = "Base01.256";

/// How many seasons the artwork has: `g_season` is 1 … 4 and
/// `Gfx_LoadCountyMode` strides eight resource-table entries per season.
pub const SEASONS: usize = 4;

/// **The season letter on a bank's filename, in `g_season` order.**
///
/// `Gfx_LoadCountyMode` takes eight consecutive entries from
/// `g_resourceTable` at `(g_season - 1) * 8`, and entries 0, 8, 16 and 24 are
/// `base1a`, `base1b`, `base1c` and `base1d` — so season 1 is `a` and season 4
/// is `d`, matching `l2_kingdom::tables::Season`'s `Spring = 1 … Winter = 4`.
///
/// **The artwork says the same thing independently.** Measured over the sixteen
/// grass frames (6 … 21) of each `Base1?.pl8` in the shipped install, with
/// "green" meaning `g > r + 8 && g > b + 8`:
///
/// | file | green pixels | mean luminance |
/// |---|---:|---:|
/// | `Base1a` | 99.1 % | 113 |
/// | `Base1b` | 68.0 % | 108 |
/// | `Base1c` | **0.3 %** | 112 |
/// | `Base1d` | 37.9 % | **135** |
///
/// `c` has no green grass and no green woodland — that is autumn — and `d` is
/// far the brightest, which is snow. The order runs spring, summer, autumn,
/// winter, which is what the loader's arithmetic already said. **[V]**
pub const SEASON_SUFFIX: [char; SEASONS] = ['a', 'b', 'c', 'd'];

/// `g_season` (1 … 4) as an index into [`Zoom::banks`].
///
/// `Gfx_LoadCountyMode` guards with `if (0 < g_season && g_season < 5)` and
/// otherwise leaves the base at zero, so an out-of-range season draws the
/// **spring** set. This clamps for the same reason.
pub fn season_slot(season: u8) -> usize {
    if (1..=SEASONS as u8).contains(&season) {
        season as usize - 1
    } else {
        0
    }
}

/// The 65 × 129 screen lattice, built once per map slot.
///
/// Cells either name a map tile or hold a background frame index for the
/// off-map surround — which is what the map file's 65 × 129 tail is, and what
/// `Map_LoadLattice` stores as `0x0FFF0000 + byte`.
pub struct Lattice {
    /// Tile index + 1, or 0 for a surround cell.
    tiles: Vec<u16>,
    /// The surround cell's `base` bank frame index, from the file's tail.
    surround: Vec<u8>,
}

impl Lattice {
    pub fn build(map: &MapSlot) -> Lattice {
        let n = LATTICE_W * LATTICE_H;
        let mut tiles = vec![0u16; n];
        let surround = map.lattice()[..n].to_vec();
        for y in 0..PLANE_DIM {
            for x in 0..PLANE_DIM {
                let (row, col) = tile_to_cell(x, y);
                tiles[row as usize * LATTICE_W + col as usize] = (y * PLANE_DIM + x) as u16 + 1;
            }
        }
        Lattice { tiles, surround }
    }

    /// The map tile at a lattice cell, or `None` for the off-map surround.
    pub fn tile(&self, row: i32, col: i32) -> Option<(usize, usize)> {
        let i = self.index(row, col)?;
        let t = self.tiles[i];
        (t != 0).then(|| {
            let t = (t - 1) as usize;
            (t % PLANE_DIM, t / PLANE_DIM)
        })
    }

    /// The background frame index for a surround cell.
    pub fn surround(&self, row: i32, col: i32) -> u8 {
        self.index(row, col).map_or(0, |i| self.surround[i])
    }

    fn index(&self, row: i32, col: i32) -> Option<usize> {
        (row >= 0 && col >= 0 && row < LATTICE_H as i32 && col < LATTICE_W as i32)
            .then(|| row as usize * LATTICE_W + col as usize)
    }
}

/// **Tile graphics the game rewrites after the map file is read.**
///
/// `L2_maps.dat` is not what the original renders. `Counties_PlaceSites`
/// (`0x00468D4F`) runs over every county at load and stamps new bank/frame
/// bytes into the runtime tile records, and the population pass re-stamps some
/// of them **every season** — so the on-disk bytes for those tiles are a
/// placeholder that the original never shows on screen.
///
/// The one that matters, and the one that sent a player looking for his town:
/// a county town's 2 × 2 block is stored as `Town1a.pl8` frames 0 … 3, and
/// **`Town1a.pl8` frames 0 … 3 are the quarry artwork** — four dark excavated
/// pits. `County_PlaceResourceSites` is the proof: it reads that same bank and
/// treats frame 0 as the stone quarry, 20 as wood and 30 as iron. The town is
/// re-stamped to frames 47 … 50, 51 … 54 or 55 … 58 by the county's population,
/// and until it is, a town renders as four quarries.
///
/// A sparse plane, because the map file is
/// the user's own and this crate has no business holding a mutated copy of it:
/// `None` everywhere means "draw the file", and the caller fills in only the
/// tiles it can account for.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Overrides {
    /// `(bank byte, frame)` per tile index `y * 64 + x`, empty when nothing is
    /// overridden at all.
    at: Vec<Option<(u8, u8)>>,
}

impl Overrides {
    pub fn new() -> Overrides {
        Overrides { at: Vec::new() }
    }

    pub fn is_empty(&self) -> bool {
        self.at.is_empty()
    }

    /// Override one tile. `bank` is the **plane-1 byte**, not the bank index —
/// the same value the file stores, so a caller copies.
    pub fn set(&mut self, x: usize, y: usize, bank: u8, frame: u8) {
        if x >= PLANE_DIM || y >= PLANE_DIM {
            return;
        }
        if self.at.is_empty() {
            self.at = vec![None; PLANE_DIM * PLANE_DIM];
        }
        self.at[y * PLANE_DIM + x] = Some((bank, frame));
    }

    pub fn get(&self, x: usize, y: usize) -> Option<(u8, u8)> {
        if x >= PLANE_DIM || y >= PLANE_DIM {
            return None;
        }
        *self.at.get(y * PLANE_DIM + x)?
    }
}

// -------------------------------------------------------------- field crops

#[cfg(test)]
mod tests {
    use super::*;

    /// `Map_SetZoom` derives `g_mapViewRight = pitch*cols + viewX` and it comes
    /// out **480 at every zoom**, which is what leaves 160 pixels for the right
    /// column. This is the single arithmetic fact the whole layout rests on, so
/// it is asserted from the constants.
    #[test]
    fn every_zoom_gives_the_map_the_same_480_pixels_and_the_panel_the_rest() {
        for z in ZOOMS {
            assert_eq!(z.right(), 480, "zoom {} width", z.id);
        }
        assert_eq!(PANEL_X + PANEL_W, 640, "the panel's own frames reach the screen edge");
        // The dead middle zoom, included because it is the third data point
// that makes 480 an intention: 28*17 + 4.
        assert_eq!(28 * 17 + 4, 480);
    }

    /// `docs/screens.md` §1.2, the resolution of `maps-layers.md`'s open
    /// 58-versus-60 discrepancy: the pitch is the artwork's width plus two and
    /// the row step is half its height, at both zooms.
    #[test]
    fn the_pitch_is_the_frame_width_plus_two_and_the_row_step_is_half_its_height() {
        for z in ZOOMS {
            assert_eq!(z.pitch, z.tile_w + 2, "zoom {} pitch", z.id);
            assert_eq!(z.row_step, z.tile_h / 2, "zoom {} row step", z.id);
            assert_eq!(z.half_pitch, z.pitch / 2, "zoom {} half pitch", z.id);
        }
    }

    /// The visible band starts at 24 at both zooms — the top of the map, and
    /// the bottom of the menu bar — and ends where `g_mapViewBottom` says.
    #[test]
    fn the_viewport_starts_under_the_menu_bar_and_ends_where_the_binary_says() {
        assert_eq!(NEAR.top(), TOP_BAR_H);
        assert_eq!(FAR.top(), TOP_BAR_H);
        assert_eq!(NEAR.bottom(), 474, "Clip_Vertical(0x18, 0x1DA) in Map_DrawPathMarker");
        assert_eq!(FAR.bottom(), 408);
    }

    /// **The derivation the single clipped blitter rests on** (`screens.md`
    /// §1.4). The original's half-tile blitters drop the two columns either
    /// side of the vertical seam; for the clip rectangle to be equivalent,
    /// those columns must land outside it at both ends and at both zooms.
    ///
/// If the clip were 480 the right-hand half of this fails,
    /// which is the point: the two dropped columns of the rightmost offset-row
    /// tile land on 478 and 479 exactly.
    #[test]
    fn the_clip_rectangle_swallows_exactly_the_columns_the_half_blitters_drop() {
        for z in ZOOMS {
            let clip = z.clip();
            // The leftmost tile of an offset row sits half a pitch left of the
            // viewport; its dropped columns are w/2-1 and w/2.
            let left_origin = z.view_x - z.half_pitch;
            for c in [z.tile_w / 2 - 1, z.tile_w / 2] {
                assert!(
                    left_origin + c < clip.x0,
                    "zoom {}: dropped column {c} of the left tile is inside the clip",
                    z.id
                );
            }
            // Its right-half pixels start at w/2+1 and must be inside.
            assert_eq!(left_origin + z.tile_w / 2 + 1, clip.x0, "zoom {} left seam", z.id);

            // The rightmost tile of an offset row.
            let right_origin = z.view_x - z.half_pitch + z.cols * z.pitch;
            for c in [z.tile_w / 2 - 1, z.tile_w / 2] {
                assert!(
                    right_origin + c >= clip.x1,
                    "zoom {}: dropped column {c} of the right tile is inside the clip",
                    z.id
                );
            }
            // And its last kept column is the last pixel the clip allows.
            assert_eq!(right_origin + z.tile_w / 2 - 2, clip.x1 - 1, "zoom {} right seam", z.id);
        }
    }

    /// `Map_ClampScroll`, and the parity the render walk depends on.
    #[test]
    fn the_scroll_origin_is_clamped_to_the_lattice_and_kept_even() {
        assert_eq!(Viewport::new(-5, -5).clamped(&NEAR), Viewport::new(0, 0));
        assert_eq!(
            Viewport::new(999, 999).clamped(&NEAR),
            Viewport::new(98, 56),
            "0x80 - 30 rows, 0x40 - 8 cols"
        );

        // An odd row would swap which lattice rows are the half-offset ones.
        assert_eq!(Viewport::new(41, 3).clamped(&NEAR).row, 40);

        // The far zoom's whole height already fits, so it has no vertical
        // scroll at all: 0x80 - 128 = 0.
        assert_eq!(FAR.max_row(), 0);
        assert_eq!(Viewport::new(50, 50).clamped(&FAR), Viewport::new(0, 24));
    }

    /// **`Map_DrawArmies(1)`, and nothing else in the walk gets it.**
    /// `FUN_004059AF` passes mode 1 for the leftmost column of an offset row
    /// only, and mode 1 adds `-2` where mode 0 adds `g_mapTileHalfStep`. See
    /// [`unit_anchor`].
    #[test]
    fn the_first_figure_of_an_offset_row_stands_two_pixels_left_of_its_tile() {
        // `tile_to_cell(x, y) = (x + y + 1, (x - y + 64) >> 1)`.
        let view = Viewport::new(20, 32);
        assert_eq!(view.row & 1, 0, "the walk starts on an aligned row");
        assert_eq!(tile_to_cell(10, 10), (21, 32), "the offset row's leftmost cell");
        assert_eq!(tile_to_cell(11, 9), (21, 33), "the next column of the same row");
        assert_eq!(tile_to_cell(10, 9), (20, 32), "the aligned row's leftmost cell");

        for z in ZOOMS {
            // mode 1: `g_drawX = g_mapViewX; local_1c += -2`.
            assert_eq!(unit_anchor(view, &z, (10, 10)).0, z.view_x - 2, "zoom {} mode 1", z.id);
            // mode 0, one column along: `g_drawX = viewX + halfStep`, `+ halfStep`.
            assert_eq!(unit_anchor(view, &z, (11, 9)).0, z.view_x + z.pitch, "zoom {} col 1", z.id);
            // mode 0 on the aligned row above it, which keeps its half-step.
            let aligned = unit_anchor(view, &z, (10, 9)).0;
            assert_eq!(aligned, z.view_x + z.half_pitch, "zoom {} aligned", z.id);
            // The two rows are half a pitch apart, and the quirk is 2 px on top
            // of that — stated as a difference so it cannot be read as spacing.
            assert_eq!(aligned - unit_anchor(view, &z, (10, 10)).0, z.half_pitch + 2, "zoom {}", z.id);
        }
    }

    /// The eight directions move by one map tile each, and the far zoom refuses
    /// to scroll at all — `Map_EdgeScroll` returns before touching the origin
    /// when `g_battlePhase == 0 && g_mapZoom == 2`.
    #[test]
    fn scrolling_moves_one_tile_per_step_and_the_far_view_does_not_move() {
        let start = Viewport::new(40, 20);
        assert_eq!(start.scrolled(Dir::N, &NEAR), Some(Viewport::new(38, 20)));
        assert_eq!(start.scrolled(Dir::S, &NEAR), Some(Viewport::new(42, 20)));
        assert_eq!(start.scrolled(Dir::E, &NEAR), Some(Viewport::new(40, 21)));
        assert_eq!(start.scrolled(Dir::W, &NEAR), Some(Viewport::new(40, 19)));
        assert_eq!(start.scrolled(Dir::NW, &NEAR), Some(Viewport::new(38, 19)));
        assert_eq!(start.scrolled(Dir::SE, &NEAR), Some(Viewport::new(42, 21)));

        // A step that changes nothing after clamping reports that it did not
        // move, so the caller does not repaint for nothing.
        assert_eq!(Viewport::new(0, 0).scrolled(Dir::NW, &NEAR), None);
        // ...and at the far zoom nothing moves in any direction.
        for d in DIRS {
            assert_eq!(Viewport::new(0, 10).scrolled(d, &FAR), None, "{d:?}");
        }
    }

    /// The projection, checked as the two identities that would break if the
    /// ±½ column term were rounded the other way.
    #[test]
    fn the_lattice_mapping_steps_a_half_tile_east_and_a_half_tile_south() {
        assert_eq!(tile_to_cell(0, 0), (1, 32));
        let v = Viewport::new(0, 0);
        let at = |x, y| {
            let (r, c) = tile_to_cell(x, y);
            cell_to_screen(v, &NEAR, r, c)
        };
        let (x0, y0) = at(0, 0);
        assert_eq!(at(1, 0), (x0 + NEAR.half_pitch, y0 + NEAR.row_step), "+x is south-east");
        assert_eq!(at(0, 1), (x0 - NEAR.half_pitch, y0 + NEAR.row_step), "+y is south-west");
        assert_eq!(at(1, 1), (x0, y0 + 2 * NEAR.row_step), "and the two cancel");
    }

    /// Centring is the original's `col - 4`, `(row & !1) - 12` — including the
    /// part that is *not* centred, which is worth pinning so nobody "fixes" it.
    #[test]
    fn centring_uses_the_originals_offsets_including_the_asymmetric_one() {
        let v = Viewport::centred_on_cell(60, 30, &NEAR);
        assert_eq!(v, Viewport::new(48, 26));
        // Horizontally the cell lands in the middle of the eight columns...
        assert_eq!(30 - v.col, NEAR.cols / 2);
        // ...vertically it does not: 12 of 30 rows, not 15.
        assert_eq!(60 - v.row, 12);
        assert_ne!(60 - v.row, NEAR.rows / 2, "the original does not centre vertically");

        // An odd lattice row is pulled to the even one below it.
        assert_eq!(Viewport::centred_on_cell(61, 30, &NEAR).row, 48);
    }

    /// Neither zoom can show the whole map. This is the claim the previous
    /// painter violated, so it gets its own test.
    #[test]
    fn no_zoom_shows_the_whole_lattice() {
        for z in ZOOMS {
            assert!(z.cols < LATTICE_W as i32, "zoom {} shows {} of 65 columns", z.id, z.cols);
        }
        // The near view shows an eighth of the width; the far one shows 40 of
        // 64 usable columns and cannot scroll, so 24 columns are unreachable.
        assert_eq!(NEAR.cols, 8);
        assert_eq!(FAR.cols, 40);
        assert_eq!(FAR.max_col(), 24);
    }

    /// A cell outside the viewport has no centre, so a marker for it is not
    /// drawn somewhere arbitrary.
    #[test]
    fn a_tile_outside_the_viewport_has_no_screen_centre() {
        let v = Viewport::new(40, 20);
        // Tile (0,0) is lattice row 1, far above a viewport starting at row 40.
        assert_eq!(tile_centre(v, &NEAR, 0, 0), None);
        // A tile inside it does have one, and it is inside the clip.
        let inside = (0..PLANE_DIM)
            .flat_map(|y| (0..PLANE_DIM).map(move |x| (x, y)))
            .find_map(|(x, y)| tile_centre(v, &NEAR, x, y))
            .expect("something is on screen");
        assert!(NEAR.clip().contains(inside.0, inside.1));
    }
}


