#![allow(unused_imports)]
use super::*;
use super::assets::*;
use terrain::*;
use render::*;
use l2_formats::maps::{MapSlot, Plane, LATTICE_H, LATTICE_W, PLANE_DIM};
use crate::canvas::{Canvas, Clip, Tags};
use crate::sheet::Sheet;

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
/// four seasons of the player's own files (`crates/l2-game/tests/screens_map/main.rs`,
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

