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
use crate::figures::{self, Anim, Colour, FACING_DELTA};
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

/// The field-battle tileset and palette. `T32_stn1` / `T32_wod1` are the siege
/// and wooded-castle variants and are not loaded here.
///
/// **The palette is not `Battle_LoadAssets`'.** `Res_LoadStatic` (`0x00499859`)
/// preloads `t32_bat1.256` into `0x00568EE0` at start-up — record 2 of
/// `g_preloadTable` (`0x004D9F48`) — and `Screen_DrawBattlefield`
/// (`0x004233F7`) ends a field battle's repaint with `Palette_Set(0x568EE0)`,
/// or `Palette_Set(0x5675A0)` (`t32_stn1.256`, record 1) for a siege. **[V]**
pub const TILESET: &str = "T32_bat1.pl8";
pub const TILE_PALETTE: &str = "T32_bat1.256";

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

        Ok(BattleAssets { palette, tiles, side4: a, side0: b, horse })
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
/// `l2_sim`'s runner does the same crossing in the other order — it counts
/// `substep` 2 … 16 on the cell he is leaving and enters on the ninth sub-step
/// (`BattleRunner`'s mover, then `enter`). Drawing the trail from `(x, y)`
/// therefore put a man up to 28 pixels *behind his own square*, walked him back
/// onto it, and jumped him a whole cell: once a cell, which is what a player
/// described. This reads the runner's state and changes none of it: while a
/// man is walking with `substep` `s > 0` he is drawn from the cell his facing
/// points into with `walking = s − 1`, which is the original's 1, 3 … 15, and
/// the tick he commits he is on that cell at 0 — the same nine pictures.
///
/// **What the picture cannot hide**, because it is the runner's order and not
/// the drawing: a step refused *after* the eight sub-steps — a friend in the
/// way, a swap, an enemy — puts him back at `walking = 0` on the square he
/// never left; and the runner re-chooses his direction every tick of the count,
/// so a man who turns mid-crossing is drawn jumping to the new neighbour. The
/// original tests the cell before it walks and holds `dirc` for the crossing,
/// and shows neither. `docs/battle.md` §13.6 has the measurement.
pub fn drawn_cell(f: &Fighter) -> ((i32, i32), u8) {
    let s = f.progress.substep;
    if f.anim == Anim::Walking && (1..=16).contains(&s) {
        let (dx, dy) = FACING_DELTA[f.facing as usize % FACING_DELTA.len()];
        ((f.x as i32 + dx, f.y as i32 + dy), (s - 1) as u8)
    } else {
        ((f.x as i32, f.y as i32), 0)
    }
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
    /// Ablation: `figures::walk_offset(f.facing, f.progress.substep as u8)` from
    /// `(f.x, f.y)` in [`figure_origin`], which is what this file drew before —
    /// red on the first sub-step, 28 pixels west.
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
