//! Drawing a battle: the tile viewport, then the men on it.
//!
//! The geometry is the original's, taken from the arguments
//! `Battle_LoadAssets` (`0x004987B7`) passes to the tile renderer's setup
//! (`0x004BC020`):
//!
//! ```text
//! FUN_004bc020(tileset, tileset2, &g_battlefield, 0x50, 0x50, 8, 0, 0x18, 0xf, 0xe, 0x20)
//!                                                  80    80   8  0    24   15   14    32
//! ```
//!
//! **[V]** a 15 x 14 viewport of 32-pixel tiles, pinned at screen `(0, 24)`,
//! over an 80 x 80 map of 8-byte cells. The renderer that consumes it
//! (`0x004BCBDC`) steps the destination by `0x20` per tile in both axes, so the
//! battlefield is a **plain square grid seen from above** — not isometric.
//!
//! Figures are drawn between the two terrain passes, sorted by map y ascending
//! (`0x004BDA92`), which makes a man lower on the field overlap one behind him.
//! **[V]**
//!
//! # Three things a player saw, and the functions they are
//!
//! * **Where a walking man is.** `BattleMan_Step` (`0x0048F1DD`) *enters* the
//!   next cell before it walks — see [`drawn_cell`] — so the picture trails him
//!   behind the cell he is in. Ours drew the trail behind the cell he was
//!   leaving, and a player saw every man *"reset on their square once as they
//!   move"*.
//! * **The clip.** `FUN_004BC020` also stores the viewport as the rectangle
//!   every battle sprite is clipped to — [`FIELD_CLIP`]. Ours blitted men
//!   unclipped into the menu bar, the right column and the strip under the
//!   field, where nothing repaints them: the *"ghosting"*.
//! * **The palette** is not here: this crate draws indices, and which `.256`
//!   they mean is the presenter's, which resolves [`TILE_PALETTE`] through
//!   `l2_game::shell::PALETTES` like every other page's name.

use l2_formats::Palette;
use l2_sim::runner::{BattleRunner, Fighter};
use l2_sim::terrain::{Battlefield, DIM};
use l2_sim::{Troop, SIDE_A};

use crate::canvas::{Canvas, Clip};
use crate::figures::{self, Anim, Colour};
use crate::sheet::Sheet;

pub const TILE: i32 = 32;
pub const VIEW_COLS: usize = 15;
pub const VIEW_ROWS: usize = 14;
pub const ORIGIN_X: i32 = 0;
pub const ORIGIN_Y: i32 = 24;

/// **The rectangle every man, horse and missile on the field is clipped to.**
///
/// `FUN_004BC020` (`0x004BC020`) stores it beside the geometry —
/// `DAT_004E6564 = param_7`, `DAT_004E5D54 = param_9 * param_11 + param_7`,
/// `DAT_004E5D48 = param_8`, `DAT_004E5D6C = param_10 * param_11 + param_8` —
/// which for the battle's arguments is `x 0 … 480`, `y 24 … 472`. And
/// `BattleFigure_Draw` (`0x004BDC31`), the horse under a knight
/// (`FUN_004BE4DF`) and the tile renderer's overlay blit all call
/// `Clip_Horizontal(DAT_004E6564, DAT_004E5D54)` and
/// `Clip_Vertical(DAT_004E5D48, DAT_004E5D6C)` before they blit. **[V]**
///
/// A man in the top row is drawn sixteen pixels above his cell, and one a cell
/// outside the view is still collected (`FUN_004BD938`), so without this the
/// menu bar, the right column and the bottom strip took his pixels — and
/// nothing on the battlefield screen repaints any of those.
pub const FIELD_CLIP: Clip = Clip::new(
    ORIGIN_X,
    ORIGIN_Y,
    ORIGIN_X + VIEW_COLS as i32 * TILE,
    ORIGIN_Y + VIEW_ROWS as i32 * TILE,
);

