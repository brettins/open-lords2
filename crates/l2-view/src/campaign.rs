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
/// **spring** set. This clamps rather than wrapping for the same reason.
pub fn season_slot(season: u8) -> usize {
    if (1..=SEASONS as u8).contains(&season) {
        season as usize - 1
    } else {
        0
    }
}

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
    /// The five isometric tile banks **per season**, indexed by
    /// [`season_slot`], in the order `Plane::GfxBank` selects them.
    ///
    /// `Gfx_LoadCountyMode` loads exactly these five, in this order, from
    /// consecutive `g_resourceTable` entries starting at
    /// `(zoom == 2 ? 0x20 : 0) + (season - 1) * 8`. This array **is** those
    /// entries, read out of the table at `0x004DA050` rather than guessed from
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
/// not inferred. **[V]** — `docs/decisions.md` C61.
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

/// The five tile banks **in every season**, the two sprite sheets and the flag
/// sheet at both zooms, decoded on demand.
///
/// # Why the banks are a pool and an index table rather than a nested array
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
    /// spring's five banks there is no map at all; without summer's the map
    /// falls back on spring, which is a map that does not change with the year
    /// rather than no map. An install that is missing `Base1c.pl8` should still
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

    /// How many distinct bank files were actually loaded. Twenty on a complete
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

/// **`Map_DrawArmies` (`0x00408438`) — one unit's figure standing on a tile.**
///
/// The original's placement, statement for statement:
///
/// ```c
/// x = walkTableX[dir * 16 + stepAccum] + g_mapTileHalfStep;   /* 30 / 6 */
/// y = walkTableY[dir * 16 + stepAccum] + g_mapHalfPitch;      /* 30 / 6 */
/// switch (kind) { case 1: case 2: y -= 4; break;
///                 case 3: case 4: y -= 2; x -= 4; break; }
/// drawX += x;  drawY += y;
/// drawX -= spriteWidth / 2;  drawY -= spriteHeight;
/// ```
///
/// `g_mapTileHalfStep` and `g_mapHalfPitch` are **both** 30 at the near zoom and
/// both 6 at the far one — `Map_SetZoom` writes them from the same literal — and
/// the near tile is 58 × 30 and the far one 10 × 6, so the anchor is the
/// diamond's **bottom vertex, one pixel right of centre**, and the figure hangs
/// upwards from it. That is why a unit reads as standing *on* the tile rather
/// than floating in it.
///
/// **The walk table is not applied here.** `0x004D8108` … `0x004D8388` are six
/// 8 × 16 `i8` tables that drag the sprite back toward the tile it stepped out
/// of while `+0x149` counts 0 … 15; index 0 is zero in all of them, which is a
/// unit at rest, and our units have no sub-tile step state to index with. So we
/// draw every unit at rest, and a unit mid-step would sit at its destination
/// tile in the original for the same reason it does here — it is only the
/// interpolation that is missing.
///
/// Returns false when the sheet or the frame is missing, so the caller can fall
/// back to a marker of its own.
#[allow(clippy::too_many_arguments)]
pub fn draw_unit(
    canvas: &mut Canvas,
    assets: &MapAssets,
    view: Viewport,
    zoom: &Zoom,
    tile: (usize, usize),
    sprite: UnitSprite,
    clip: Clip,
) -> bool {
    let Some(sheet) = assets.sprite_sheet(zoom, sprite.sheet) else { return false };
    let Some(decoded) = sheet.frame(sprite.frame) else { return false };
    let (row, col) = tile_to_cell(tile.0, tile.1);
    let (sx, sy) = cell_to_screen(view, zoom, row, col);
    let (nx, ny) = sprite.nudge;
    let x = sx + zoom.half_pitch + nx - decoded.width as i32 / 2;
    let y = sy + zoom.half_pitch + ny - decoded.height as i32;
    canvas.blit_clipped(&decoded, x, y, clip);
    true
}

