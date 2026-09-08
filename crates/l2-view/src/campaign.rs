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
//! it 1. See `docs/screens.md` §2.2 — there is no shipped PL8 holding 26 × 14
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

use l2_formats::maps::{MapSlot, Plane, LATTICE_H, LATTICE_W, PLANE_DIM};

use crate::canvas::{Canvas, Clip, Tags};
use crate::sheet::Sheet;

/// Height of the menu bar, and the top of the map viewport. `Screen_DrawMenuBar`
/// fills 640 × 24, and `Map_DrawCountyFlag` clips the map to
/// `Clip_Vertical(0x18, …)`.
pub const TOP_BAR_H: i32 = 24;

/// Left edge of the right-hand panel, and the map's right clip.
/// `Map_DrawCountyFlag` calls `Clip_Horizontal(g_mapViewX, 0x1DE)`; the panel's
/// `Misc_cty` frames are 162 wide and are drawn at 478, and 478 + 162 = 640.
pub const PANEL_X: i32 = 478;
pub const PANEL_W: i32 = 162;

/// The palette `Screen_DrawCampaign` installs: `Palette_Set(g_paletteBase01)`,
/// and `0x005691F0` is entry 0 of the startup preload table, `Base01.256`.
pub const PALETTE: &str = "Base01.256";

/// One of the two campaign zooms, exactly as `Map_SetZoom` sets it up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Zoom {
    /// `g_mapZoom`. 0 near, 2 far — the original's own numbering, with the gap
    /// where the dead middle zoom is.
    pub id: u8,
    /// Which of [`MapAssets`]'s two sets of banks this zoom draws from.
    pub set: usize,
    /// The tile artwork's own size. `pitch == tile_w + 2` and
    /// `row_step == tile_h / 2` at every zoom (`docs/screens.md` §1.2), which
    /// is what resolved `maps-layers.md`'s 58-versus-60 discrepancy.
    pub tile_w: i32,
    pub tile_h: i32,
    /// `g_mapTilePitch`, `g_mapHalfPitch`, `g_mapRowStep`.
    pub pitch: i32,
    pub half_pitch: i32,
    pub row_step: i32,
    /// `g_mapViewCols`, `g_mapViewRows`.
    pub cols: i32,
    pub rows: i32,
    /// `g_mapViewX`, `g_mapViewY` — where the *first drawn lattice row* goes,
    /// not where the visible band starts. The first row is top-clipped.
    pub view_x: i32,
    pub view_y: i32,
    /// `g_mapScrollStep` — lattice columns per scroll step.
    pub scroll_step: i32,
    /// The five isometric tile banks, in the order `Plane::GfxBank` selects
    /// them. `Gfx_LoadCountyMode` loads exactly these five, in this order, from
    /// consecutive `g_resourceTable` entries.
    pub banks: [&'static str; 5],
}

/// Zoom 0: 58 × 30 tiles, eight lattice columns on screen.
pub const NEAR: Zoom = Zoom {
    id: 0,
    set: 0,
    tile_w: 58,
    tile_h: 30,
    pitch: 60,
    half_pitch: 30,
    row_step: 15,
    cols: 8,
    rows: 30,
    view_x: 0,
    view_y: 9,
    scroll_step: 1,
    banks: ["Base1a.pl8", "Mtns1a.pl8", "Roads1a.pl8", "Town1a.pl8", "Castle1a.pl8"],
};

/// Zoom 2: 10 × 6 tiles, forty lattice columns on screen. The original pins the
/// scroll origin at this zoom and refuses to scroll at all.
pub const FAR: Zoom = Zoom {
    id: 2,
    set: 1,
    tile_w: 10,
    tile_h: 6,
    pitch: 12,
    half_pitch: 6,
    row_step: 3,
    cols: 40,
    rows: 128,
    view_x: 0,
    view_y: 21,
    scroll_step: 4,
    banks: ["Base2a.pl8", "Mtns2a.pl8", "Roads2a.pl8", "Town2a.pl8", "Castle2a.pl8"],
};

/// The two zooms the campaign screen actually has, near first.
pub const ZOOMS: [Zoom; 2] = [NEAR, FAR];

impl Zoom {
    /// Top of the visible band. 24 at both zooms, because the first drawn row
    /// is top-clipped: `view_y + row_step` is 9 + 15 and 21 + 3.
    pub const fn top(&self) -> i32 {
        self.view_y + self.row_step
    }

