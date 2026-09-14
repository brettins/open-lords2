#![allow(unused_imports)]
use super::*;

use l2_formats::Palette;
use l2_sim::runner::{BattleRunner, Fighter};
use l2_sim::terrain::{Battlefield, DIM};
use l2_sim::{Troop, SIDE_A};
use crate::canvas::{Canvas, Clip};
use crate::engines;
use crate::figures::{self, Anim, Colour};
use crate::missiles;
use crate::sheet::Sheet;

/// Paint the terrain viewport — `Battlefield_Draw32` (`0x004BCBDC`).
///
/// Tiles are blitted opaque: the original's tile path does not test for index
/// 0, and no frame of `T32_bat1.pl8` contains one. `two` is slot 1 —
/// [`Ground::tileset2`] — which a cell asks for through [`Cell::tileset`];
/// pass `None` for a field battle, where no cell ever does.
///
/// [`Cell::tileset`]: l2_sim::terrain::Cell::tileset
pub fn draw_terrain(
    canvas: &mut Canvas,
    field: &Battlefield,
    tiles: &Sheet,
    two: Option<&Sheet>,
    cam: Camera,
) {
    for row in 0..VIEW_ROWS {
        for col in 0..VIEW_COLS {
            let (mx, my) = (cam.x + col, cam.y + row);
            if mx >= DIM || my >= DIM {
                continue;
            }
            let cell = field.at(mx, my);
            let (x, y) = (ORIGIN_X + col as i32 * TILE, ORIGIN_Y + row as i32 * TILE);
            let sheet = match cell.tileset() {
                0 => Some(tiles),
                _ => two,
            };
            if let Some(frame) = sheet.and_then(|s| s.frame(cell.gfx as usize)) {
                canvas.blit_opaque(&frame, x, y);
            }
            // The second pass of the same cell, always out of slot 1.
            if cell.terrain != 0 && OVERLAY_ELEVATIONS.contains(&cell.elevation) {
                let idx = match cell.terrain {
                    t if t < 0x10 => OVERLAY_BASE + t as usize,
                    _ => OVERLAY_CAP,
                };
                if let Some(frame) = two.and_then(|s| s.frame(idx)) {
                    canvas.blit_clipped(&frame, x, y, FIELD_CLIP);
                }
            }
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
/// the square he
/// so he could turn mid-crossing and be drawn jumping sideways. 324 drawn
/// jumps of half a cell or more over a 42-figure battle; 0 once
/// `BattleRunner::step_one` took `BattleMan_Step`'s order. `docs/battle.md`
/// §13.6, `docs/decisions.md` C200.
/// **The trail is the mover's, not the pose's.** `BattleFigure_Draw` indexes
/// `g_walkOffset32[dirc][walking]` with the figure's own crossing counter; no
/// `Anim_*` handler touches it. Ours read `anim == Walking`, so a man the
/// melee arm re-posed mid-crossing — `BattleMan_StateMelee`'s `Anim_Strike` at
/// the top of the tick that drops him out (`00480000.c:1384`) — snapped 30 px
/// onto the cell he was still entering.
pub fn drawn_cell(f: &Fighter) -> ((i32, i32), u8) {
    let walking = if f.progress.free { 0 } else { f.progress.substep.min(16) as u8 };
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
            // A body the corpse state has counted out is gone — the original
            // frees the record, so `FUN_004BD938` never collects it.
            // [`BattleRunner::corpse_gone`].
            if runner.corpse_gone(i) {
                return None;
            }
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
                if let Some(frame) = horse.frame(figures::horse_frame(f.facing, f.anim, f.phase)) {
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

        let index = match f.troop.is_siege() {
            true => match engines::frame(f.troop, f.anim, f.facing, f.polar, f.phase) {
                Some(i) => i,
                None => continue,
            },
            // **Two facings, and which one is read is part of the pose.**
            // `docs/battle.md` §13.8: `dirc` (`+0x18`) drives the sub-cell
            // offset and the **walk** frame; `dirc2` (`+0x19`), copied to
            // `facingDrawn` (`+0x0D`) at the end of every handler, drives the
            // **strike** frame and is what `Anim_StandA2`'s fidget turns.
            false => {
                let facing = match f.anim {
                    Anim::Walking => f.facing,
                    // `Anim_DyingA2` (`00480000.c:3009`) takes the half-facing
                    // band from `dirc` as well — `(dirc & 6) >> 1` — and only
                    // then copies it into `facingDrawn` (`3017`).
                    Anim::Dying => f.facing,
                    // **A knight stands at `dirc`.** `Anim_StandA2`'s knight
                    // arm (`00480000.c:2896-2900`) overwrites the frame the
                    // fidget just computed with the bare `dirc`, so a standing
                    // knight's body never follows the shuffle — only the men
                    // on foot do. The horse sheet reads `dirc` too.
                    Anim::Idle if f.troop == Troop::Knights => f.facing,
                    _ => f.facing_drawn,
                };
                // The three fields the handlers read besides the phase: the
                // figure's index (`Anim_StandA2`'s pose), its reload counter
                // (`Anim_DrawBowA2`'s `swingTimer`) and its melee role
                // (`Anim_StrikeA2` swings only under `role == 1`).
                figures::frame(f.troop, f.anim, facing, figures::pose_of(runner, i))
            }
        };
        let Some(frame) = sheet.frame(index) else { continue };
        // `BattleFigure_Draw` centres on the cell using the sprite *width* for
        // both axes,
        // sixteen above his cell's corner. Reproduced
        //
        // Two troop types get one more nudge on y and only on y —
        // `engines::body_y_nudge`, the painter's `troopType == 9` and
        // `== 10` arms.
        let w = frame.width as i32;
        canvas.blit_clipped(
            &frame,
            cell_x + (TILE / 2 - w / 2),
            cell_y - w / 2 + 8 + engines::body_y_nudge(f.troop),
            FIELD_CLIP,
        );
        drawn += 1;

        // **The catapult's arm** — `FUN_004BE7BE`, a second sprite from
        // `Catarm1/2.pl8` over the carriage, from the *unnudged* cell corner
        // (the painter restores `g_drawX`/`g_drawY` before it calls this).
        if f.troop == Troop::Catapults {
            let swing = runner.sim.figures[f.sim].reload_counter;
            let arm = engines::arm_frame(f.facing, swing);
            if let Some(sheet) = assets.catarm[engines::arm_sheet(f.facing)].as_ref() {
                if let Some(frame) = sheet.frame(arm) {
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

        // **The ram's beam** — `FUN_004BEAB9`, two more `Engine.pl8` frames
        // above and below the carriage, and only while it is beating on a
        // gate. Neither carries the `+ 8` the body and the arm do.
        if f.troop == Troop::BatteringRams {
            if let Some(strips) = engines::ram_strips(f.anim, f.phase) {
                for (index, dy) in [strips.0, strips.1] {
                    let Some(frame) = sheet.frame(index) else { continue };
                    let w = frame.width as i32;
                    canvas.blit_clipped(
                        &frame,
                        cell_x + (TILE / 2 - w / 2),
                        cell_y - w / 2 + dy,
                        FIELD_CLIP,
                    );
                }
            }
        }
    }
    drawn
}

pub fn banner_frame(shield: u8, phase: u8) -> usize {
    BANNER_BASE + shield as usize * BANNER_PHASES as usize + (phase % BANNER_PHASES) as usize
}

/// **Which cell flies it.** `Battlefield_BuildCastle`'s structure code 6 — the
/// keep — is the one arm that sets cell byte `+2` bit `0x80`, and the frame
/// byte that carries code 6 in both structure tables is **0**. The overlap pass
/// branches on exactly that: `gfx ? FUN_004bd759(gfx) : FUN_004bd574()`. So the
/// keep cell, and only the keep cell, takes the banner. **[V]** — `code::KEEP`
/// in `l2_sim::siege`, and `frames_with_code(level, KEEP) == [0]` at every
/// level.
fn banner_cell(cell: l2_sim::terrain::Cell) -> bool {
    cell.flags2 & 0x80 != 0 && cell.gfx == 0
}

/// **The pass after the men** — `FUN_004BD355` (`0x004BD355`), which walks the
/// viewport plus a one-cell ring and, per cell, draws what overlaps the
/// figures and then that cell's missiles.
///
/// ```c
/// FUN_004bd938(); FUN_004bda92(); FUN_004bdb95();      /* collect, sort, draw */
/// for (row …) for (col …) {
///     if (cell[+2] & 0x80) { gfx = cell[+3]; gfx ? FUN_004bd759(gfx) : FUN_004bd574(); }
///     if (cell[+6]) FUN_004beed4(cell[+6]);            /* the missile list    */
/// }
/// ```
///
/// **`docs/battle.md` §13.7 named the wrong byte for the overlap pass.** It
/// reads *"a second terrain pass for cells flagged `0x04` on byte `+1`"*; the
/// test here is byte **`+2`** bit **`0x80`**, and the thing drawn is
/// terrain tile — it is `Engine.pl8` (a docked tower's stair,
/// [`engines::dock_overlay_frame`]) or `A2_miss.pl8` (`FUN_004BD574`'s animated
/// banner — [`banner_frame`]).
///
/// `banner` is the garrison's `(shield, phase)`: `g_units[g_battleArmyB]+0x02`
/// and `DAT_004E5B18`. `None` for a field battle, which has no keep cell.
///
/// Returns how many missiles were drawn.
pub fn draw_overlay_and_missiles(
    canvas: &mut Canvas,
    runner: &BattleRunner,
    assets: &BattleAssets,
    cam: Camera,
    banner: Option<(u8, u8)>,
) -> usize {
    // Every live missile, filed by the cell it stands on, in ascending slot
    // order — `Missile_LinkToCell` (`0x0046EFBE`) appends to the tail and
    // `Missile_UpdateAll` relinks slots 1 … 100 ascending every tick, so the
    // list a cell hands the renderer is in slot order.
    let mut by_cell: std::collections::BTreeMap<(i16, i16), Vec<usize>> = Default::default();
    for (slot, m) in runner.missiles.iter() {
        by_cell.entry((m.cell_y, m.cell_x)).or_default().push(slot);
    }

    let (camx, camy) = (cam.x as i32, cam.y as i32);
    let mut drawn = 0;
    let mut nth_fire = 0usize;
    for row in -1..=VIEW_ROWS as i32 {
        for col in -1..=VIEW_COLS as i32 {
            let (mx, my) = (camx + col, camy + row);
            if mx < 0 || my < 0 || mx >= DIM as i32 || my >= DIM as i32 {
                continue;
            }
            let (px, py) = (ORIGIN_X + col * TILE, ORIGIN_Y + row * TILE);

            dock_overlay(canvas, runner, assets, mx as usize, my as usize, px, py);

            // The overlap pass's other arm — `gfx == 0` picks the banner.
            if let Some((shield, phase)) = banner {
                if banner_cell(runner.field.at(mx as usize, my as usize)) {
                    if let Some(sheet) = assets.missiles.as_ref() {
                        if let Some(frame) = sheet.frame(banner_frame(shield, phase)) {
                            // `g_drawX += 0x10 - (w >> 1); g_drawY += 0x10 - (w >> 1);`
                            // — the *width* on both axes, as `FUN_004BD759`
                            // beside it also does. Reproduced, not corrected.
                            let w = frame.width as i32;
                            canvas.blit_clipped(&frame, px + (TILE / 2 - w / 2), py + (TILE / 2 - w / 2), FIELD_CLIP);
                        }
                    }
                }
            }

            let Some(slots) = by_cell.get(&(my as i16, mx as i16)) else { continue };
            for &slot in slots.iter().take(missiles::CELL_LIST_LIMIT) {
                let m = runner.missiles.get(slot);
                let Some(index) = missiles::frame(m) else { continue };
                let Some(sheet) = assets.missiles.as_ref() else { continue };
                let Some(frame) = sheet.frame(index) else { continue };
                // A missile's position is already in pixels: thirty-seconds
                // of a cell, and a cell is 32 pixels. `DAT_004E5D44` is
                // `tileSize / 2` and is the whole of the centring — no
                // sprite-width term, unlike every figure on the field.
                let mut x = ORIGIN_X + (m.x as i32 - camx * TILE) + TILE / 2;
                let mut y = ORIGIN_Y + (m.y as i32 - camy * TILE) + TILE / 2;
                if m.class == l2_sim::missile::CLASS_FIRE {
                    let (jx, jy) = missiles::jitter(runner.tick, nth_fire);
                    nth_fire += 1;
                    x += jx;
                    y += jy;
                }
                canvas.blit_clipped(&frame, x, y, FIELD_CLIP);
                drawn += 1;
            }
        }
    }
    drawn
}

/// **A docked siege tower's stair** — `FUN_004BD759` (`0x004BD759`), one of
/// `Engine.pl8` frames `0x1F … 0x22` over the centre cell of the 3 × 3
/// `FUN_00491492` leaves behind, drawn after the men so the tower's top
/// overlaps them. Centred `(0x10 - w / 2)` on **both** axes.
///
/// The original gates on cell byte `+2` bit `0x80`; our cells have no byte
/// `+2`, so this gates on `flags & 1`, which `l2_sim::siege::lay_tower_ramp`
/// sets on those nine cells and nothing else in `l2_sim` sets anywhere. The
/// gate is not optional: `Engine.pl8`'s four codes are 73, 76, 97 and 100, and
/// the field tileset's hills occupy 64 … 111.
fn dock_overlay(
    canvas: &mut Canvas,
    runner: &BattleRunner,
    assets: &BattleAssets,
    mx: usize,
    my: usize,
    px: i32,
    py: i32,
) {
    let cell = runner.field.at(mx, my);
    if cell.flags & 1 == 0 {
        return;
    }
    let Some(index) = engines::dock_overlay_frame(cell.gfx) else { return };
    let Some(sheet) = assets.engine.as_ref() else { return };
    let Some(frame) = sheet.frame(index) else { return };
    let w = frame.width as i32;
    canvas.blit_clipped(&frame, px + (TILE / 2 - w / 2), py + (TILE / 2 - w / 2), FIELD_CLIP);
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
/// **Two things the original does here
/// bits of byte `+2`, which our cells do not carry:
///
/// * the per-cell dirty bits, `flags & 3` — we repaint every cell of the rows
///   we visit, which is what `g_mapRedraw` makes the original do anyway;
/// * the erase tile, `t2_spri` frame 0, drawn over a cell whose `flags & 2` is
///   set and whose occupant has gone — one pass later that cell draws its
///   terrain again, and repainting the whole row goes straight there.
///
/// The third, `flags & 0x1C == 4`, **is** honoured now: it is
/// [`Cell::tileset`](l2_sim::terrain::Cell::tileset), and a siege's ground,
/// moat and rubble come from the second sheet here
/// pixels.
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
            let cell = field.at(x, y);
            let frame = if colour == 0 {
                match cell.tileset() {
                    0 => sheets.tiles.frame(cell.gfx as usize),
                    _ => sheets.tiles2.as_ref().and_then(|s| s.frame(cell.gfx as usize)),
                }
            } else {
                sheets.men.frame(colour as usize)
            };
            if let Some(frame) = frame {
                raster.blit_opaque(&frame, px, py);
            }
        }
    }
}

/// One whole frame, in the original's order: the terrain
/// (`FUN_004BCBDC`), the men sorted by map y (`FUN_004BD938` →
/// `FUN_004BDA92` → `FUN_004BDB95`), then the per-cell pass that puts what
/// overlaps them and the missiles on top (`FUN_004BD355`).
///
/// The terrain's two sheets come from the [`Ground`] — C201 — and the third
/// pass from C202; the passes are independent,
/// take both.
///
/// Returns the number of figures drawn, which is what the headless tests
/// assert on; [`draw_overlay_and_missiles`] returns the missiles.
pub fn draw(
    canvas: &mut Canvas,
    runner: &BattleRunner,
    assets: &BattleAssets,
    ground: Ground,
    cam: Camera,
    banner: Option<(u8, u8)>,
) -> usize {
    let art = assets.ground(ground);
    draw_terrain(canvas, &runner.field, &art.tiles, art.tiles2.as_ref(), cam);
    let figures = draw_figures(canvas, runner, assets, cam);
    draw_overlay_and_missiles(canvas, runner, assets, cam, banner);
    figures
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