/// Where [`draw_unit`] would put a unit's figure, and the frame it would use.
///
/// The same arithmetic, factored out so that a **hit test** can ask where the
/// figure actually is rather than guessing at a box around the tile centre. A
/// sprite is anchored on the tile's bottom vertex and is taller than the tile,
/// so most of it stands over the tiles behind — see `docs/decisions.md` C57 and C58.
/// `None` when the sheet or the frame is missing, which is a caller's cue to
/// fall back on whatever it draws instead of the figure.
pub fn unit_sprite_rect(
    assets: &MapAssets,
    view: Viewport,
    zoom: &Zoom,
    tile: (usize, usize),
    sprite: UnitSprite,
) -> Option<(i32, i32, l2_formats::pl8::DecodedFrame)> {
    let decoded = assets.sprite_sheet(zoom, sprite.sheet)?.frame(sprite.frame)?;
    let (row, col) = tile_to_cell(tile.0, tile.1);
    let (sx, sy) = cell_to_screen(view, zoom, row, col);
    let (nx, ny) = sprite.nudge;
    Some((
        sx + zoom.half_pitch + nx - decoded.width as i32 / 2,
        sy + zoom.half_pitch + ny - decoded.height as i32,
        decoded,
    ))
}

/// Which sheet, which frame and which per-kind nudge one unit draws with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnitSprite {
    /// 0 for `g_spriteSheetA`, 1 for `g_spriteSheetB`. `Map_DrawArmies` picks B
    /// for `kind == 4` — a transport — and A for everything else.
    pub sheet: usize,
    pub frame: usize,
    /// The per-kind `(x, y)` the original adds before centring.
    pub nudge: (i32, i32),
}

/// **`FUN_004071A0`'s flag** — an owner-coloured banner over a tile.
///
/// Placed at `tileOrigin + Zoom::flag_at` with **no centring at all**: the
/// function adds the offset to the draw cursor and blits, and never reads the
/// frame record's `cx`/`cy`, which are atlas coordinates. Near zoom that is
/// `(+26, −28)` — up and to the right of the diamond's top-left, so the flag
/// flies above the tile.
pub fn draw_flag(
    canvas: &mut Canvas,
    assets: &MapAssets,
    view: Viewport,
    zoom: &Zoom,
    tile: (usize, usize),
    frame: usize,
    clip: Clip,
) -> bool {
    let Some(sheet) = assets.flag_sheet(zoom) else { return false };
    let Some(decoded) = sheet.frame(frame) else { return false };
    let (row, col) = tile_to_cell(tile.0, tile.1);
    let (sx, sy) = cell_to_screen(view, zoom, row, col);
    canvas.blit_clipped(&decoded, sx + zoom.flag_at.0, sy + zoom.flag_at.1, clip);
    true
}

/// **`Map_DrawPathMarker` (`0x004081A6`) — one ball of an ordered path.**
///
/// `Path_MarkPreviewTiles` (`0x004A91BA`) sets bank bit `0x40` on each step of
/// the local player's path, and this draws `g_flagsSheet` on every tile
/// carrying it. `docs/decisions.md` C49 is what this function is: for four
/// documents it was `Map_DrawCountyFlag` and was said to draw a castle's flag
/// at a frame derived from the castle level. Every clause of that was wrong,
/// and the frame arithmetic those documents quoted was **this** one's.
///
/// **The placement is ours.** The frame is centred on the tile, which is where
/// a marker for a tile goes ([`tile_centre`]); the original's own offset is not
/// traced, and the two flag offsets in [`Zoom::flag_at`] belong to
/// `FUN_004071A0`, which is a different function on a different bit.
///
/// Returns false when the sheet or the frame is missing, so the caller can fall
/// back to a mark of its own.
pub fn draw_path_marker(
    canvas: &mut Canvas,
    assets: &MapAssets,
    view: Viewport,
    zoom: &Zoom,
    tile: (usize, usize),
    frame: usize,
    clip: Clip,
) -> bool {
    let Some(sheet) = assets.flag_sheet(zoom) else { return false };
    let Some(decoded) = sheet.frame(frame) else { return false };
    let (row, col) = tile_to_cell(tile.0, tile.1);
    let (sx, sy) = cell_to_screen(view, zoom, row, col);
    let x = sx + zoom.tile_w / 2 - decoded.width as i32 / 2;
    let y = sy + zoom.tile_h / 2 - decoded.height as i32 / 2;
    canvas.blit_clipped(&decoded, x, y, clip);
    true
}

