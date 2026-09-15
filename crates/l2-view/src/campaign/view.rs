#![allow(unused_imports)]
use super::*;
use super::assets::*;
use terrain::*;
use render::*;
use l2_formats::maps::{MapSlot, Plane, LATTICE_H, LATTICE_W, PLANE_DIM};
use crate::canvas::{Canvas, Clip, Tags};
use crate::sheet::Sheet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Zoom {
    pub id: u8,
    pub set: usize,
    pub tile_w: i32,
    pub tile_h: i32,
    pub pitch: i32,
    pub half_pitch: i32,
    pub row_step: i32,
    pub cols: i32,
    pub rows: i32,
    pub view_x: i32,
    pub view_y: i32,
    pub scroll_step: i32,
    /// `Gfx_LoadCountyMode` loads exactly these five, in this order, from
    /// consecutive `g_resourceTable` entries starting at
    /// `(zoom == 2 ? 0x20 : 0) + (season - 1) * 8`. This array **is** those
/// entries, read out of the table at `0x004DA050`
    /// the filenames on disk — which matters, because the far zoom's four rows
    /// are not four files. See [`SEASON_SUFFIX`].
    pub banks: [[&'static str; 5]; SEASONS],
    pub sprites: [&'static str; 2],
    pub flags: &'static str,
    /// `FUN_004071A0` adds this and blits the frame there with no further
    /// centring: the frame's `cx`/`cy` fields are atlas coordinates and the
    /// function never reads them.
    pub flag_at: (i32, i32),
    /// Ten pixels left and ten up of the banner at the near zoom, identical at
    /// the far one. `local_c` differs too — the banner marks a 48-pixel dirty
    /// square through `Gfx_MarkTile48` and the mercenary marks **nothing**,
    /// which is a repaint economy we do not have and do not need. **[V]**
    pub mercenary_at: (i32, i32),
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

/// so `base + (season - 1) * 8` lands on the same five filenames whichever
/// season it is. Read out of the table at `0x004DA050` in the shipped binary,
/// not inferred. **[V]** — `docs/decisions.md` C63.
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

pub const ZOOMS: [Zoom; 2] = [NEAR, FAR];

impl Zoom {
    pub const fn top(&self) -> i32 {
        self.view_y + self.row_step
    }

    pub const fn bottom(&self) -> i32 {
        self.view_y + self.row_step * (self.rows + 1)
    }

    pub const fn right(&self) -> i32 {
        self.view_x + self.pitch * self.cols
    }

    pub const fn clip(&self) -> Clip {
        Clip::new(self.view_x, self.top(), PANEL_X, self.bottom())
    }

    pub const fn max_col(&self) -> i32 {
        64 - self.cols
    }

    pub const fn max_row(&self) -> i32 {
        128 - self.rows
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Viewport {
    pub row: i32,
    pub col: i32,
}

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

pub const DIRS: [Dir; 8] =
    [Dir::N, Dir::NE, Dir::E, Dir::SE, Dir::S, Dir::SW, Dir::W, Dir::NW];

impl Dir {
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
    pub const START: Viewport = Viewport { row: 0x4A, col: 0x14 };

    pub const fn new(row: i32, col: i32) -> Viewport {
        Viewport { row, col }
    }

    pub fn clamped(self, zoom: &Zoom) -> Viewport {
        Viewport {
            row: (self.row.clamp(0, zoom.max_row())) & !1,
            col: self.col.clamp(0, zoom.max_col()),
        }
    }

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

    pub fn centred_on_cell(row: i32, col: i32, zoom: &Zoom) -> Viewport {
        Viewport { row: (row & !1) - 12, col: col - 4 }.clamped(zoom)
    }

    pub fn centred_on_tile(x: usize, y: usize, zoom: &Zoom) -> Viewport {
        let (row, col) = tile_to_cell(x, y);
        Viewport::centred_on_cell(row, col, zoom)
    }
}

pub type Fog<'a> = Option<&'a dyn Fn(usize, usize) -> bool>;

/// `fog` is the one input the five terrain painters of the original read that
/// is not the map, and it changes two things, each **`[V]`** out of the
/// decompilation:
///
///   `Map_DrawTile` (`0x004063C1`):
///
///   `if (g_optExploration == 1 && (bank & 0x20) == 0) { bank = 0; frame = 0; }`,
///   and `Map_DrawTileApex` (`0x00406673`) draws its overhang only under the
/// opposite test — so a mountain, a town or a castle in the dark is flat.
///
/// * **The whole off-map surround is frame 0 while the option is on**, seen or
///   not. `Map_RenderIso` (`0x0040526E`), `Map_RenderAlignedRow` (`0x00405AE9`)
///   and `Map_RenderOffsetRow` (`0x00405C2F`), all six surround arms:
///
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
        let span = if row & 1 == 1 { zoom.cols + 1 } else { zoom.cols };
        for dc in 0..span {
            let col = view.col + dc;
            let (sx, sy) = cell_to_screen(view, zoom, row, col);
            match lattice.tile(row, col) {
                Some((x, y)) if fog.is_some_and(|hidden| hidden(x, y)) => {
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
    let overhang = (decoded.height as i32 - zoom.tile_h).max(0);
    canvas.blit_clipped_tagged(&decoded, sx, sy - overhang, clip, tags, county);
    true
}