/// The field-battle tileset. `T32_stn1` / `T32_wod1` are the siege and
/// wooded-castle variants and are not loaded here.
pub const TILESET: &str = "T32_bat1.pl8";

/// **The two battle palettes**, records 2 and 1 of `g_preloadTable`
/// (`0x004D9F48`) — the filenames are the table's own bytes.
///
/// **Neither is `Battle_LoadAssets`'.** `Res_LoadStatic` (`0x00499859`)
/// preloads both at start-up, into `0x00568EE0` and `0x005675A0`, and
/// `Screen_DrawBattlefield` (`0x004233F7`) ends every repaint with
/// `if (g_battleIsSiege == 0) Palette_Set(0x568ee0); else
/// Palette_Set(0x5675a0);`. **[V]**
///
/// The siege one is the palette of a screen we draw from the *field* tileset,
/// because `T32_stn1.pl8` is not ported. That is the original's colour over
/// the wrong tiles, which is what the original's own siege men and walls are
/// drawn in; the alternative measured worse — a whole siege in the field's
/// colours.
pub const TILE_PALETTE: &str = "T32_bat1.256";
pub const SIEGE_PALETTE: &str = "T32_stn1.256";

/// **The overview panel's two sheets.** `Battle_LoadAssets` (`0x004987B7`)
/// registers them with
///
/// ```text
/// FUN_004BC107(DAT_0053F044, DAT_0056D680, DAT_0056D5A0, 0x1E0, 0x18, 2)
///              t2_bat1.pl8   t2_bat2.pl8   t2_spri.pl8   480    24    mode
/// ```
///
/// and the three buffers are entries `0x0B`, `0x0C` and `0x11` of the battle
/// asset table at `0x004DA550` — `t2_bat1.pl8`, `t2_bat2.pl8`, `t2_spri.pl8`
/// for a field battle, `t2_stn1`/`t2_stn2` or `t2_wod1`/`t2_wod2` in place of
/// the first two for a siege. **[V]** from the table's bytes and the loader's
/// `local_10 → local_18` ladder.
///
/// **The second sheet is never drawn in a field battle.** `t2_bat2.pl8`'s size
/// in that table is `0`, the file is not in the install, and the loader's
/// non-siege arm jumps over its slot entirely; `FUN_004BC51A` reaches it only
/// when a cell's flag byte has `flags & 0x1C == 4`. So the field's raster is
/// `t2_bat1.pl8` alone. **[V]**
pub const OVERVIEW_TILESET: &str = "T2_bat1.pl8";
pub const OVERVIEW_SPRITES: &str = "T2_spri.pl8";

/// Two pixels a cell, at `(0x1E0, 0x18)` — `FUN_004BC51A`'s
/// `g_drawY = row * 2 + _DAT_004E5D60`, `g_drawX = DAT_004E5D68` then `+= 2`
/// a column. 80 × 80 cells makes a 160 × 160 raster, which ends exactly where
/// `Screen_DrawBattlefield` puts `Misc_bat.pl8` frame 0, at `(0x1E0, 0xB8)`.
/// **[V]**
pub const OVERVIEW_ORIGIN_X: i32 = 0x1E0;
pub const OVERVIEW_ORIGIN_Y: i32 = 0x18;
pub const OVERVIEW_SCALE: i32 = 2;
pub const OVERVIEW_SIDE: usize = DIM * OVERVIEW_SCALE as usize;

/// **Four rows a frame** — `Battle_Frame` (`0x004B99C0`) calls
/// `FUN_004BC1D1(4)` once a frame while `g_battlePhase == 2` and the screen is
/// `0x28 … 0x2A`, and `FUN_004BC1D1` advances a row cursor by its argument,
/// wraps it to zero at `DAT_004E6570 − n` (the map's 80 rows), and paints
/// `FUN_004BC51A(cursor, n)`. A full sweep of the panel therefore takes
/// **20 frames**. **[V]**
pub const OVERVIEW_ROWS_PER_FRAME: usize = 4;