/// **The path ball's frame** — `0x38 + n`, and the frame index *is* the
/// accumulated cost.
///
/// `Map_DrawPathMarker`, in full:
///
/// ```c
/// n = distance[tile] - 1;
/// if (unit.moveAllowance - unit.movesUsed < n) n = 0;         /* out of range */
/// frame = (tile is castle/settlement/plot) ? 0x4E : 0x38 + n;
/// ```
///
/// So nothing about the realm, the shield or the unit's kind selects the
/// colour: **the cost does**, and everything past the remaining budget collapses
/// to [`PATH_MARKER_FIRST`]. `docs/armies.md` §2.3. **[V]**
///
/// # What the sheet says, now that somebody has looked at it
///
/// `docs/armies.md` carried *"**[I]** that frame `0x38` is specifically the grey
/// one — nobody has looked at the sheet"*. Measured against a real
/// `Flags1a.pl8`: frames `0x38 … 0x4E` are **23 frames, every one of them
/// 15 × 15 with the same 177-pixel silhouette** — one picture, recoloured — and
/// **`0x38` is the only frame in the run whose every opaque pixel is a true
/// grey** (`r == g == b`). Frame `0x39` has 13 coloured pixels, and the count
/// climbs to 39 by `0x4D`, so the ball gains colour as the cost rises. The
/// inference is now a measurement, and it is asserted in
/// `crates/l2-view/tests/install.rs` against the user's own file. **[V]**
///
/// `0x4E` is the odd one out and is the castle marker the ladder above names:
/// 177 coloured pixels and **not one grey**, a different picture entirely.
pub const PATH_MARKER_FIRST: usize = 0x38;

/// The last cost-indexed ball. `0x4E` is the castle/settlement marker and is
/// not part of the ramp.
pub const PATH_MARKER_LAST: usize = 0x4D;

/// `Map_DrawPathMarker`'s frame for a tile the path reaches at accumulated cost
/// `n`, with `in_range` false for a step past the remaining budget.
///
/// The clamp at the top of the ramp is ours: the original indexes
/// `0x38 + n` with no bound, and a path costing more than 21 would walk off the
/// end of the bank into `0x4E`, the castle marker. Ours stops at
/// [`PATH_MARKER_LAST`]. That is a divergence and it is a deliberate one — see
/// `docs/bugs.md`.
pub fn path_marker_frame(cost: i32, in_range: bool) -> usize {
    if !in_range || cost <= 0 {
        return PATH_MARKER_FIRST;
    }
    (PATH_MARKER_FIRST + cost as usize).min(PATH_MARKER_LAST)
}

/// **The waving flag's frame** — `shield * 8 - 8 + phase`, i.e.
/// `(shield − 1) * 8 + phase`.
///
/// `Flags1a.pl8`'s first 40 frames are 32 × 24 and lie on the artist's sheet as
/// five rows of eight: **five shields × eight wave phases**, and `shield = 5,
/// phase = 7` lands on frame 39, the last of them. The colour is in the frame
/// index; there is no palette remap. `shield` is the realm's `shieldIndex`,
/// clamped 1 … 5 by the original, so a zero shield has no flag.
pub const FLAG_PHASES: u8 = 8;

pub fn flag_frame(shield: u8, phase: u8) -> Option<usize> {
    (1..=5).contains(&shield).then(|| {
        (shield as usize - 1) * FLAG_PHASES as usize + (phase % FLAG_PHASES) as usize
    })
}

/// `Flags1a.pl8` frame `0x81`, the mercenary-offer marker, drawn on the town
/// block's north-east quadrant when the county has a band standing.
pub const MERCENARY_MARKER_FRAME: usize = 0x81;

// ------------------------------------------------------------- the cattle

