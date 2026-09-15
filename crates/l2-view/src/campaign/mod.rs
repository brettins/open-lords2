//! > `Map_RenderIso` (`0x0040526E`) does not walk the lattice. It walks a
//! > **window** into it. The start row, the start column, the column count and
//! > the tile pitch are all globals, and `Map_SetZoom` (`0x00451FCC`) sets them
//! > together. **Neither zoom shows the whole map** — the lattice is 65 columns
//! > wide and the far view shows 40 of them.

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

pub const TOP_BAR_H: i32 = 24;

pub const PANEL_X: i32 = 478;
pub const PANEL_W: i32 = 162;

/// `Map_ResolvePick` (`0x0046D5FE`) tests `(tile.bank & 0x1c) == 4`, and
/// `FUN_004063C1` switches on the same mask.
pub const BANK_MASK: u8 = 0x1c;

/// The palette `Screen_DrawCampaign` installs: `Palette_Set(g_paletteBase01)`,
/// and `0x005691F0` is entry 0 of the startup preload table, `Base01.256`.
pub const PALETTE: &str = "Base01.256";

pub const SEASONS: usize = 4;

/// `c` has no green grass and no green woodland — that is autumn — and `d` is
/// far the brightest, which is snow. The order runs spring, summer, autumn,
/// winter, which is what the loader's arithmetic already said. **[V]**
pub const SEASON_SUFFIX: [char; SEASONS] = ['a', 'b', 'c', 'd'];

pub fn season_slot(season: u8) -> usize {
    if (1..=SEASONS as u8).contains(&season) {
        season as usize - 1
    } else {
        0
    }
}

/// Cells either name a map tile or hold a background frame index for the
/// off-map surround — which is what the map file's 65 × 129 tail is, and what
/// `Map_LoadLattice` stores as `0x0FFF0000 + byte`.
pub struct Lattice {
    tiles: Vec<u16>,
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

    pub fn tile(&self, row: i32, col: i32) -> Option<(usize, usize)> {
        let i = self.index(row, col)?;
        let t = self.tiles[i];
        (t != 0).then(|| {
            let t = (t - 1) as usize;
            (t % PLANE_DIM, t / PLANE_DIM)
        })
    }

    pub fn surround(&self, row: i32, col: i32) -> u8 {
        self.index(row, col).map_or(0, |i| self.surround[i])
    }

    fn index(&self, row: i32, col: i32) -> Option<usize> {
        (row >= 0 && col >= 0 && row < LATTICE_H as i32 && col < LATTICE_W as i32)
            .then(|| row as usize * LATTICE_W + col as usize)
    }
}

/// `L2_maps.dat` is not what the original renders. `Counties_PlaceSites`
/// (`0x00468D4F`) runs over every county at load and stamps new bank/frame
/// bytes into the runtime tile records, and the population pass re-stamps some
/// of them **every season** — so the on-disk bytes for those tiles are a
/// placeholder that the original never shows on screen.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Overrides {
    at: Vec<Option<(u8, u8)>>,
}

impl Overrides {
    pub fn new() -> Overrides {
        Overrides { at: Vec::new() }
    }

    pub fn is_empty(&self) -> bool {
        self.at.is_empty()
    }

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


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_zoom_gives_the_map_the_same_480_pixels_and_the_panel_the_rest() {
        for z in ZOOMS {
            assert_eq!(z.right(), 480, "zoom {} width", z.id);
        }
        assert_eq!(PANEL_X + PANEL_W, 640, "the panel's own frames reach the screen edge");
        assert_eq!(28 * 17 + 4, 480);
    }

    #[test]
    fn the_pitch_is_the_frame_width_plus_two_and_the_row_step_is_half_its_height() {
        for z in ZOOMS {
            assert_eq!(z.pitch, z.tile_w + 2, "zoom {} pitch", z.id);
            assert_eq!(z.row_step, z.tile_h / 2, "zoom {} row step", z.id);
            assert_eq!(z.half_pitch, z.pitch / 2, "zoom {} half pitch", z.id);
        }
    }

    #[test]
    fn the_viewport_starts_under_the_menu_bar_and_ends_where_the_binary_says() {
        assert_eq!(NEAR.top(), TOP_BAR_H);
        assert_eq!(FAR.top(), TOP_BAR_H);
        assert_eq!(NEAR.bottom(), 474, "Clip_Vertical(0x18, 0x1DA) in Map_DrawPathMarker");
        assert_eq!(FAR.bottom(), 408);
    }

    #[test]
    fn the_clip_rectangle_swallows_exactly_the_columns_the_half_blitters_drop() {
        for z in ZOOMS {
            let clip = z.clip();
            let left_origin = z.view_x - z.half_pitch;
            for c in [z.tile_w / 2 - 1, z.tile_w / 2] {
                assert!(
                    left_origin + c < clip.x0,
                    "zoom {}: dropped column {c} of the left tile is inside the clip",
                    z.id
                );
            }
            assert_eq!(left_origin + z.tile_w / 2 + 1, clip.x0, "zoom {} left seam", z.id);

            let right_origin = z.view_x - z.half_pitch + z.cols * z.pitch;
            for c in [z.tile_w / 2 - 1, z.tile_w / 2] {
                assert!(
                    right_origin + c >= clip.x1,
                    "zoom {}: dropped column {c} of the right tile is inside the clip",
                    z.id
                );
            }
            assert_eq!(right_origin + z.tile_w / 2 - 2, clip.x1 - 1, "zoom {} right seam", z.id);
        }
    }