/// The two 2 × 2 sheets the overview panel is built from. Optional on
/// [`BattleAssets`] because an install that lacks them still plays — the panel
/// falls back to the flat fill.
pub struct OverviewSheets {
    /// `t2_bat1.pl8`: 252 frames of 2 × 2, one for each of `T32_bat1.pl8`'s 252
    /// 32 × 32 tiles, so a cell's `gfx` byte indexes both. **[V]** from the two
    /// files' headers.
    pub tiles: Sheet,
    /// `t2_spri.pl8`: seven frames of 2 × 2. Frame 0 is the erase tile
    /// `FUN_004BC51A` stamps on a cell a man has just left; frames 1 … 6 are
    /// flat colours, indexed by the owning realm's `shieldIndex` — or 6 for the
    /// neutral owner. **[V]**
    pub men: Sheet,
}

/// Where the camera's top-left tile is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Camera {
    pub x: usize,
    pub y: usize,
}

impl Camera {
    /// Clamp so the viewport never runs off the 80 x 80 map, which is what the
    /// original's scroll clamp does.
    pub fn clamped(x: i32, y: i32) -> Self {
        Camera {
            x: x.clamp(0, (DIM - VIEW_COLS) as i32) as usize,
            y: y.clamp(0, (DIM - VIEW_ROWS) as i32) as usize,
        }
    }

    /// Centre the viewport on a cell.
    pub fn centred_on(x: usize, y: usize) -> Self {
        Camera::clamped(x as i32 - VIEW_COLS as i32 / 2, y as i32 - VIEW_ROWS as i32 / 2)
    }
}

/// Everything a battle needs to draw. Sheets are loaded once and their frames
/// decoded on demand.
pub struct BattleAssets {
    pub palette: Palette,
    pub tiles: Sheet,
    /// One sheet per troop type that has one, for each side. `None` for the
    /// siege engines, which are drawn from other files entirely.
    side4: Vec<Option<Sheet>>,
    side0: Vec<Option<Sheet>>,
    horse: Option<Sheet>,
    /// [`OverviewSheets`], when the install has both files.
    pub overview: Option<OverviewSheets>,
}

/// The seven troop types with battlefield sprites, indexed by
/// `Troop::index()`.
const SPRITE_TROOPS: [Troop; 7] = [
    Troop::Peasants,
    Troop::Crossbowmen,
    Troop::Macemen,
    Troop::Swordsmen,
    Troop::Pikemen,
    Troop::Archers,
    Troop::Knights,
];

impl BattleAssets {
    /// Load through a caller-supplied reader, so this works equally against a
    /// plain directory and against the mod overlay's case-insensitive VFS.
    pub fn load<F>(mut read: F, side4: Colour, side0: Colour) -> Result<Self, String>
    where
        F: FnMut(&str) -> Result<Vec<u8>, String>,
    {
        let palette = Palette::from_bytes(&read(TILE_PALETTE)?)
            .map_err(|e| format!("{TILE_PALETTE}: {e}"))?;
        let tiles = Sheet::new(read(TILESET)?).map_err(|e| format!("{TILESET}: {e}"))?;

        let mut load_side = |colour: Colour| -> Result<Vec<Option<Sheet>>, String> {
            let mut out: Vec<Option<Sheet>> = (0..11).map(|_| None).collect();
            for troop in SPRITE_TROOPS {
                let Some(name) = figures::sprite_file(colour, troop) else { continue };
                let bytes = read(&name)?;
                out[troop.index()] = Some(Sheet::new(bytes).map_err(|e| format!("{name}: {e}"))?);
            }
            Ok(out)
        };
        let a = load_side(side4)?;
        let b = load_side(side0)?;
        let horse = read(figures::HORSE_FILE).ok().and_then(|b| Sheet::new(b).ok());
        let mut sheet = |name: &str| read(name).ok().and_then(|b| Sheet::new(b).ok());
        let overview = match (sheet(OVERVIEW_TILESET), sheet(OVERVIEW_SPRITES)) {
            (Some(tiles), Some(men)) => Some(OverviewSheets { tiles, men }),
            _ => None,
        };

        Ok(BattleAssets { palette, tiles, side4: a, side0: b, horse, overview })
    }