/// **The herd grazing on a pasture** — `FUN_004071A0`'s farm arm, the third of
/// its four and the only one that had never been read.
///
/// The same overlay pass that draws the town's flag draws the animals, off the
/// same sheet, on the same bank bit. `Terrain_Set` sets bank bit `0x80` when
/// `0x0E < terrain && terrain < 0x17` — **exactly the pasture range**, which is
/// why a pasture is the one field state that gets a second blit at all
/// ([`field_graphic`] already carried the bit and called it *"presumably the
/// animals"*; it is).
///
/// The arm, in full, with `flags & 0x20` (farmland) and neither `0x40` nor
/// `0x10` set:
///
/// ```c
/// if (g_mapZoom == 2)        return;      /* no animals at the far zoom */
/// if (content < 0x0F)        return;
/// if (0x16 < content)        return;
/// if (content < 0x13) {                   /* the dead half — see below   */
///     dx = 0; dy = 0;
///     if (2 < (byte)(content - 0x10)) return;
///     frame = (content - 0x10) * 6 + phase + 0x67;
/// } else {
///     dx = 4; dy = -4;
///     if (2 < (byte)(content - 0x14)) return;
///     frame = (content - 0x14) * 6 + phase + 0x55;
/// }
/// ```
///
/// `phase` is `_DAT_0057D38C`, and the `* 6` is the whole shape of the sheet:
/// **three herds of six frames each**, one herd per crowding band.
///
/// # Two states in that ladder draw nothing, and it is not a mistake
///
/// `content == 0x0F` and `content == 0x13` both fall through the unsigned
/// `2 < (content - base)` guard and return — `0x13 - 0x14` is `0xFF` as a byte.
/// `0x13` is the value `l2_kingdom::land::herd_graphic` writes for **an empty
/// herd**, so a county that has lost every animal keeps its pasture and shows
/// bare grass. That is the original saying *"no cattle"* by drawing no cattle,
/// and it is the reason the guard is written as an unsigned compare rather than
/// a range. **[V]**
///
/// # The `0x67` half is vestigial, and it is not a second herd
///
/// The obvious reading of a second three-group block is *sheep*. It is not, and
/// two independent things say so.
///
/// **The game's own words.** The tile-info table at `0x004D2EC8` — 16 bytes an
/// entry, indexed by `content`, read by `TileInfo_Draw` — gives `0x0F … 0x12`
/// `L2.eng` group 30 descriptions **40 … 43** and mode **20**, which are
/// *"Over time and with continued labor, this field will return to a usable
/// state…"* through *"Almost reclaimed…"*, marked *"- Being reclaimed."* Those
/// are the **same four strings** the table gives `0x19 … 0x1C`, the live
/// reclamation ladder. `0x0F … 0x12` is an earlier encoding of reclamation that
/// moved, and the block of art reserved for it moved with it.
///
/// **The sheet.** Frames `0x67 … 0x78` are eighteen **2 × 2 stubs** — the same
/// padding as `0x4F … 0x54`, which is what makes the livestock block start on a
/// row of six — while `0x55 … 0x66` are eighteen frames of **58 × 30**, exactly
/// the near-zoom diamond. There is no second herd in the file to draw.
/// [`tests::the_pasture_frames_are_full_tiles_and_the_dead_block_is_stubs`]
/// measures both against a real `Flags1a.pl8`.
///
/// And nothing writes `0x0F … 0x12` onto a farm tile in any case. `Terrain_Set`
/// is the single writer of a tile's `content` (`maps-layers.md` §5.5) and its
/// sixteen call sites pass `0`, `1`, `2 … 0x0E`, `0x13 … 0x16`, `0x17`, `0x18`,
/// `0x19 … 0x1C` and the brush's `{0, 1, 2, 0x13, 0x19}`. `County_UpdateDwellings`
/// does write those four values, but onto a tile carrying plane-0 bit `0x10`,
/// which `FUN_004071A0` tests **before** `0x20` and sends down the dwelling arm.
///
/// The arm is reproduced rather than dropped because the ladder is what the
/// function does, and a renderer that silently narrowed it would be asserting
/// the absence rather than recording it. **[V]** — an exhaustive scan of every
/// `Terrain_Set` call site and every direct `content` write in the corpus, plus
/// the two readings above, which were arrived at separately and agree.
///
/// # There are no sheep
///
/// A pasture is a *"dairy meadow"* in `L2.eng` group 30 and its mode line is
/// *"- Cattle."* There is no sheep sprite, no sheep sheet, and no `farm_style`
/// branch anywhere that picks a species — the five styles choose grain against
/// pasture and how many fields, never what grazes. Goods 3 (*Sheep*) and 5
/// (*Wool*) carry price 0, have no `Merchant_Trade` branch and no `mercgrid.pl8`
/// cell. **[V]**, and worth writing down because the question is natural and the
/// answer is a negative that cannot be found by looking harder.
pub const HERD_PHASES: u8 = 6;