    /// `g_mapViewBottom` = `(rows + 1) * row_step + view_y`: 474 near, 408 far.
    pub const fn bottom(&self) -> i32 {
        self.view_y + self.row_step * (self.rows + 1)
    }

    /// `g_mapViewRight` = `pitch * cols + view_x`. **480 at both zooms**, which
    /// is the arithmetic that fixes the 160-pixel right column.
    pub const fn right(&self) -> i32 {
        self.view_x + self.pitch * self.cols
    }

    /// What a tile blit is allowed to write. See the module docs.
    pub const fn clip(&self) -> Clip {
        Clip::new(self.view_x, self.top(), PANEL_X, self.bottom())
    }

    /// `Map_ClampScroll`'s bounds: `0x40 - cols` and `0x80 - rows`.
    pub const fn max_col(&self) -> i32 {
        64 - self.cols
    }

    pub const fn max_row(&self) -> i32 {
        128 - self.rows
    }
}

/// Where the viewport sits in the lattice: `g_mapStartRow`, `g_mapStartCol`.
///
/// `row` is **always even** in the original — every setter uses an even
/// literal, `row & !1`, or a step of two — because the render walk alternates
/// aligned and half-offset lattice rows starting from it, and
/// `maps-layers.md` §4 makes the *odd* lattice rows the half-offset ones.
/// [`Viewport::clamped`] enforces it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Viewport {
    pub row: i32,
    pub col: i32,
}

/// The eight directions `Map_EdgeScroll` produces, clockwise from north.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dir {
    N,
    NE,
    E,
    SE,
    S,
    SW,
    W,
    NW,
}

/// Every direction, in the original's own 0..7 order.
pub const DIRS: [Dir; 8] =
    [Dir::N, Dir::NE, Dir::E, Dir::SE, Dir::S, Dir::SW, Dir::W, Dir::NW];

impl Dir {
    /// `Map_ScrollStep`: the row moves by twice the step and the column by one,
    /// which is one map tile per press in a compass direction.
    const fn delta(self) -> (i32, i32) {
        match self {
            Dir::N => (-2, 0),
            Dir::NE => (-2, 1),
            Dir::E => (0, 1),
            Dir::SE => (2, 1),
            Dir::S => (2, 0),
            Dir::SW => (2, -1),
            Dir::W => (0, -1),
            Dir::NW => (-2, -1),
        }
    }
}

impl Viewport {
    /// `Map_InitMode`'s opening position: row 0x4A, col 0x14.
    pub const START: Viewport = Viewport { row: 0x4A, col: 0x14 };

    pub const fn new(row: i32, col: i32) -> Viewport {
        Viewport { row, col }
    }

    /// `Map_ClampScroll`, plus the parity the original maintains by
    /// construction rather than by clamping.
    pub fn clamped(self, zoom: &Zoom) -> Viewport {
        Viewport {
            row: (self.row.clamp(0, zoom.max_row())) & !1,
            col: self.col.clamp(0, zoom.max_col()),
        }
    }

    /// One `Map_ScrollStep` in a direction, clamped.
    ///
    /// `Map_EdgeScroll` returns without scrolling at the far zoom, so this
    /// refuses there too and the caller can see that it did.
    pub fn scrolled(self, dir: Dir, zoom: &Zoom) -> Option<Viewport> {
        if zoom.id == FAR.id {
            return None;
        }
        let (dr, dc) = dir.delta();
        let moved = Viewport {
            row: self.row + dr * zoom.scroll_step,
            col: self.col + dc * zoom.scroll_step,
        };
        let clamped = moved.clamped(zoom);
        (clamped != self).then_some(clamped)
    }

    /// `Map_CentreOnTile`: `col - 4`, `(row & !1) - 12`, clamped.
    ///
    /// The two literals are the original's and they are *not* the centre — 8
    /// visible columns make −4 the horizontal centre, but 30 visible rows would
    /// need −15, so the target lands below the middle. Only the near zoom
    /// centres in the original; the far one has nowhere to go.
    pub fn centred_on_cell(row: i32, col: i32, zoom: &Zoom) -> Viewport {
        Viewport { row: (row & !1) - 12, col: col - 4 }.clamped(zoom)
    }