    fn sheet_for(&self, side: l2_sim::Side, troop: Troop) -> Option<&Sheet> {
        let bank = if side == SIDE_A { &self.side0 } else { &self.side4 };
        bank.get(troop.index()).and_then(|s| s.as_ref())
    }
}

/// Paint the terrain viewport. Tiles are blitted opaque: the original's tile
/// path does not test for index 0, and no frame of `T32_bat1.pl8` contains one.
pub fn draw_terrain(canvas: &mut Canvas, field: &Battlefield, tiles: &Sheet, cam: Camera) {
    for row in 0..VIEW_ROWS {
        for col in 0..VIEW_COLS {
            let (mx, my) = (cam.x + col, cam.y + row);
            if mx >= DIM || my >= DIM {
                continue;
            }
            let gfx = field.at(mx, my).gfx as usize;
            let Some(frame) = tiles.frame(gfx) else { continue };
            canvas.blit_opaque(
                &frame,
                ORIGIN_X + col as i32 * TILE,
                ORIGIN_Y + row as i32 * TILE,
            );
        }
    }
}

/// **The cell a figure is drawn from, and the original's `walking` counter
/// for it** — figure `+0x32`, the column of `g_walkOffset32`.
///
/// `BattleMan_Step` (`0x0048F1DD`) takes a step in this order:
///
/// ```c
/// local_10 = BattleMan_TryStepDir(dir);          /* Cell_TryEnter: 1 = free */
/// if (local_10 == 1) {
///     man.dirc = dir;  man.walking = 1;
///     FUN_00491B1F(man);                         /* mapX/mapY += the delta, cell byte moved */
/// }
/// …and every later tick: walking += 2; if (walking >= 17) { stepFlags |= 1; walking = 0; }
/// ```
///
/// So the original's man is **in** the cell he is walking into for the whole
/// crossing, and `BattleFigure_Draw` (`0x004BDC31`) trails him `32 − 2·walking`
/// pixels behind it: 30, 26 … 2, then 0. **[V]**
///
/// `l2_sim`'s runner now crosses in that order too, so this is a plain read of
/// `mapX`/`mapY` and `walking` with no compensation in it.
///
/// **It used to compensate, and a compensation cannot fix an order.** The
/// runner counted `substep` 2 … 16 on the cell it was leaving and entered on
/// the last one, so this returned the cell the facing pointed into with
/// `walking = substep − 1`. That got the trail right and left two faults the
/// drawing cannot reach: a step refused *after* the count put the man back on
/// the square he never left, and the runner re-chose his direction every tick,
/// so he could turn mid-crossing and be drawn jumping sideways. 324 drawn
/// jumps of half a cell or more over a 42-figure battle; 0 once
/// `BattleRunner::step_one` took `BattleMan_Step`'s order. `docs/battle.md`
/// §13.6, `docs/decisions.md` C200.
pub fn drawn_cell(f: &Fighter) -> ((i32, i32), u8) {
    let s = f.progress.substep;
    let walking = if f.anim == Anim::Walking { s.min(16) as u8 } else { 0 };
    ((f.x as i32, f.y as i32), walking)
}

/// **The pixel a figure's cell corner is drawn at**, trail included —
/// `BattleFigure_Draw`'s `(mapXY − cameraXY) · 32 + origin +
/// g_walkOffset32[dirc][walking]`, before the sprite is centred on it. See
/// [`drawn_cell`] for which cell and which `walking`.
pub fn figure_origin(f: &Fighter, cam: Camera) -> (i32, i32) {
    let ((cx, cy), walking) = drawn_cell(f);
    let (ox, oy) = figures::walk_offset(f.facing, walking);
    (
        ORIGIN_X + (cx - cam.x as i32) * TILE + ox,
        ORIGIN_Y + (cy - cam.y as i32) * TILE + oy,
    )
}