/// The first frame of the reachable herds — `content 0x14`'s six.
pub const HERD_FIRST_FRAME: usize = 0x55;
/// The first frame of the three unreachable ones — `content 0x10`'s six.
pub const HERD_DEAD_FIRST_FRAME: usize = 0x67;

/// Where the animals go relative to the tile origin, for each half of the
/// ladder. The original does not scale either by zoom — unlike
/// [`Zoom::flag_at`], which has a value per zoom — so both are literal.
pub const HERD_AT: (i32, i32) = (4, -4);
pub const HERD_DEAD_AT: (i32, i32) = (0, 0);

/// The sprite for a pasture tile: `(frame, offset)`, or `None` for a field
/// state that draws no animals.
///
/// `phase` is the animation phase, `0 … 5`; see [`HERD_PHASES`].
pub fn herd_sprite(terrain: u8, phase: u8) -> Option<(usize, (i32, i32))> {
    let phase = (phase % HERD_PHASES) as usize;
    match terrain {
        // The reachable herds: three crowding bands.
        0x14..=0x16 => Some((
            (terrain - 0x14) as usize * HERD_PHASES as usize + phase + HERD_FIRST_FRAME,
            HERD_AT,
        )),
        // Dead in the shipped game; kept because the ladder is.
        0x10..=0x12 => Some((
            (terrain - 0x10) as usize * HERD_PHASES as usize + phase + HERD_DEAD_FIRST_FRAME,
            HERD_DEAD_AT,
        )),
        // `0x0F` and `0x13` are pasture and draw nothing at all.
        _ => None,
    }
}

