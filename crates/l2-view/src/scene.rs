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

use l2_formats::Palette;
use l2_sim::{Troop, SIDE_A};

use crate::battle::BattleRunner;
use crate::canvas::Canvas;
use crate::figures::{self, Anim, Colour};
use crate::sheet::Sheet;
use crate::terrain::{Battlefield, DIM};

pub const TILE: i32 = 32;
pub const VIEW_COLS: usize = 15;
pub const VIEW_ROWS: usize = 14;
pub const ORIGIN_X: i32 = 0;
pub const ORIGIN_Y: i32 = 24;

/// The field-battle tileset and palette. `T32_stn1` / `T32_wod1` are the siege
/// and wooded-castle variants and are not loaded here.
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
    // The original collects the visible figures, bubble-sorts them by map y and
    // draws in that order. A stable sort by y reproduces it, ties resolving to
    // figure index either way.
    let mut order: Vec<usize> = (0..runner.fighters.len())
        .filter(|&i| {
            let f = &runner.fighters[i];
            let (x, y) = (f.x as usize, f.y as usize);
            x + 1 >= cam.x
                && x <= cam.x + VIEW_COLS
                && y + 1 >= cam.y
                && y <= cam.y + VIEW_ROWS
        })
        .collect();
    order.sort_by_key(|&i| (runner.fighters[i].y, i));

    let mut drawn = 0;
    for i in order {
        let f = &runner.fighters[i];
        let Some(sheet) = assets.sheet_for(f.side, f.troop) else { continue };

        let (ox, oy) = figures::walk_offset(f.facing, f.progress.substep as u8);
        let cell_x = ORIGIN_X + (f.x as i32 - cam.x as i32) * TILE;
        let cell_y = ORIGIN_Y + (f.y as i32 - cam.y as i32) * TILE;

        // A knight rides; the horse goes down first.
        if f.troop == Troop::Knights {
            if let Some(horse) = &assets.horse {
                if let Some(frame) = horse.frame(figures::horse_frame(f.facing, f.phase)) {
                    let w = frame.width as i32;
                    canvas.blit(
                        &frame,
                        cell_x + ox + (TILE / 2 - w / 2),
                        cell_y + oy - w / 2 + 8,
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
        canvas.blit(&frame, cell_x + ox + (TILE / 2 - w / 2), cell_y + oy - w / 2 + 8);
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

    #[test]
    fn the_viewport_matches_the_original_and_fits_the_screen() {
        assert_eq!(VIEW_COLS as i32 * TILE, 480);
        assert_eq!(VIEW_ROWS as i32 * TILE + ORIGIN_Y, 472);
        assert!(VIEW_COLS as i32 * TILE <= crate::canvas::WIDTH as i32);
        assert!(ORIGIN_Y + VIEW_ROWS as i32 * TILE <= crate::canvas::HEIGHT as i32);
    }

    #[test]
    fn the_camera_never_lets_the_viewport_leave_the_map() {
        assert_eq!(Camera::clamped(-5, -5), Camera { x: 0, y: 0 });
        assert_eq!(Camera::clamped(500, 500), Camera { x: 65, y: 66 });
        let c = Camera::centred_on(40, 40);
        assert!(c.x + VIEW_COLS <= DIM && c.y + VIEW_ROWS <= DIM);
    }
}