/// Paint the figures, back to front by map y.
///
/// Returns how many were drawn, which is what the headless tests assert on
/// when they do not want to pin exact artwork.
pub fn draw_figures(
    canvas: &mut Canvas,
    runner: &BattleRunner,
    assets: &BattleAssets,
    cam: Camera,
) -> usize {
    // `FUN_004BD938` collects every man within one cell of the viewport,
    // `FUN_004BDA92` bubble-sorts them by map y and `FUN_004BDB95` draws in
    // that order. The cell both of them read is `mapX`/`mapY`, which is the
    // cell a walking man is walking *into* — [`drawn_cell`]. A stable sort by y
    // reproduces the bubble sort, ties resolving to figure index either way.
    let (camx, camy) = (cam.x as i32, cam.y as i32);
    let mut order: Vec<(i32, usize)> = (0..runner.fighters.len())
        .filter_map(|i| {
            let ((x, y), _) = drawn_cell(&runner.fighters[i]);
            let inside = x >= camx - 1
                && x <= camx + VIEW_COLS as i32
                && y >= camy - 1
                && y <= camy + VIEW_ROWS as i32;
            inside.then_some((y, i))
        })
        .collect();
    order.sort();

    let mut drawn = 0;
    for (_, i) in order {
        let f = &runner.fighters[i];
        let Some(sheet) = assets.sheet_for(f.side, f.troop) else { continue };

        let (cell_x, cell_y) = figure_origin(f, cam);

        // A knight rides; the horse goes down first — `FUN_004BE4DF`, through
        // the same clip.
        if f.troop == Troop::Knights {
            if let Some(horse) = &assets.horse {
                if let Some(frame) = horse.frame(figures::horse_frame(f.facing, f.phase)) {
                    let w = frame.width as i32;
                    canvas.blit_clipped(
                        &frame,
                        cell_x + (TILE / 2 - w / 2),
                        cell_y - w / 2 + 8,
                        FIELD_CLIP,
                    );
                }
            }
        }

        let index = figures::frame(f.troop, f.anim, f.facing, f.phase);
        let Some(frame) = sheet.frame(index) else { continue };
        // `BattleFigure_Draw` centres on the cell using the sprite *width* for
        // both axes, which is why a 48-pixel man sits eight pixels left of and
        // sixteen above his cell's corner. Reproduced rather than corrected.
        let w = frame.width as i32;
        canvas.blit_clipped(&frame, cell_x + (TILE / 2 - w / 2), cell_y - w / 2 + 8, FIELD_CLIP);
        drawn += 1;
    }
    drawn
}