/// Blit one pasture's herd. Returns false when the sheet or the frame is
/// missing, so a caller can fall back to a mark of its own.
///
/// **`Zoom::id == 2` draws nothing**, which is the arm's own first line rather
/// than a simplification: the far zoom's tiles are 10 × 6 and the animals are
/// simply not drawn there.
///
/// The original's `param_1` — `if (param_1 == 1) dx -= g_mapTileHalfStep` — is
/// **not** reproduced, and its absence is the point. `FUN_00405FAC` walks an
/// offset row with `g_drawX` starting at `g_mapViewX` unshifted and adds the
/// half step only after the first cell, so `param_1 == 1` marks the
/// half-clipped left-edge tile and the subtraction puts its overlay back where
/// the diamond is. [`cell_to_screen`] shifts *every* cell of an odd row by
/// `half_pitch` instead, so the two agree cell for cell — the first at
/// `viewX − half`, the second at `viewX + half`, the third at
/// `viewX + half + pitch`. [`draw_flag`] rests on the same identity. **[D]**
pub fn draw_herd(
    canvas: &mut Canvas,
    assets: &MapAssets,
    view: Viewport,
    zoom: &Zoom,
    tile: (usize, usize),
    terrain: u8,
    phase: u8,
    clip: Clip,
) -> bool {
    if zoom.id == 2 {
        return false;
    }
    let Some((frame, (dx, dy))) = herd_sprite(terrain, phase) else { return false };
    let Some(sheet) = assets.flag_sheet(zoom) else { return false };
    let Some(decoded) = sheet.frame(frame) else { return false };
    let (row, col) = tile_to_cell(tile.0, tile.1);
    let (sx, sy) = cell_to_screen(view, zoom, row, col);
    canvas.blit_clipped(&decoded, sx + dx, sy + dy, clip);
    true
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
/// A sparse plane rather than a rewritten `MapSlot`, because the map file is
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
    /// the same value the file stores, so a caller copies rather than converts.
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

/// **`Terrain_Set` (`0x0046D7F4`) — the single writer of a tile's `content`
/// byte, and the terrain → picture map the renderer needs.** **[V]**
///
/// Every state change on the campaign map goes through it: the field brush
/// (`Field_SetType`), the seasonal crop pass, `Field_ReclaimTick`,
/// `County_DestroyField`, `Unit_TrampleTile` and `Counties_PlaceSites` all
/// call it, sixteen call sites in all. So this is not inferred from what the
/// tiles look like — it is the game's own assignment.
///
/// ```c
/// frame = ((frame - oldBase) & 3) + base + variant * 4;
/// bank  = (((bank | 1) & 0xE3) | layer) & 0x7F;
/// if (0x0E < terrain && terrain < 0x17) bank |= 0x80;
/// ```
///
/// # `& 3` is the point
///
/// Every field state is **four consecutive frames** and the tile keeps
/// whichever of the four it already had, so a repaint changes the crop without
/// changing the tile's variation. Every base below is a multiple of four
/// **except** 130 and 134, and `oldBase` exists for exactly that: it is `0x82`
/// when the *previous* terrain was `0x17` or `0x18` and zero otherwise, which
/// subtracts the offset those two introduce. (The original writes `0x82` for
/// both, not `0x82` and `0x86`; `130 & 3` and `134 & 3` are both 2, so it makes
/// no difference and the shortcut is harmless.)
///
/// The consequence is the one this function relies on: **the low two bits of a
/// farm tile's frame never change.** Whatever `L2_maps.dat` stored is the
/// variant for the life of the game, which is why [`field_frame`] can be a
/// pure function of the terrain and the stored frame rather than needing the
/// tile's history.
///
/// The claim self-checks against the map file: farm tiles on disk are bank
/// `0x20`/roads **frame 80 only**, with boundary twins 81 … 83 — which is
/// `base = 80` and its four variants exactly (`maps-layers.md` §1.2).
///
/// # `variant` is dead
///
/// The third parameter is `variant * 4` — a whole sub-block shift on top of the
/// base. **All sixteen call sites in the shipped binary pass zero**, including
/// the two that forward a parameter (`FUN_00469D21`, whose only callers are
/// `Grain_SeasonTick` and `Herd_UpdateCrowding`, and both pass `'\0'`). So the
/// term contributes nothing to any picture the game draws and this function
/// omits it. If it ever mattered it would move a field into the *next* state's
/// frame block, which is presumably why nothing uses it.
pub const FIELD_BASES: [(u8, u8, u8); 10] = [
    // (terrain, first frame of the four, plane-1 bank byte for the layer)
    (0x00, 80, BANK_ROADS),  // wild
    (0x01, 84, BANK_ROADS),  // fallow — ploughed and bare
    (0x02, 88, BANK_ROADS),  // 0x02 … 0x12: grain, and pasture up to 18
    (0x13, 104, BANK_ROADS), // 0x13 … 0x16, and anything from 0x1D up
    (0x17, 130, BANK_BASE),  // harvested — and in the **base** bank, not roads
    (0x18, 134, BANK_BASE),
    (0x19, 108, BANK_ROADS), // the four reclamation stages
    (0x1A, 112, BANK_ROADS),
    (0x1B, 116, BANK_ROADS),
    (0x1C, 120, BANK_ROADS),
];

/// The plane-1 byte for the `base` bank — `Base1?.pl8`, bank index 0.
pub const BANK_BASE: u8 = 0x00;
/// The plane-1 byte for the `roads` bank — `Roads1?.pl8`, bank index 2.
pub const BANK_ROADS: u8 = 0x08;

/// The first frame of a terrain's four, and the bank layer it draws from.
///
/// The ladder is `Terrain_Set`'s, in its own order — the specific values are
/// tested before the two ranges, which is why `0x17` and `0x18` do not fall
/// into the `0x13 …` arm and `0x19 … 0x1C` do not fall into the tail.
///
/// **The tail catches more than `maps-layers.md` §5.5's table says.** The
/// original's last arm is a bare `else`, so every terrain at `0x13` or above
/// that is not one of `0x17 … 0x1C` lands on base 104 — including `0x1D` and
/// up, which `County_RecountFields` buckets as *being reclaimed*. The
/// document's table reads as if `0x13 … 0x16` were exhaustive. It is not
/// wrong about those four; it is silent about the ones past `0x1C`.
pub fn field_base(terrain: u8) -> (u8, u8) {
    match terrain {
        0x00 => (80, BANK_ROADS),
        0x17 => (130, BANK_BASE),
        0x18 => (134, BANK_BASE),
        0x01 => (84, BANK_ROADS),
        0x19 => (108, BANK_ROADS),
        0x1A => (112, BANK_ROADS),
        0x1B => (116, BANK_ROADS),
        0x1C => (120, BANK_ROADS),
        t if t < 0x13 => (88, BANK_ROADS),
        _ => (104, BANK_ROADS),
    }
}

/// **The picture for one farm tile.** Returns `(plane-1 byte, frame)`, ready
/// for [`Overrides::set`].
///
/// `stored_frame` is the tile's graphic index as `L2_maps.dat` holds it — the
/// low two bits of which are the tile's variant, permanently (see
/// [`FIELD_BASES`]). `Map_PlaceStartingFields` writes
/// `frame = base + ((frame + 0xB0) & 3)` from the other side and `0xB0` is a
/// multiple of four, so the two functions agree on which two bits carry the
/// variant.
///
/// The bank byte reproduces `Terrain_Set`'s own arithmetic —
/// `(((bank | 1) & 0xE3) | layer) & 0x7F` — applied to the byte the file holds
/// for a farm tile, which is always `0x08`, the roads bank. That comes out
/// `0x09` for a roads-layer state and `0x01` for a base-layer one; bit `0x01`
/// is the road bit the original sets unconditionally and bits `0x1C` are the
/// bank, which is all [`draw`] reads.
///
/// **Bit `0x80` is set for terrain `0x0F … 0x16` and changes no pixel here.**
/// It is a run-time *draw* bit asking for the building-overlay blitter
/// (`maps-layers.md` §5.3) — on a pasture, presumably the animals — and this
/// crate has no such blitter, so it is carried rather than acted on:
/// [`Overrides`] stores the plane-1 byte and a caller reading it back should
/// see what the game's own tile record would hold.
pub fn field_graphic(terrain: u8, stored_frame: u8) -> (u8, u8) {
    let (base, layer) = field_base(terrain);
    let bank = ((((BANK_ROADS | 1) & 0xE3) | layer) & 0x7F)
        | if (0x0F..0x17).contains(&terrain) { 0x80 } else { 0 };
    (bank, base + (stored_frame & 3))
}

/// The frame alone, for a caller that already knows the bank.
pub fn field_frame(terrain: u8, stored_frame: u8) -> u8 {
    field_base(terrain).0 + (stored_frame & 3)
}

/// Paint the viewport, stamping county ids into `tags`. Returns tiles drawn.
///
/// The traversal is `Map_RenderIso`'s: `rows + 1` lattice rows starting at
/// `view.row`, the first and last half-clipped by the viewport rectangle rather
/// than by a special blitter. An aligned row draws `cols` cells; an offset row
/// draws `cols + 1`, because shifting left by half a pitch exposes one more
/// column on the right.
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
                    let frame = lattice.surround(row, col) as usize;
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
        assert_eq!(NEAR.bottom(), 474, "Clip_Vertical(0x18, 0x1DA) in Map_DrawPathMarker");
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