    #[test]
    fn the_scroll_origin_is_clamped_to_the_lattice_and_kept_even() {
        assert_eq!(Viewport::new(-5, -5).clamped(&NEAR), Viewport::new(0, 0));
        assert_eq!(
            Viewport::new(999, 999).clamped(&NEAR),
            Viewport::new(98, 56),
            "0x80 - 30 rows, 0x40 - 8 cols"
        );

        assert_eq!(Viewport::new(41, 3).clamped(&NEAR).row, 40);

        assert_eq!(FAR.max_row(), 0);
        assert_eq!(Viewport::new(50, 50).clamped(&FAR), Viewport::new(0, 24));
    }

    /// `FUN_004059AF` passes mode 1 for the leftmost column of an offset row
    /// only, and mode 1 adds `-2` where mode 0 adds `g_mapTileHalfStep`. See
    /// [`unit_anchor`].
    #[test]
    fn the_first_figure_of_an_offset_row_stands_two_pixels_left_of_its_tile() {
        let view = Viewport::new(20, 32);
        assert_eq!(view.row & 1, 0, "the walk starts on an aligned row");
        assert_eq!(tile_to_cell(10, 10), (21, 32), "the offset row's leftmost cell");
        assert_eq!(tile_to_cell(11, 9), (21, 33), "the next column of the same row");
        assert_eq!(tile_to_cell(10, 9), (20, 32), "the aligned row's leftmost cell");

        for z in ZOOMS {
            assert_eq!(unit_anchor(view, &z, (10, 10)).0, z.view_x - 2, "zoom {} mode 1", z.id);
            assert_eq!(unit_anchor(view, &z, (11, 9)).0, z.view_x + z.pitch, "zoom {} col 1", z.id);
            let aligned = unit_anchor(view, &z, (10, 9)).0;
            assert_eq!(aligned, z.view_x + z.half_pitch, "zoom {} aligned", z.id);
            assert_eq!(aligned - unit_anchor(view, &z, (10, 10)).0, z.half_pitch + 2, "zoom {}", z.id);
        }
    }

    #[test]
    fn scrolling_moves_one_tile_per_step_and_the_far_view_does_not_move() {
        let start = Viewport::new(40, 20);
        assert_eq!(start.scrolled(Dir::N, &NEAR), Some(Viewport::new(38, 20)));
        assert_eq!(start.scrolled(Dir::S, &NEAR), Some(Viewport::new(42, 20)));
        assert_eq!(start.scrolled(Dir::E, &NEAR), Some(Viewport::new(40, 21)));
        assert_eq!(start.scrolled(Dir::W, &NEAR), Some(Viewport::new(40, 19)));
        assert_eq!(start.scrolled(Dir::NW, &NEAR), Some(Viewport::new(38, 19)));
        assert_eq!(start.scrolled(Dir::SE, &NEAR), Some(Viewport::new(42, 21)));

        assert_eq!(Viewport::new(0, 0).scrolled(Dir::NW, &NEAR), None);
        for d in DIRS {
            assert_eq!(Viewport::new(0, 10).scrolled(d, &FAR), None, "{d:?}");
        }
    }

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

    #[test]
    fn centring_uses_the_originals_offsets_including_the_asymmetric_one() {
        let v = Viewport::centred_on_cell(60, 30, &NEAR);
        assert_eq!(v, Viewport::new(48, 26));
        assert_eq!(30 - v.col, NEAR.cols / 2);
        assert_eq!(60 - v.row, 12);
        assert_ne!(60 - v.row, NEAR.rows / 2, "the original does not centre vertically");

        assert_eq!(Viewport::centred_on_cell(61, 30, &NEAR).row, 48);
    }

    #[test]
    fn no_zoom_shows_the_whole_lattice() {
        for z in ZOOMS {
            assert!(z.cols < LATTICE_W as i32, "zoom {} shows {} of 65 columns", z.id, z.cols);
        }
        assert_eq!(NEAR.cols, 8);
        assert_eq!(FAR.cols, 40);
        assert_eq!(FAR.max_col(), 24);
    }

    #[test]
    fn a_tile_outside_the_viewport_has_no_screen_centre() {
        let v = Viewport::new(40, 20);
        assert_eq!(tile_centre(v, &NEAR, 0, 0), None);
        let inside = (0..PLANE_DIM)
            .flat_map(|y| (0..PLANE_DIM).map(move |x| (x, y)))
            .find_map(|(x, y)| tile_centre(v, &NEAR, x, y))
            .expect("something is on screen");
        assert!(NEAR.clip().contains(inside.0, inside.1));
    }
}