/// **The overview panel, `rows` rows of it starting at `row`** —
/// `FUN_004BC51A(param_1, param_2)` (`0x004BC51A`), the painter
/// `Battle_LoadAssets` registers and `Battle_Frame` schedules.
///
/// ```c
/// DAT_004E6588 = stride * width * param_1;              /* the first cell   */
/// g_drawY      = param_1 * 2 + _DAT_004E5D60;           /* 24 + 2 a row     */
/// for (row = param_1; row < param_1 + param_2; row++) {
///     g_drawX = DAT_004E5D68;                           /* 480              */
///     for (col = 0; col < width; col++) {
///         flags = cell[+2];  occupant = cell[+5];
///         if (g_mapRedraw || (flags & 3)) {
///             if (occupant == 0) {  /* terrain: frame cell[+3], sheet by flags & 0x1C */ }
///             else {
///                 colour = men[occupant].owner == 6 ? 6
///                        : g_realms[men[occupant].owner].shieldIndex;
///                 if (colour != 0) Pl8_DrawFrameHere(t2_spri, colour, …);
///             }
///         }
///         g_drawX += 2;
///     }
///     g_drawY += 2;
/// }
/// ```
///
/// `occupants` is one byte a cell in the same order as the field: the
/// `t2_spri.pl8` frame for the man standing there, `0` for empty ground — which
/// is exactly the original's `shieldIndex`, with its `!= 0` guard folded in.
/// The cell a walking man occupies is the one he is walking *into*
/// ([`drawn_cell`]): `FUN_00491B1F` moves his cell byte at the start of the
/// crossing, and `+5` is that byte.
///
/// **Three things the original does here and this does not**, each because our
/// cells do not carry byte `+2`:
///
/// * the per-cell dirty bits, `flags & 3` — we repaint every cell of the rows
///   we visit, which is what `g_mapRedraw` makes the original do anyway;
/// * `flags & 0x1C == 4`, the second tileset, which a field battle never
///   reaches (see [`OVERVIEW_TILESET`]);
/// * the erase tile, `t2_spri` frame 0, drawn over a cell whose `flags & 2` is
///   set and whose occupant has gone — one pass later that cell draws its
///   terrain again, and repainting the whole row goes straight there.
///
/// **And one branch that is dead in a battle**: column 0 draws `t2_spri` frame
/// 0 when `g_appPhase == 3`, and `g_appPhase` is past 8 by the time `App_Draw`
/// runs, let alone a battle. **[V]** on `g_appPhase`'s two writes.
///
/// There is **no viewport rectangle** anywhere in `FUN_004BC51A`, and nothing
/// else writes inside `(0x1E0, 0x18)`–`(0x280, 0xB8)`.
pub fn draw_overview_rows(
    raster: &mut Canvas,
    field: &Battlefield,
    occupants: &[u8],
    sheets: &OverviewSheets,
    row: usize,
    rows: usize,
) {
    for y in row..(row + rows).min(DIM) {
        for x in 0..DIM {
            let (px, py) = (x as i32 * OVERVIEW_SCALE, y as i32 * OVERVIEW_SCALE);
            let colour = occupants.get(y * DIM + x).copied().unwrap_or(0);
            let frame = if colour == 0 {
                sheets.tiles.frame(field.at(x, y).gfx as usize)
            } else {
                sheets.men.frame(colour as usize)
            };
            if let Some(frame) = frame {
                raster.blit_opaque(&frame, px, py);
            }
        }
    }
}

/// One whole frame: terrain, then figures.
pub fn draw(canvas: &mut Canvas, runner: &BattleRunner, assets: &BattleAssets, cam: Camera) -> usize {
    draw_terrain(canvas, &runner.field, &assets.tiles, cam);
    draw_figures(canvas, runner, assets, cam)
}

/// Where the camera should sit to watch the fighting.
///
/// Not the mean of everybody: the two armies deploy forty cells apart and their
/// midpoint is empty ground that neither of them is on. So it centres on the
/// men who are *fighting* when anybody is, and otherwise on side 0 — the side
/// the player would be driving. Integer arithmetic throughout; the camera reads
/// simulation state and never writes it.
pub fn follow(runner: &BattleRunner) -> Camera {
    let centre_of = |pick: &dyn Fn(usize) -> bool| -> Option<Camera> {
        let (mut sx, mut sy, mut n) = (0i32, 0i32, 0i32);
        for (i, f) in runner.fighters.iter().enumerate() {
            if runner.is_alive(i) && pick(i) {
                sx += f.x as i32;
                sy += f.y as i32;
                n += 1;
            }
        }
        (n > 0).then(|| Camera::centred_on((sx / n) as usize, (sy / n) as usize))
    };

    centre_of(&|i| runner.fighters[i].anim == Anim::Attacking)
        .or_else(|| centre_of(&|i| runner.fighters[i].side == SIDE_A))
        .or_else(|| centre_of(&|_| true))
        .unwrap_or_else(|| Camera::centred_on(DIM / 2, DIM / 2))
}

#[cfg(test)]
mod tests {
    use super::*;
    use l2_sim::runner::Army;