    /// The same, for a map tile rather than a lattice cell.
    pub fn centred_on_tile(x: usize, y: usize, zoom: &Zoom) -> Viewport {
        let (row, col) = tile_to_cell(x, y);
        Viewport::centred_on_cell(row, col, zoom)
    }
}

/// `maps-layers.md` §4, rotation 0 — recovered again here out of
/// `Map_BuildLattice`'s loop, which writes `row = 1 + y + x`,
/// `col = (64 - y + x) / 2`.
///
/// **Rotations 2, 4 and 6 are not implemented.** `Map_BuildLattice` builds all
/// four and `Map_RotateCW` / `Map_RotateCCW` step between them; we only ever
/// build rotation 0 and there is no way to ask for another.
pub fn tile_to_cell(x: usize, y: usize) -> (i32, i32) {
    let (x, y) = (x as i32, y as i32);
    (x + y + 1, (x - y + PLANE_DIM as i32) >> 1)
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

/// The five tile banks at both zooms, decoded on demand.
pub struct MapAssets {
    sets: [Vec<Sheet>; 2],
}

impl MapAssets {
    /// Load through a caller-supplied reader, so this works equally against a
    /// plain directory and against the mod overlay's case-insensitive VFS.
    pub fn load<F>(mut read: F) -> Result<MapAssets, String>
    where
        F: FnMut(&str) -> Result<Vec<u8>, String>,
    {
        let mut sets = [Vec::new(), Vec::new()];
        for zoom in ZOOMS {
            for name in zoom.banks {
                let bytes = read(name)?;
                sets[zoom.set].push(Sheet::new(bytes).map_err(|e| format!("{name}: {e}"))?);
            }
        }
        Ok(MapAssets { sets })
    }

    pub fn bank(&self, zoom: &Zoom, index: usize) -> Option<&Sheet> {
        self.sets.get(zoom.set)?.get(index)
    }
}

/// Where a lattice cell's tile is drawn, given the viewport.
///
/// Odd lattice rows are shifted left by half a pitch — `maps-layers.md` §4's
/// parity term, and the reason `Map_RenderOffsetRow` exists as a separate
/// function in the original. The returned `y` is the **diamond's** top, not the
/// frame's: a frame with apex ("overhang") rows is taller than its tile and
/// [`draw`] subtracts the difference.
pub fn cell_to_screen(view: Viewport, zoom: &Zoom, row: i32, col: i32) -> (i32, i32) {
    let offset = if row & 1 == 1 { zoom.half_pitch } else { 0 };
    (
        zoom.view_x + (col - view.col) * zoom.pitch - offset,
        zoom.view_y + (row - view.row) * zoom.row_step,
    )
}

/// The centre of a map tile's diamond on screen, which is where a marker for
/// that tile goes. `None` when the tile is not in the viewport.
pub fn tile_centre(view: Viewport, zoom: &Zoom, x: usize, y: usize) -> Option<(i32, i32)> {
    let (row, col) = tile_to_cell(x, y);
    let (sx, sy) = cell_to_screen(view, zoom, row, col);
    let (cx, cy) = (sx + zoom.tile_w / 2, sy + zoom.tile_h / 2);
    zoom.clip().contains(cx, cy).then_some((cx, cy))
}

/// Paint the viewport, stamping county ids into `tags`. Returns tiles drawn.
///
/// The traversal is `Map_RenderIso`'s: `rows + 1` lattice rows starting at
/// `view.row`, the first and last half-clipped by the viewport rectangle rather
/// than by a special blitter. An aligned row draws `cols` cells; an offset row
/// draws `cols + 1`, because shifting left by half a pitch exposes one more
/// column on the right.
pub fn draw(
    canvas: &mut Canvas,
    map: &MapSlot,
    lattice: &Lattice,
    assets: &MapAssets,
    view: Viewport,
    zoom: &Zoom,
    tags: &mut Tags,
) -> usize {
    let clip = zoom.clip();
    let mut drawn = 0;
    for dr in 0..=zoom.rows {
        let row = view.row + dr;
        // An offset row is exposed by half a pitch on the right, so it needs
        // one lattice column more than an aligned one.
        let span = if row & 1 == 1 { zoom.cols + 1 } else { zoom.cols };
        for dc in 0..span {
            let col = view.col + dc;
            let (sx, sy) = cell_to_screen(view, zoom, row, col);
            match lattice.tile(row, col) {
                Some((x, y)) => {
                    let bank = (map.at(Plane::GfxBank, x, y) / 4) as usize;
                    let frame = map.at(Plane::GfxIndex, x, y) as usize;
                    let county = map.county_at(x, y);
                    if blit_cell(canvas, assets, zoom, bank, frame, sx, sy, clip, tags, county) {
                        drawn += 1;
                    }
                }
                None => {
                    // The surround. `Map_DrawSurroundTile` takes the frame from
                    // the lattice byte and always uses the `base` bank.
                    //
                    // **Ours, and visibly different:** the original permutes
                    // that index at load — stored 6 becomes one of the 16 grass
                    // frames and stored 22 one of the 8 water frames
                    // (`maps-layers.md` §4.1). We draw what is stored, so the
                    // sea and the off-map grass repeat where the original
                    // varies them.
                    let frame = lattice.surround(row, col) as usize;
                    blit_cell(canvas, assets, zoom, 0, frame, sx, sy, clip, tags, 0);
                }
            }
        }
    }
    drawn
}

#[allow(clippy::too_many_arguments)]
fn blit_cell(
    canvas: &mut Canvas,
    assets: &MapAssets,
    zoom: &Zoom,
    bank: usize,
    frame: usize,
    sx: i32,
    sy: i32,
    clip: Clip,
    tags: &mut Tags,
    county: u8,
) -> bool {
    let Some(sheet) = assets.bank(zoom, bank) else { return false };
    let Some(decoded) = sheet.frame(frame) else { return false };
    // The diamond sits at the bottom of the decoded frame; anything above it is
    // the apex rows `Map_DrawTileApex` blits separately in the original.
    let overhang = (decoded.height as i32 - zoom.tile_h).max(0);
    canvas.blit_clipped_tagged(&decoded, sx, sy - overhang, clip, tags, county);
    true
}

/// Outline every pixel of `county` that touches something else, in `colour`.
///
/// **This is ours.** The original has no county outline: its borders are in the
/// tile data — `maps-layers.md` §2.1, plane-0 bit `0x02` switches the tile to
/// the `roads` bank's boundary frames — and its *selection* is not drawn on the
/// map at all, it is which county the right panel describes.
///
/// Done on the tag plane rather than on tile geometry, so the outline follows
/// the shape the player can see. Returns the number of pixels painted.
pub fn outline(canvas: &mut Canvas, tags: &Tags, county: u8, colour: u8, clip: Clip) -> usize {
    if county == 0 {
        return 0;
    }
    let mut painted = 0;
    for y in 0..tags.height as i32 {
        for x in 0..tags.width as i32 {
            if tags.at(x, y) != county || !clip.contains(x, y) {
                continue;
            }
            let edge = tags.at(x - 1, y) != county
                || tags.at(x + 1, y) != county
                || tags.at(x, y - 1) != county
                || tags.at(x, y + 1) != county;
            if edge {
                canvas.set(x as usize, y as usize, colour);
                painted += 1;
            }
        }
    }
    painted
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `Map_SetZoom` derives `g_mapViewRight = pitch*cols + viewX` and it comes
    /// out **480 at every zoom**, which is what leaves 160 pixels for the right
    /// column. This is the single arithmetic fact the whole layout rests on, so
    /// it is asserted from the constants rather than written down as 480.
    #[test]
    fn every_zoom_gives_the_map_the_same_480_pixels_and_the_panel_the_rest() {
        for z in ZOOMS {
            assert_eq!(z.right(), 480, "zoom {} width", z.id);
        }
        assert_eq!(PANEL_X + PANEL_W, 640, "the panel's own frames reach the screen edge");
        // The dead middle zoom, included because it is the third data point
        // that makes 480 an intention rather than a coincidence: 28*17 + 4.
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
        assert_eq!(NEAR.bottom(), 474, "Clip_Vertical(0x18, 0x1DA) in Map_DrawCountyFlag");
        assert_eq!(FAR.bottom(), 408);
    }

    /// **The derivation the single clipped blitter rests on** (`screens.md`
    /// §1.4). The original's half-tile blitters drop the two columns either
    /// side of the vertical seam; for the clip rectangle to be equivalent,
    /// those columns must land outside it at both ends and at both zooms.
    ///
    /// If the clip were 480 rather than 478 the right-hand half of this fails,
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
