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

/// One of the two campaign zooms,
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
    /// The five isometric tile banks **per season**, indexed by
    /// [`season_slot`], in the order `Plane::GfxBank` selects them.
    ///
    /// `Gfx_LoadCountyMode` loads exactly these five, in this order, from
    /// consecutive `g_resourceTable` entries starting at
    /// `(zoom == 2 ? 0x20 : 0) + (season - 1) * 8`. This array **is** those
/// entries, read out of the table at `0x004DA050`
    /// the filenames on disk — which matters, because the far zoom's four rows
    /// are not four files. See [`SEASON_SUFFIX`].
    pub banks: [[&'static str; 5]; SEASONS],
    /// `g_spriteSheetA` and `g_spriteSheetB` — resource table entries 5 and 6
    /// of the zoom's block. `Map_DrawArmies` picks **B for a transport and A
    /// for everything else**, and that is the whole of the choice.
    pub sprites: [&'static str; 2],
    /// `g_flagsSheet` — entry 7. Not seasonal: entries 7, 15, 23 and 31 all
    /// name `flags1a.pl8`, and `Flags1b/c/d.pl8` ship and are never loaded.
    pub flags: &'static str,
    /// Where a flag goes, as an offset from the **tile origin** — the top-left
    /// of the diamond's bounding box, which is `cell_to_screen`'s answer.
    /// `FUN_004071A0` adds this and blits the frame there with no further
    /// centring: the frame's `cx`/`cy` fields are atlas coordinates and the
    /// function never reads them.
    pub flag_at: (i32, i32),
    /// **Where the mercenary marker goes, which is not where the flag goes.**
    ///
    /// `Sprite_TopIt`'s two town arms are the same shape and set *different*
    /// offsets, and reading one and reusing it for the other is the mistake this
    /// field exists to make impossible:
    ///
    /// ```c
    /// if (part == 0) { ... if (zoom == 0) { dx = 0x1A; dy = -0x1C; }   /* the banner    */
    ///                      else if (zoom == 2) { dx = 6; dy = -0x15; } local_c = 2; }
    /// if (part == 2) { ... if (zoom == 0) { dx = 0x10; dy = -0x12; }   /* the mercenary */
    ///                      else if (zoom == 2) { dx = 6; dy = -0x15; } local_c = 0; }
    /// ```
    ///
    /// Ten pixels left and ten up of the banner at the near zoom, identical at
    /// the far one. `local_c` differs too — the banner marks a 48-pixel dirty
    /// square through `Gfx_MarkTile48` and the mercenary marks **nothing**,
    /// which is a repaint economy we do not have and do not need. **[V]**
    pub mercenary_at: (i32, i32),
    /// **Where the besieger's camp mark goes** — `Sprite_TopIt`'s arm 6, the
    /// one call site of [`besieger_marker`]'s painter:
    ///
    /// ```c
    /// if (units[garrison].besiegedBy != 0) {
    ///   if      (g_mapZoom == 0) FUN_00407f82(units[besieger].siegeSeasonsLeft, 8, -0x38);
    ///   else if (g_mapZoom == 2) FUN_00407f82(units[besieger].siegeSeasonsLeft, 2, -0x28);
    /// }
    /// ```
    ///
    /// **The far zoom's pair is dead and is kept anyway.** `FUN_00407F82`'s
    /// entire body is inside `if (g_mapZoom == 0)` — `83 3D 18CB5700 00` then
    /// `0F 84 05` / `E9 07 02` at `0x00407F8C`, read out of the shipped bytes —
    /// so the second call draws nothing at all and a besieged castle carries no
    /// mark at zoom 2. `docs/bugs.md`; the offset is recorded here so nobody
    /// re-derives it when asking why. **[V]**
    pub besieger_at: (i32, i32),
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
    banks: [
        ["Base1a.pl8", "Mtns1a.pl8", "Roads1a.pl8", "Town1a.pl8", "Castle1a.pl8"],
        ["Base1b.pl8", "Mtns1b.pl8", "Roads1b.pl8", "Town1b.pl8", "Castle1b.pl8"],
        ["Base1c.pl8", "Mtns1c.pl8", "Roads1c.pl8", "Town1c.pl8", "Castle1c.pl8"],
        ["Base1d.pl8", "Mtns1d.pl8", "Roads1d.pl8", "Town1d.pl8", "Castle1d.pl8"],
    ],
    sprites: ["Sprite1a.pl8", "Sprite1b.pl8"],
    flags: "Flags1a.pl8",
    flag_at: (0x1A, -0x1C),
    mercenary_at: (0x10, -0x12),
    besieger_at: (8, -0x38),
};

/// Zoom 2: 10 × 6 tiles, forty lattice columns on screen. The original pins the
/// scroll origin at this zoom and refuses to scroll at all.
///
/// # The far zoom has no seasons, and this is not an omission
///
/// `Base2b.pl8`, `Mtns2c.pl8` and the other eleven zoom-2 seasonal files ship
/// in the install and **the game never opens one.** `g_resourceTable`'s zoom-2
/// half, entries 32 … 63, is four *identical* blocks:
///
/// ```text
/// 32 base2a  33 mtns2a  34 roads2a  35 town2a  36 castle2a  37 sprite2a  38 sprite2b  39 flags2a
/// 40 base2a  41 mtns2a  42 roads2a  43 town2a  44 castle2a  …          (season 2)
/// 48 base2a  …                                                        (season 3)
/// 56 base2a  …                                                        (season 4)
/// ```
///
/// so `base + (season - 1) * 8` lands on the same five filenames whichever
/// season it is. Read out of the table at `0x004DA050` in the shipped binary,
/// not inferred. **[V]** — `docs/decisions.md` C63.
///
/// The dead files are not even consistent with the live one: `Town2a.pl8` has
/// **61** frames and `Town2b/c/d.pl8` have **94**, and their frame records do
/// not line up. Pointing the far zoom at them by filename — which is what
/// deriving the name from the suffix would have done — would have drawn the
/// wrong picture for three seasons out of four.
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
    banks: [
        ["Base2a.pl8", "Mtns2a.pl8", "Roads2a.pl8", "Town2a.pl8", "Castle2a.pl8"],
        ["Base2a.pl8", "Mtns2a.pl8", "Roads2a.pl8", "Town2a.pl8", "Castle2a.pl8"],
        ["Base2a.pl8", "Mtns2a.pl8", "Roads2a.pl8", "Town2a.pl8", "Castle2a.pl8"],
        ["Base2a.pl8", "Mtns2a.pl8", "Roads2a.pl8", "Town2a.pl8", "Castle2a.pl8"],
    ],
    sprites: ["Sprite2a.pl8", "Sprite2b.pl8"],
    flags: "Flags2a.pl8",
    flag_at: (6, -0x15),
    mercenary_at: (6, -0x15),
    besieger_at: (2, -0x28),
};

/// The two zooms the campaign screen has, near first.
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
/// construction.
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

/// The same, for a map tile.
    pub fn centred_on_tile(x: usize, y: usize, zoom: &Zoom) -> Viewport {
        let (row, col) = tile_to_cell(x, y);
        Viewport::centred_on_cell(row, col, zoom)
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

/// The five tile banks **in every season**, the two sprite sheets and the flag
/// sheet at both zooms, decoded on demand.
///
/// # Why the banks are a pool and an index table
///
/// `Gfx_LoadCountyMode` does not hold four seasons at once: it frees the eight
/// buffers and reloads them from a different eight resource-table entries every
/// time the season turns. We hold all of them, because a `Sheet` decodes lazily
/// and re-reading five files on the season boundary would need the reader kept
/// alive for the life of the program.
///
/// That makes the repetition matter. The far zoom names the *same five files*
/// for all four seasons ([`FAR`]), so a naive `[[Sheet; 5]; 4]` per zoom would
/// hold four copies of `Base2a.pl8`. Interning by filename collapses those to
/// one and costs a string compare at load. Twenty-five names resolve to
/// **twenty** distinct files.
pub struct MapAssets {
    /// Every distinct bank file, decoded once. `Gfx_LoadCountyMode` repoints
    /// eight pointers at eight buffers; this is what they point into.
    banks: Vec<Sheet>,
    /// `[set][season slot][bank index]` → an index into `banks`, or `None` when
    /// that file would not load.
    bank_at: [[[Option<usize>; 5]; SEASONS]; 2],
    sprites: [Vec<Sheet>; 2],
    flags: [Option<Sheet>; 2],
}

impl MapAssets {
    /// Load through a caller-supplied reader, so this works equally against a
    /// plain directory and against the mod overlay's case-insensitive VFS.
    ///
    /// **Spring is required and the other three seasons are not.** Without
    /// spring's five banks; without summer's the map
    /// falls back on spring, which is a map that does not change with the year
    /// An install that is missing `Base1c.pl8` should still
    /// play, and a mod that ships one season should not have to ship four.
    ///
    /// The sprite and flag sheets are optional for the same reason: everything
    /// that draws from them falls back to a marker of ours, so a partial
    /// install still shows where its units are.
    pub fn load<F>(mut read: F) -> Result<MapAssets, String>
    where
        F: FnMut(&str) -> Result<Vec<u8>, String>,
    {
        let mut banks: Vec<Sheet> = Vec::new();
        let mut names: Vec<String> = Vec::new();
        let mut bank_at = [[[None; 5]; SEASONS]; 2];
        let mut sprites = [Vec::new(), Vec::new()];
        let mut flags = [None, None];
        for zoom in ZOOMS {
            for (season, set) in zoom.banks.iter().enumerate() {
                for (index, &name) in set.iter().enumerate() {
                    if let Some(i) = names.iter().position(|n| n == name) {
                        bank_at[zoom.set][season][index] = Some(i);
                        continue;
                    }
                    // Spring is the one the caller is entitled to an error
                    // about; a season that will not load is left `None` and
                    // resolves back to spring at draw time.
                    let sheet = match read(name).and_then(|b| {
                        Sheet::new(b).map_err(|e| format!("{name}: {e}"))
                    }) {
                        Ok(s) => s,
                        Err(e) if season == 0 => return Err(e),
                        Err(_) => continue,
                    };
                    names.push(name.to_string());
                    banks.push(sheet);
                    bank_at[zoom.set][season][index] = Some(banks.len() - 1);
                }
            }
            for name in zoom.sprites {
                if let Some(s) = read(name).ok().and_then(|b| Sheet::new(b).ok()) {
                    sprites[zoom.set].push(s);
                }
            }
            flags[zoom.set] = read(zoom.flags).ok().and_then(|b| Sheet::new(b).ok());
        }
        Ok(MapAssets { banks, bank_at, sprites, flags })
    }

    /// One tile bank, for a zoom and a `g_season` value.
    ///
    /// A season whose file would not load falls back to spring, which is
    /// `Gfx_LoadCountyMode`'s own behaviour for a season outside 1 … 4.
    pub fn bank(&self, zoom: &Zoom, season: u8, index: usize) -> Option<&Sheet> {
        let table = self.bank_at.get(zoom.set)?;
        let slot = season_slot(season);
        let at = table[slot].get(index).copied().flatten().or_else(|| {
            table[0].get(index).copied().flatten()
        })?;
        self.banks.get(at)
    }

/// How many distinct bank files were loaded. Twenty on a complete
    /// install: fifteen for the near zoom's four seasons and five for the far
    /// zoom's one.
    pub fn bank_files(&self) -> usize {
        self.banks.len()
    }

    /// Whether this season has artwork of its own, or is falling back on
    /// spring. A caller that wants to say "this install has no winter" can ask.
    pub fn has_season(&self, zoom: &Zoom, season: u8) -> bool {
        self.bank_at[zoom.set][season_slot(season)].iter().all(Option::is_some)
    }

    /// `g_spriteSheetA` (0) or `g_spriteSheetB` (1) for this zoom.
    pub fn sprite_sheet(&self, zoom: &Zoom, index: usize) -> Option<&Sheet> {
        self.sprites.get(zoom.set)?.get(index)
    }

    /// `g_flagsSheet` for this zoom.
    pub fn flag_sheet(&self, zoom: &Zoom) -> Option<&Sheet> {
        self.flags.get(zoom.set)?.as_ref()
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

/// **The fog of war, as the terrain pass sees it** — `Some` when
/// `g_optExploration` is on, holding the viewer's test *"is tile `(x, y)`
/// unseen?"*, and `None` when the option is off.
///
/// A closure because this crate does not know where the
/// seen bits live, and should not: `l2_kingdom::explore::hides` is the test and
/// the campaign screen hands it in.
pub type Fog<'a> = Option<&'a dyn Fn(usize, usize) -> bool>;

/// Paint the viewport, stamping county ids into `tags`. Returns tiles drawn.
///
/// The traversal is `Map_RenderIso`'s: `rows + 1` lattice rows starting at
/// `view.row`, the first and last half-clipped by the viewport rectangle rather
/// than by a special blitter. An aligned row draws `cols` cells; an offset row
/// draws `cols + 1`, because shifting left by half a pitch exposes one more
/// column on the right.
///
/// # The fog
///
/// `fog` is the one input the five terrain painters of the original read that
/// is not the map, and it changes two things, each **`[V]`** out of the
/// decompilation:
///
/// **Frame 0 of the `base` bank is two different pictures**, measured over all
/// four seasons of the player's own files (`crates/l2-game/tests/screens_map.rs`,
/// `a_dark_tile_…`). At the near zoom it is blank — every pixel palette index
/// 0, which every blitter skips — so both arms below paint nothing and the dark
/// is the black ground the map is drawn on, *"blacked out"* literally. At the
/// far zoom it is a filled 10 × 6 diamond of green, so zoomed out the dark is
/// plain grass.
///
/// * **An unseen tile is the `base` bank's frame 0 and nothing above it.**
///   `Map_DrawTile` (`0x004063C1`):
///   `if (g_optExploration == 1 && (bank & 0x20) == 0) { bank = 0; frame = 0; }`,
///   and `Map_DrawTileApex` (`0x00406673`) draws its overhang only under the
/// opposite test — so a mountain, a town or a castle in the dark is flat.
///   The county id still reaches `tags`: `Map_DrawTile` loads the tile's county
///   byte before the test, and a click still resolves the county under it.
/// * **The whole off-map surround is frame 0 while the option is on**, seen or
///   not. `Map_RenderIso` (`0x0040526E`), `Map_RenderAlignedRow` (`0x00405AE9`)
///   and `Map_RenderOffsetRow` (`0x00405C2F`), all six surround arms:
///   `frame = g_optExploration == 1 ? 0 : cell - 0x0FFF0000;`. The sea round a
///   fogged map is never the sea.
#[allow(clippy::too_many_arguments)]
pub fn draw(
    canvas: &mut Canvas,
    map: &MapSlot,
    lattice: &Lattice,
    assets: &MapAssets,
    view: Viewport,
    zoom: &Zoom,
    tags: &mut Tags,
    overrides: &Overrides,
    season: u8,
    fog: Fog,
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
                Some((x, y)) if fog.is_some_and(|hidden| hidden(x, y)) => {
                    // `Map_DrawTile`'s fog arm: base bank, frame 0. And
                    // `Map_DrawTileApex` draws nothing, so the frame is clipped
                    // to the diamond's own rows — nothing above `sy`.
                    let flat = Clip { y0: clip.y0.max(sy), ..clip };
                    let county = map.county_at(x, y);
                    if blit_cell(canvas, assets, zoom, season, 0, 0, sx, sy, flat, tags, county) {
                        drawn += 1;
                    }
                }
                Some((x, y)) => {
                    let (bank_byte, frame) = overrides
                        .get(x, y)
                        .unwrap_or((map.at(Plane::GfxBank, x, y), map.at(Plane::GfxIndex, x, y)));
                    // `Map_ResolvePick` reads the bank as `tile.bank & 0x1c`,
                    // and the mask is not decoration: at run time the same byte
                    // also carries `0x01`, `0x20`, `0x40` and `0x80`, which
                    // `County_FindTownTile` and `FUN_0046ac22` set. Nothing on
                    // disk has them — 0 of 180,224 tiles — so this changed no
                    // pixel, and it is the difference between a renderer that
                    // works on the file and one that works on the game's state.
                    let bank = ((bank_byte & BANK_MASK) >> 2) as usize;
                    let frame = frame as usize;
                    let county = map.county_at(x, y);
                    if blit_cell(
                        canvas, assets, zoom, season, bank, frame, sx, sy, clip, tags, county,
                    ) {
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
                    //
                    // **And frame 0, whatever the lattice says, while the fog
                    // option is on** — the six surround arms of the three row
                    // walkers, `frame = g_optExploration == 1 ? 0 : cell -
                    // 0x0FFF0000`. See the heading.
                    let frame =
                        if fog.is_some() { 0 } else { lattice.surround(row, col) as usize };
                    blit_cell(canvas, assets, zoom, season, 0, frame, sx, sy, clip, tags, 0);
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
    season: u8,
    bank: usize,
    frame: usize,
    sx: i32,
    sy: i32,
    clip: Clip,
    tags: &mut Tags,
    county: u8,
) -> bool {
    let Some(sheet) = assets.bank(zoom, season, bank) else { return false };
    let Some(decoded) = sheet.frame(frame) else { return false };
    // The diamond sits at the bottom of the decoded frame; anything above it is
    // the apex rows `Map_DrawTileApex` blits separately in the original.
    let overhang = (decoded.height as i32 - zoom.tile_h).max(0);
    canvas.blit_clipped_tagged(&decoded, sx, sy - overhang, clip, tags, county);
    true
}

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