    #[test]
    fn the_viewport_matches_the_original_and_fits_the_screen() {
        assert_eq!(VIEW_COLS as i32 * TILE, 480);
        assert_eq!(VIEW_ROWS as i32 * TILE + ORIGIN_Y, 472);
        assert!(VIEW_COLS as i32 * TILE <= crate::canvas::WIDTH as i32);
        assert!(ORIGIN_Y + VIEW_ROWS as i32 * TILE <= crate::canvas::HEIGHT as i32);
    }

    /// The clip, against `FUN_004BC020`'s four stores worked out by hand for the
    /// battle's arguments `(…, 0, 0x18, 0xF, 0xE, 0x20)`.
    #[test]
    fn the_sprite_clip_is_the_one_fun_004bc020_stores() {
        assert_eq!(FIELD_CLIP, Clip::new(0, 0x18, 0xF * 0x20, 0xE * 0x20 + 0x18));
    }

    #[test]
    fn the_camera_never_lets_the_viewport_leave_the_map() {
        assert_eq!(Camera::clamped(-5, -5), Camera { x: 0, y: 0 });
        assert_eq!(Camera::clamped(500, 500), Camera { x: 65, y: 66 });
        let c = Camera::centred_on(40, 40);
        assert!(c.x + VIEW_COLS <= DIM && c.y + VIEW_ROWS <= DIM);
    }

    /// **A man walking east is drawn further east every tick, from the running
    /// simulation** — the ungated half of `tests/battle_picture.rs`, which finds
    /// him in the pixels.
    ///
    /// One maceman, ordered five cells east over open ground, stepped by the
    /// real runner. The origin this module draws him at must never move west,
    /// must move on at least eight sub-steps a cell, and must end exactly 160
    /// pixels east — the original's `g_walkOffset32` column `walking` 1, 3 … 15
    /// and then the cell.
    ///
    /// Ablation: return `(f.x + dx, f.y + dy)` with `walking = substep − 1`
    /// from [`drawn_cell`], the compensation this file carried while the
    /// runner crossed in the other order — red on the first sub-step, a whole
    /// cell east of where he is.
    #[test]
    fn a_man_walking_east_is_drawn_further_east_every_tick() {
        let mut layer = vec![0u8; l2_sim::terrain::CELLS];
        layer[36 * DIM + 40] = 0x04;
        layer[74 * DIM + 40] = 0x0F;
        let field = l2_sim::terrain::build(&layer, 1);
        let mut runner = BattleRunner::deploy_armies(
            field,
            0x5EED,
            Army { troops: &[(Troop::Peasants, 1)], owner: 2, human: false },
            Army { troops: &[(Troop::Macemen, 1)], owner: 1, human: true },
        );
        let man = runner.fighters.iter().position(|f| f.side == SIDE_A).unwrap();
        let (x0, y0) = (runner.fighters[man].x, runner.fighters[man].y);
        runner.order_side(SIDE_A, x0 + 5, y0);
        let cam = Camera::clamped(x0 as i32 - 4, y0 as i32 - 6);

        let mut xs = vec![figure_origin(&runner.fighters[man], cam).0];
        for _ in 0..(5 * 18 + 40) {
            runner.step();
            let (x, y) = figure_origin(&runner.fighters[man], cam);
            assert_eq!(y, ORIGIN_Y + (y0 as i32 - cam.y as i32) * TILE, "he left his row");
            xs.push(x);
        }
        for (t, w) in xs.windows(2).enumerate() {
            assert!(w[1] >= w[0], "tick {t}: drawn {} pixels west — {xs:?}", w[0] - w[1]);
        }
        assert_eq!(xs.last().unwrap() - xs[0], 5 * TILE, "{xs:?}");
        let steps = xs.windows(2).filter(|w| w[1] > w[0]).count();
        assert!(steps >= 5 * 8, "{steps} forward steps over five cells: {xs:?}");
        assert_eq!((runner.fighters[man].x, runner.fighters[man].y), (x0 + 5, y0));
    }
}
