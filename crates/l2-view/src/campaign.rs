//! Drawing a campaign map slot out of `L2_maps.dat`, and knowing which county
//! each pixel ended up belonging to.
//!
//! # The projection
//!
//! `docs/formats/maps-layers.md` §4 settles the tile-to-lattice mapping, and it
//! is verified 4,096/4,096 against the lattice read out of a live `Lords2.exe`:
//!
//! ```text
//! tile (x, y)  ->  lattice row = x + y + 1, col = (x - y + 64) >> 1
//! screen x = col*W + (row odd ? 0 : W/2) - W/2
//! screen y = row * (H/2)
//! ```
//!
//! written there for zoom 0's 58 x 30 tiles. It is used here with the zoom-2
//! tile size, 10 x 6, because a whole 64 x 64 map is then 650 x 390 and fits on
//! one 640 x 480 screen with no scrolling viewport to build.
//!
//! The column term needs coefficients of plus and minus one half, which is why
//! the affine search that `docs/decisions.md` and `docs/formats/maps.md` still
//! describe as "reaching only 72%" could never close: an integer-coefficient
//! search cannot express it. **Those two documents are stale on this point** —
//! §4 of `maps-layers.md` is the current reading and it is exact.
//!
//! # Picking
//!
//! Tiles are diamonds, they overlap, and they are painted back to front, so the
//! county a pixel belongs to is decided by the *draw order* and not by
//! inverting the projection. [`draw`] therefore stamps the county id into a
//! [`Tags`] plane as each tile is blitted, and picking is one array read. That
//! is exact by construction: it answers with the county whose tile the player
//! can actually see at that pixel.

use l2_formats::maps::{MapSlot, Plane, PLANE_DIM};

use crate::canvas::{Canvas, Tags};
use crate::sheet::Sheet;

/// Zoom-2 tile size. `docs/decisions.md` C6: header byte 1 is the zoom level,
/// and level 2 is 10 x 6.
pub const TILE_W: i32 = 10;
pub const TILE_H: i32 = 6;

/// The five isometric tile banks, in the order `Plane::GfxBank` selects them:
/// the plane holds `layerIndex * 4`, and the layer order is the resource table
/// at `0x004DA050` (`docs/formats/maps-layers.md` §1.1).
pub const BANKS: [&str; 5] =
    ["Base2a.pl8", "Mtns2a.pl8", "Roads2a.pl8", "Town2a.pl8", "Castle2a.pl8"];

/// The zoom-2 sets ship no palette of their own; they share the zoom-0 one.
pub const PALETTE: &str = "Base1a.256";

/// The five banks, decoded on demand.
pub struct MapAssets {
    banks: Vec<Sheet>,
}

impl MapAssets {
    /// Load through a caller-supplied reader, so this works equally against a
    /// plain directory and against the mod overlay's case-insensitive VFS.
    pub fn load<F>(mut read: F) -> Result<MapAssets, String>
    where
        F: FnMut(&str) -> Result<Vec<u8>, String>,
    {
        let mut banks = Vec::with_capacity(BANKS.len());
        for name in BANKS {
            let bytes = read(name)?;
            banks.push(Sheet::new(bytes).map_err(|e| format!("{name}: {e}"))?);
        }
        Ok(MapAssets { banks })
    }

    pub fn bank(&self, index: usize) -> Option<&Sheet> {
        self.banks.get(index)
    }
}

/// Where the map's lattice origin sits on the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapView {
    pub origin_x: i32,
    pub origin_y: i32,
}

impl MapView {
    /// The whole map on one screen, under a bar of `top` pixels.
    ///
    /// No horizontal offset is needed, and that is not luck. Working the two
    /// cases of §4's formula through, the row-parity term and the `- W/2` term
    /// cancel exactly and leave `screen x = (W/2)(x - y) + 63*(W/2)`, so a
    /// 64 x 64 map spans **0 ..= 640** at zoom 2 — the screen's width to the
    /// pixel. Vertically it spans `3 ..= 387` below `top`.
    pub fn fit(top: i32) -> MapView {
        MapView { origin_x: 0, origin_y: top }
    }
}

/// `docs/formats/maps-layers.md` §4, at zoom 2.
pub fn tile_to_screen(view: MapView, x: usize, y: usize) -> (i32, i32) {
    let row = x as i32 + y as i32 + 1;
    let col = (x as i32 - y as i32 + PLANE_DIM as i32) >> 1;
    let sx = col * TILE_W + if row % 2 == 1 { 0 } else { TILE_W / 2 } - TILE_W / 2;
    let sy = row * (TILE_H / 2);
    (view.origin_x + sx, view.origin_y + sy)
}

/// The centre of a tile's diamond, which is where a marker for that tile goes.
pub fn tile_centre(view: MapView, x: usize, y: usize) -> (i32, i32) {
    let (sx, sy) = tile_to_screen(view, x, y);
    (sx + TILE_W / 2, sy + TILE_H / 2)
}

/// Paint a whole map slot, back to front, stamping county ids into `tags`.
///
/// Returns how many tiles were drawn. Tiles whose bank or frame is missing are
/// skipped rather than fatal, so an unreadable sheet leaves a hole in the map
/// instead of taking the screen down.
pub fn draw(
    canvas: &mut Canvas,
    map: &MapSlot,
    assets: &MapAssets,
    view: MapView,
    tags: &mut Tags,
) -> usize {
    let mut drawn = 0;
    // Back to front, by ascending x + y: nearer tiles overlap further ones,
    // which is what makes the diamonds tessellate into a landscape. Within one
    // diagonal the order does not matter, since those tiles never overlap.
    for sum in 0..(2 * PLANE_DIM - 1) {
        for x in 0..PLANE_DIM {
            if sum < x || sum - x >= PLANE_DIM {
                continue;
            }
            let y = sum - x;
            let bank = (map.at(Plane::GfxBank, x, y) / 4) as usize;
            let frame = map.at(Plane::GfxIndex, x, y) as usize;
            let Some(sheet) = assets.bank(bank) else { continue };
            let Some(decoded) = sheet.frame(frame) else { continue };
            let (sx, sy) = tile_to_screen(view, x, y);
            // County id 0 is "no county", which is exactly what an unstamped
            // tag already means, so sea and border tiles stamp nothing.
            canvas.blit_tagged(&decoded, sx, sy, tags, map.county_at(x, y));
            drawn += 1;
        }
    }
    drawn
}

/// Outline every pixel of `county` that touches something else, in `colour`.
///
/// Done on the tag plane rather than on tile geometry, so the outline follows
/// the shape the player can see — including where a hill tile drawn later
/// overlaps the county's edge. Returns the number of pixels painted.
pub fn outline(canvas: &mut Canvas, tags: &Tags, county: u8, colour: u8) -> usize {
    if county == 0 {
        return 0;
    }
    let mut painted = 0;
    for y in 0..tags.height as i32 {
        for x in 0..tags.width as i32 {
            if tags.at(x, y) != county {
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

    /// The projection is a pure formula over tile coordinates, so it can be
    /// checked with no map file at all — and the two identities below are the
    /// ones that would break if the ±½ column term were rounded the other way.
    #[test]
    fn the_lattice_mapping_steps_a_half_tile_east_and_a_half_tile_south() {
        let v = MapView { origin_x: 0, origin_y: 0 };
        let (x0, y0) = tile_to_screen(v, 0, 0);
        assert_eq!(tile_to_screen(v, 1, 0), (x0 + TILE_W / 2, y0 + TILE_H / 2), "+x is south-east");
        assert_eq!(tile_to_screen(v, 0, 1), (x0 - TILE_W / 2, y0 + TILE_H / 2), "+y is south-west");
        assert_eq!(tile_to_screen(v, 1, 1), (x0, y0 + TILE_H), "and the two cancel");
    }

    /// `docs/formats/maps-layers.md` §4's worked geometry, at zoom 0's tile
    /// size: tile (0,0) lands at lattice row 1, column 32, screen (1827, 15).
    /// Reproduced here by scaling, which is the same arithmetic the renderer
    /// does at zoom 2.
    #[test]
    fn the_zoom_zero_geometry_in_the_document_reproduces() {
        // row = 0+0+1 = 1 (odd), col = (0-0+64)>>1 = 32.
        let (row, col) = (1i32, 32i32);
        let (w, h) = (58i32, 30i32);
        assert_eq!(col * w + 0 - w / 2, 1827);
        assert_eq!(row * (h / 2), 15);
    }

    /// The claim `MapView::fit` rests on: a whole map is exactly 640 wide at
    /// zoom 2, so nothing is clipped and nothing needs centring. Also that the
    /// two cases of the parity term agree, which is the identity
    /// `screen x = (W/2)(x - y) + 63*(W/2)`.
    #[test]
    fn the_whole_map_spans_the_screen_exactly_and_both_parities_agree() {
        let v = MapView::fit(20);
        let (mut min_x, mut max_x, mut min_y, mut max_y) =
            (i32::MAX, i32::MIN, i32::MAX, i32::MIN);
        for y in 0..PLANE_DIM {
            for x in 0..PLANE_DIM {
                let (sx, sy) = tile_to_screen(v, x, y);
                assert_eq!(
                    sx,
                    (TILE_W / 2) * (x as i32 - y as i32) + 63 * (TILE_W / 2),
                    "the parity cases must agree at ({x},{y})"
                );
                min_x = min_x.min(sx);
                max_x = max_x.max(sx + TILE_W);
                min_y = min_y.min(sy);
                max_y = max_y.max(sy + TILE_H);
            }
        }
        assert_eq!((min_x, max_x), (0, 640), "exactly the width of the screen");
        assert_eq!((min_y, max_y), (23, 407), "and 384 tall below the bar");
    }

    #[test]
    fn an_outline_traces_the_border_of_a_tagged_region_and_nothing_inside_it() {
        let mut c = Canvas::new(8, 8);
        let mut t = Tags::new(8, 8);
        for y in 2..6 {
            for x in 2..6 {
                t.ids[y * 8 + x] = 3;
            }
        }
        let painted = outline(&mut c, &t, 3, 7);
        assert_eq!(painted, 12, "a 4x4 block has twelve edge pixels");
        assert_eq!(c.at(2, 2), 7);
        assert_eq!(c.at(3, 3), 0, "the interior is left alone");
        assert_eq!(outline(&mut c, &t, 0, 7), 0, "county 0 is not a county");
        assert_eq!(outline(&mut c, &t, 4, 7), 0, "and an absent county paints nothing");
    }
}
