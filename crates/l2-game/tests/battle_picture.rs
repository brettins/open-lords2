//! **The battlefield, looked at** — three reports from one player on build
//! `73DF34969`, each driven through the screen stack and asserted in pixels.
//!
//! ```text
//! cargo test -p l2-game --test battle_picture
//! LORDS2_DIR="F:\games\Lords of the Realm II" cargo test -p l2-game --test battle_picture
//! ```
//!
//! > *"Just got into a battle, it's all reverse color ...or..something. It's
//! > blue grainy madness. The unit seems to like...reset on their square once as
//! > they move. And there's some ghosting of them."*
//!
//! 1. **The colours.** `Res_LoadStatic` (`0x00499859`) preloads record 2 of
//!    `g_preloadTable` (`0x004D9F48`), `t32_bat1.256`, into `0x00568EE0`, and
//!    `Screen_DrawBattlefield` (`0x004233F7`) ends a field battle's repaint with
//!    `Palette_Set(0x568EE0)`. Ours named that file and never loaded it, so the
//!    presenter fell back to `base01.256`.
//! 2. **The snap.** `BattleMan_Step` (`0x0048F1DD`) enters the next cell
//!    *first* — `FUN_00491B1F` rewrites `mapX`/`mapY` — and then counts `walking`
//!    (`+0x32`) up 1, 3 … 15 while `BattleFigure_Draw` (`0x004BDC31`) trails the
//!    man behind the cell he is already in. Our simulation counts first and
//!    enters last, and the picture applied the trailing offset to the cell he
//!    was *leaving*: up to 28 pixels behind his own square, then a jump.
//! 3. **The ghosts.** `BattleFigure_Draw` clips every man to
//!    `Clip_Horizontal(0, 480)` / `Clip_Vertical(24, 472)`, the viewport
//!    `FUN_004BC020` stores. Ours blitted unclipped, into the menu bar, the
//!    right column and the bottom strip, which nothing on this screen repaints.
//!
//! **The probes are literals out of the binary** — the viewport clip, the
//! palette file — and never an expression of the constants under test.
//! `docs/agents.md`, *compute the probe from the constant you are ablating*.
//!
//! And the fourth test is the one the brief insists on: **drawing does not
//! change the battle**, tick for tick and by the saved game's bytes.

use l2_formats::{DecodedFrame, Palette};
use l2_game::battlefield::{self as bf, LiveBattle};
use l2_game::game::Assets;
use l2_game::input::Event;
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::Game;
use l2_sim::runner::{Army, BattleRunner};
use l2_sim::terrain::DIM;
use l2_sim::{Motion, Troop, SIDE_A, SIDE_B};
use l2_view::sheet::Sheet;
use l2_view::Canvas;

// ---------------------------------------------------------------- the literals

/// `FUN_004BC020(…, 0x50, 0x50, 8, 0, 0x18, 0xF, 0xE, 0x20)` stores
/// `DAT_004E6564 = 0`, `DAT_004E5D54 = 15 × 32 + 0`, `DAT_004E5D48 = 0x18` and
/// `DAT_004E5D6C = 14 × 32 + 0x18`: the clip every battle sprite is drawn
/// through. Written out, not read from `l2_view::scene`.
const FIELD_X0: i32 = 0;
const FIELD_Y0: i32 = 24;
const FIELD_X1: i32 = 480;
const FIELD_Y1: i32 = 472;

// ------------------------------------------------------------------ the world

/// Open ground with the human's marker at (40, 36) and the AI's at (40, `ai_row`).
fn field(ai_row: usize) -> l2_sim::Battlefield {
    let mut layer = vec![0u8; l2_sim::terrain::CELLS];
    layer[36 * DIM + 40] = 0x04;
    layer[ai_row * DIM + 40] = 0x0F;
    l2_sim::terrain::build(&layer, 1)
}

/// A live battle, unpaused, between a human (realm 1, side 0) and an AI (realm
/// 2, side 4), staged on the battlefield screen. The camera is placed by
/// `cam_of` from where the human's first figure was deployed.
fn staged(
    ai_row: usize,
    human: &[(Troop, u16)],
    ai: &[(Troop, u16)],
    cam_of: impl Fn((u8, u8)) -> (i32, i32),
) -> (Game, Machine) {
    let runner = BattleRunner::deploy_armies(
        field(ai_row),
        0x5EED,
        Army { troops: ai, owner: 2, human: false },
        Army { troops: human, owner: 1, human: true },
    );
    let first = runner.fighters.iter().find(|f| f.side == SIDE_A).expect("a human figure");
    let cam = cam_of((first.x, first.y));
    let mut live = LiveBattle::new(runner, 0, 0, 0, None, 1, 1);
    live.paused = false;
    live.cam = cam;
    let mut g = Game::new(5);
    g.prefs.tip_screens = false;
    g.prefs.tool_tips = false;
    g.player = 1;
    g.battle = Some(Box::new(live));
    (g, Machine::new(ScreenId::Battlefield))
}

fn send(m: &mut Machine, g: &mut Game, a: &Assets, e: Event) {
    let mut ctx = Ctx { game: g, assets: a };
    m.handle(e, &mut ctx);
}

/// One frame as `App` runs it: the tick, then the paint, **onto the same
/// canvas as every frame before** — `Machine::draw` never clears it, so a
/// remnant is only visible to a test that keeps the canvas.
fn frame(m: &mut Machine, g: &mut Game, a: &Assets, canvas: &mut Canvas) {
    let mut ctx = Ctx { game: g, assets: a };
    m.update(&mut ctx);
    m.draw(&ctx, canvas);
}

fn paint(m: &mut Machine, g: &mut Game, a: &Assets, canvas: &mut Canvas) {
    let ctx = Ctx { game: g, assets: a };
    m.draw(&ctx, canvas);
}

fn live(g: &Game) -> &LiveBattle {
    g.battle.as_deref().expect("a live battle")
}

/// The centre of a cell on screen.
fn pixel(live: &LiveBattle, cell: (u8, u8)) -> (i32, i32) {
    let (cx, cy) = (cell.0 as i32 - live.cam.0, cell.1 as i32 - live.cam.1);
    assert!(
        (0..bf::VIEW_COLS).contains(&cx) && (0..bf::VIEW_ROWS).contains(&cy),
        "cell {cell:?} is off screen with the camera at {:?}",
        live.cam
    );
    (bf::VIEW.x + cx * bf::TILE + bf::TILE / 2, bf::VIEW.y + cy * bf::TILE + bf::TILE / 2)
}

fn click_at(m: &mut Machine, g: &mut Game, a: &Assets, (x, y): (i32, i32)) {
    send(m, g, a, Event::Pointer { x, y });
    send(m, g, a, Event::Click { x, y });
    send(m, g, a, Event::Release { x, y });
}

/// Pick the human's figure `man` by clicking on him, then order him to `to`
/// through **the overview panel** — `BattleMap_Click`, at two pixels a cell —
/// which reaches a cell whether or not it is in view. The pointer is then
/// parked mid-field, clear of every edge, so the camera never scrolls.
fn send_man(m: &mut Machine, g: &mut Game, a: &Assets, man: usize, to: (u8, u8)) {
    let f = &live(g).runner.fighters[man];
    let at = pixel(live(g), (f.x, f.y));
    click_at(m, g, a, at);
    assert_eq!(live(g).runner.selected_fighters(1), vec![man], "the click did not pick him");
    let (ox, oy) = (bf::OVERVIEW.x + 2 * to.0 as i32, bf::OVERVIEW.y + 2 * to.1 as i32);
    click_at(m, g, a, (ox, oy));
    // A right release on the field drops the selection (`FUN_0043C55C`) and
    // keeps the order, so no selection marker is painted over the man.
    send(m, g, a, Event::Pointer { x: 200, y: 200 });
    send(m, g, a, Event::RightClick { x: 200, y: 200 });
    assert!(live(g).runner.selected_fighters(1).is_empty(), "the right click kept him picked");
}

fn human_figure(g: &Game) -> usize {
    live(g).runner.fighters.iter().position(|f| f.side == SIDE_A).expect("a human figure")
}

fn install() -> Option<(Assets, l2_mods::Platform)> {
    let dir = l2_testkit::install_dir()?;
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let assets = Assets::load(&platform.vfs).expect("assets load");
    Some((assets, platform))
}

// ----------------------------------------------------------------- the colours

/// **Every pixel of the field is shown in `t32_bat1.256`'s colours** — which
/// `Screen_DrawBattlefield` (`0x004233F7`) sets with `Palette_Set(0x568EE0)`,
/// the buffer `Res_LoadStatic` (`0x00499859`) fills from record 2 of
/// `g_preloadTable`. The expected colour of each pixel is read out of the
/// install's own `.256`, independently of every table in this tree; the
/// applied colour is the shell palette the name the stack gives resolves to —
/// the map the presenter reads, and **the only one**: a name it does not hold
/// falls through to `base01.256`, which is what the player saw.
///
/// It stops at the resolved palette rather than at presented bytes because the
/// presenter is being moved into the library by the concurrent overlay-palette
/// branch, as `Machine::present`; once that has landed, the loop below belongs
/// on `m.present(…)`'s bytes, and the answer must not change.
///
/// The discrimination half is what keeps it from passing on a palette that
/// merely agrees: the same pixels through `base01.256` must differ, and on the
/// shipped files they differ at most of them.
///
/// Ablation: delete `"T32_bat1.256"` from `shell::PALETTES` — red, on the
/// lookup. Run with `main.rs`'s presenter line as the probe instead, before it
/// was moved, the same ablation showed the first field pixel, index 77, as
/// `(0, 97, 190)` where `T32_bat1.256` has `(64, 85, 12)`.
#[test]
fn the_battlefield_is_shown_in_the_palette_screen_drawbattlefield_sets() {
    let Some((assets, platform)) = install() else {
        l2_testkit::skip!("no game install, so no T32_bat1.256 to compare against");
    };
    let bat1 = Palette::from_bytes(&platform.vfs.read("T32_bat1.256").expect("T32_bat1.256"))
        .expect("768 bytes");
    let base01 = Palette::from_bytes(&platform.vfs.read("base01.256").expect("base01.256"))
        .expect("768 bytes");

    let (mut g, mut m) =
        staged(74, &[(Troop::Swordsmen, 3), (Troop::Archers, 3)], &[(Troop::Peasants, 1)], |(x, y)| {
            (x as i32 - 7, y as i32 - 6)
        });
    let mut canvas = Canvas::screen();
    paint(&mut m, &mut g, &assets, &mut canvas);
    assert_eq!(m.top_id(), Some(ScreenId::Battlefield));

    let name = m.palette_name().expect("the battlefield names a palette of its own");
    let applied = assets.shell.palette(name).unwrap_or_else(|| {
        panic!(
            "{name}, which the battlefield names, is in no palette the presenter reads — every \
             battle frame would be shown through base01.256"
        )
    });
    let (mut seen, mut differs) = (0usize, 0usize);
    let (mut sum_applied, mut sum_base) = ([0u64; 3], [0u64; 3]);
    for y in FIELD_Y0..FIELD_Y1 {
        for x in FIELD_X0..FIELD_X1 {
            let idx = canvas.at(x as usize, y as usize);
            assert_eq!(
                applied.rgb(idx),
                bat1.rgb(idx),
                "the field pixel at ({x}, {y}), index {idx}, is not shown in T32_bat1.256's colour"
            );
            seen += 1;
            if base01.rgb(idx) != bat1.rgb(idx) {
                differs += 1;
            }
            for c in 0..3 {
                sum_applied[c] += applied.rgb(idx)[c] as u64;
                sum_base[c] += base01.rgb(idx)[c] as u64;
            }
        }
    }
    let mean = |s: [u64; 3]| [s[0] / seen as u64, s[1] / seen as u64, s[2] / seen as u64];
    eprintln!(
        "field mean RGB: through T32_bat1.256 {:?}, through base01.256 {:?}; {differs} of {seen} pixels differ",
        mean(sum_applied),
        mean(sum_base)
    );
    assert!(differs * 2 > seen, "base01.256 agrees with T32_bat1.256 on the field: {differs} of {seen}");
}

// ------------------------------------------------------------------- the snap

/// The opaque pixels of a frame, relative to its top-left corner.
fn opaque(frame: &DecodedFrame) -> Vec<(i32, i32, u8)> {
    let w = frame.width as usize;
    (0..frame.indices.len())
        .filter(|&i| frame.opaque[i])
        .map(|i| ((i % w) as i32, (i / w) as i32, frame.indices[i]))
        .collect()
}

/// Every top-left corner in `xs × ys` at which `frame` sits on the canvas
/// exactly: every opaque pixel that lands inside the field equals the canvas,
/// and at least half of them land inside it.
fn locate(canvas: &Canvas, frame: &DecodedFrame, xs: std::ops::Range<i32>, ys: std::ops::Range<i32>) -> Vec<(i32, i32)> {
    let pts = opaque(frame);
    let mut hits = Vec::new();
    for y in ys {
        for x in xs.clone() {
            let mut compared = 0;
            let ok = pts.iter().all(|&(dx, dy, idx)| {
                let (px, py) = (x + dx, y + dy);
                if px < FIELD_X0 || px >= FIELD_X1 || py < FIELD_Y0 || py >= FIELD_Y1 {
                    return true;
                }
                compared += 1;
                canvas.at(px as usize, py as usize) == idx
            });
            if ok && compared * 2 >= pts.len() && compared > 0 {
                hits.push((x, y));
            }
        }
    }
    hits
}

/// **A walking man is drawn further along every tick and never back on the
/// square he left** — found in the pixels, by locating the exact frame he is
/// showing on the canvas the screen painted.
///
/// `BattleMan_Step` (`0x0048F1DD`) on a free cell: `dirc = dir; walking = 1;
/// FUN_00491B1F(man)` — and `FUN_00491B1F` is the move, `mapX += 1` for facing 2.
/// Then `walking += 2` a sub-step until it passes 16, and `BattleFigure_Draw`
/// (`0x004BDC31`) adds `g_walkOffset32[dirc][walking]` to the cell he is **in**.
/// So a man crossing a cell is drawn 30, 26, … 2 pixels short of it and then on
/// it: forward, four pixels a sub-step, and never backward.
///
/// He walks east five cells, so the man's sprite centre must end exactly 160
/// pixels right of where it began, moving monotonically on the way.
///
/// Ablation: draw `walk_offset(facing, substep)` from `(f.x, f.y)` again in
/// `l2_view::scene::figure_origin` — red on the first sub-step, 28 pixels
/// backward, which is the player's *"reset on their square"*.
#[test]
fn a_walking_man_is_drawn_advancing_every_tick_and_never_back_on_his_old_square() {
    let Some((assets, platform)) = install() else {
        l2_testkit::skip!("no game install, so no sprite sheet to find the man with");
    };
    // Side 0's bank is the blue one — `Assets::load(…, Red, Blue)`.
    let sheet = Sheet::new(platform.vfs.read("A2b_mace.pl8").expect("A2b_mace.pl8")).expect("a sheet");

    let (mut g, mut m) =
        staged(74, &[(Troop::Macemen, 1)], &[(Troop::Peasants, 1)], |(x, y)| (x as i32 - 4, y as i32 - 6));
    let man = human_figure(&g);
    let start = (live(&g).runner.fighters[man].x, live(&g).runner.fighters[man].y);
    let cam = live(&g).cam;
    send_man(&mut m, &mut g, &assets, man, (start.0 + 5, start.1));

    let row_y = FIELD_Y0 + (start.1 as i32 - cam.1) * 32;
    let mut canvas = Canvas::screen();
    let mut centres: Vec<(u32, i32, Motion)> = Vec::new();
    // Macemen cross a cell in 18 ticks; five cells and a margin.
    for t in 0..(5 * 18 + 40) {
        frame(&mut m, &mut g, &assets, &mut canvas);
        let f = &live(&g).runner.fighters[man];
        let index = l2_view::figures::frame(f.troop, f.anim, f.facing, f.phase);
        let pic = sheet.frame(index).expect("the frame the man is showing");
        let hits = locate(&canvas, &pic, -48..FIELD_X1, row_y - 48..row_y + 16);
        assert_eq!(hits.len(), 1, "tick {t}: the man's frame {index} was found at {hits:?}");
        // The sprite is placed at `origin + 16 − w/2`, so `x + w/2` is the
        // origin plus a constant whatever the pose's width.
        centres.push((t, hits[0].0 + pic.width as i32 / 2, f.anim));
    }

    let first = centres[0].1;
    for w in centres.windows(2) {
        let ((t0, x0, _), (t1, x1, a1)) = (w[0], w[1]);
        assert!(
            x1 >= x0,
            "tick {t0} → {t1}: the man was drawn {} pixels backward ({x0} → {x1}, {a1:?}) — \
             every centre: {:?}",
            x0 - x1,
            centres.iter().map(|c| c.1).collect::<Vec<_>>()
        );
    }
    let last = centres.last().unwrap().1;
    assert_eq!(last - first, 5 * 32, "he did not end five cells east of where he began");
    let steps = centres.windows(2).filter(|w| w[1].1 > w[0].1).count();
    assert!(steps >= 5 * 8, "only {steps} forward steps in five cells: he is jumping, not walking");
    assert_eq!(
        (live(&g).runner.fighters[man].x, live(&g).runner.fighters[man].y),
        (start.0 + 5, start.1),
        "the simulation did not put him where he was sent"
    );
}

// ------------------------------------------------------------------ the ghosts

/// The pixels no painter on this screen writes: the menu bar's 24 rows, the
/// strip below the field, and the column between the field and the banners.
fn untouched_regions() -> Vec<(i32, i32, i32, i32)> {
    vec![
        (0, 0, 640, FIELD_Y0),
        (FIELD_X0, FIELD_Y1, FIELD_X1, 480),
        // `DAT_004D31F4`'s banners begin at x 484 at the earliest, and the
        // overview's fill ends at y 184; the buttons start at 448.
        (FIELD_X1, 184, 484, 448),
    ]
}

fn snapshot(canvas: &Canvas) -> Vec<u8> {
    let mut out = Vec::new();
    for (x0, y0, x1, y1) in untouched_regions() {
        for y in y0..y1 {
            for x in x0..x1 {
                out.push(canvas.at(x as usize, y as usize));
            }
        }
    }
    out
}

/// Walk one man off the field in direction `(dx, dy)` from a camera that puts
/// him at view cell `at`, and hold every frame to two claims.
fn walk_off(assets: &Assets, at: (i32, i32), (dx, dy): (i32, i32)) {
    let (mut g, mut m) = staged(74, &[(Troop::Macemen, 1)], &[(Troop::Peasants, 1)], |(x, y)| {
        (x as i32 - at.0, y as i32 - at.1)
    });
    let man = human_figure(&g);
    let start = (live(&g).runner.fighters[man].x, live(&g).runner.fighters[man].y);
    let to = ((start.0 as i32 + 4 * dx) as u8, (start.1 as i32 + 4 * dy) as u8);
    send_man(&mut m, &mut g, assets, man, to);

    let mut canvas = Canvas::screen();
    paint(&mut m, &mut g, assets, &mut canvas);
    let before = snapshot(&canvas);
    let art = assets.battle.as_ref().expect("battle artwork");
    for t in 0..(4 * 18 + 30) {
        frame(&mut m, &mut g, assets, &mut canvas);
        let now = snapshot(&canvas);
        let changed = before.iter().zip(&now).filter(|(a, b)| a != b).count();
        assert_eq!(
            changed, 0,
            "tick {t}, walking ({dx}, {dy}): {changed} pixels outside the field were written, and \
             nothing on this screen will ever repaint them"
        );

        // **The square he left is the ground again.** Once he is two cells
        // away, the start cell's 32 × 32 on the kept canvas equals a fresh
        // terrain pass — the full repaint `scene::draw_terrain` does every
        // frame, which is our `Battlefield_Draw32` dirty-cell restore.
        let f = &live(&g).runner.fighters[man];
        let away = (f.x as i32 - start.0 as i32).abs().max((f.y as i32 - start.1 as i32).abs());
        if away >= 2 {
            let cam = l2_view::scene::Camera::clamped(live(&g).cam.0, live(&g).cam.1);
            let mut ground = Canvas::screen();
            l2_view::scene::draw_terrain(&mut ground, &live(&g).runner.field, &art.tiles, cam);
            let (sx, sy) = (start.0 as i32 - cam.x as i32, start.1 as i32 - cam.y as i32);
            for y in 0..32 {
                for x in 0..32 {
                    let (px, py) = ((sx * 32 + x) as usize, (24 + sy * 32 + y) as usize);
                    assert_eq!(
                        canvas.at(px, py),
                        ground.at(px, py),
                        "tick {t}: a remnant at ({px}, {py}) on the square he left"
                    );
                }
            }
        }
    }
    let f = &live(&g).runner.fighters[man];
    assert_eq!((f.x, f.y), to, "he did not walk off the field");
}

/// **A man who walks off the edge of the field leaves nothing behind him** —
/// not on the square he left, and not in the menu bar, the right column or the
/// strip under the field, which is where the ghosts were: nothing on this
/// screen repaints those pixels, so a man drawn into them stayed there.
///
/// `BattleFigure_Draw` (`0x004BDC31`) and the horse under a knight
/// (`FUN_004BE4DF`) both call `Clip_Horizontal(DAT_004E6564, DAT_004E5D54)` and
/// `Clip_Vertical(DAT_004E5D48, DAT_004E5D6C)` before the blit, and
/// `FUN_004BC020` has set those to 0, 480, 24 and 472. Three walks: north-east
/// across the top-right corner, south under the bottom, and east along a row.
///
/// Ablation: `blit` in place of `blit_clipped` for the man in
/// `l2_view::scene::draw_figures` — red on the first walk, in the menu bar.
/// **The square-he-left half is a green ablation, and it is a finding**: our
/// terrain pass repaints every cell every frame, so there is no dirty-cell
/// restore to forget; skipping `draw_terrain` on every frame after the first
/// turns it red.
#[test]
fn a_man_walking_off_the_field_leaves_nothing_on_the_screen_behind_him() {
    let Some((assets, _platform)) = install() else {
        l2_testkit::skip!("no game install, so no sprites to spill");
    };
    walk_off(&assets, (13, 1), (1, -1));
    walk_off(&assets, (6, 12), (0, 1));
    walk_off(&assets, (13, 6), (1, 0));
}

// -------------------------------------------------------------- determinism

/// FNV-1a, 64-bit — a fingerprint of the saved bytes to print, so the same
/// battle can be compared across two builds by eye.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

fn cells_of(live: &LiveBattle, side: u8) -> Vec<(u8, u8)> {
    live.runner
        .fighters
        .iter()
        .enumerate()
        .filter(|(i, f)| f.side == side && live.runner.is_alive(*i))
        .map(|(_, f)| (f.x, f.y))
        .collect()
}

/// One battle, twice from the same events: one copy painted after every tick,
/// the other never painted. Returns both.
fn played(assets: &Assets, ticks: u32) -> (Game, Game) {
    let human = [(Troop::Swordsmen, 3), (Troop::Archers, 3)];
    let ai = [(Troop::Crossbowmen, 3), (Troop::Macemen, 3)];
    let cam = |_: (u8, u8)| (33, 33);
    let (mut drawn, mut dm) = staged(44, &human, &ai, cam);
    let (mut blind, mut bm) = staged(44, &human, &ai, cam);

    for (g, m) in [(&mut drawn, &mut dm), (&mut blind, &mut bm)] {
        // A box round the whole human army, then a click on an enemy.
        let cells = cells_of(live(g), SIDE_A);
        let lo = (cells.iter().map(|c| c.0).min().unwrap(), cells.iter().map(|c| c.1).min().unwrap());
        let hi = (cells.iter().map(|c| c.0).max().unwrap(), cells.iter().map(|c| c.1).max().unwrap());
        let (lx, ly) = pixel(live(g), lo);
        let (hx, hy) = pixel(live(g), hi);
        let from = (lx - bf::TILE / 2 + 2, ly - bf::TILE / 2 + 2);
        let to = (hx + bf::TILE / 2 - 2, hy + bf::TILE / 2 - 2);
        send(m, g, assets, Event::Pointer { x: from.0, y: from.1 });
        send(m, g, assets, Event::Click { x: from.0, y: from.1 });
        send(m, g, assets, Event::Pointer { x: to.0, y: to.1 });
        send(m, g, assets, Event::Release { x: to.0, y: to.1 });
        let enemy = *cells_of(live(g), SIDE_B).first().unwrap();
        click_at(m, g, assets, pixel(live(g), enemy));
        send(m, g, assets, Event::Pointer { x: 240, y: 240 });
    }
    assert!(live(&drawn).runner.selected_count(1) > 0, "the box picked nobody");
    assert_eq!(drawn.battle, blind.battle, "the same events left the two games different");

    let mut canvas = Canvas::screen();
    for t in 0..ticks {
        frame(&mut dm, &mut drawn, assets, &mut canvas);
        let mut ctx = Ctx { game: &mut blind, assets };
        bm.update(&mut ctx);
        assert!(drawn.battle == blind.battle, "painting changed the battle at tick {t}");
    }
    (drawn, blind)
}

/// Check the two plays agree to the byte, that the battle did something, and
/// print the fingerprint.
fn same_battle(drawn: &Game, blind: &Game, label: &str) {
    let (a, b) = (l2_game::save::encode(drawn), l2_game::save::encode(blind));
    assert_eq!(a, b, "the saved bytes differ between the painted and the unpainted battle");
    eprintln!("{label}: saved battle {} bytes, fnv1a {:016x}", a.len(), fnv1a(&a));
    let runner = &live(drawn).runner;
    let killed: u32 =
        l2_sim::ALL_TROOPS.iter().map(|&t| runner.sim.cues.melee_casualties(t)).sum();
    assert!(killed > 0, "nobody fought, so the comparison is about a battle with nothing in it");
}

/// **Painting does not change the battle** — with the placeholder artwork, so
/// it runs everywhere.
///
/// Compared by the whole `LiveBattle` at every tick and by `save::encode`'s
/// bytes at the end, which carry the lockstep state and more. The fingerprint
/// it prints was taken before the three fixes in this file and after them, and
/// was the same (see the commit that added it).
///
/// **A green ablation, and that is the finding**, as it was for sound in C166:
/// `Screen::draw` receives `&Ctx`, whose `game` it can only read, so there is no
/// line in any painter whose deletion turns this red. Ablated the other way —
/// an extra `runner.step()` inside `BattlefieldScreen::draw`'s caller on one
/// copy — it goes red on that tick. It is the tripwire for the day a painter is
/// handed something it can write.
#[test]
fn painting_the_battlefield_does_not_change_the_battle() {
    let assets = Assets::placeholder();
    let (drawn, blind) = played(&assets, 3_000);
    same_battle(&drawn, &blind, "placeholder");
}

/// The same with the install's artwork, so the painter that runs is
/// `l2_view::scene::draw` — the one this file changed — rather than the
/// placeholder's blocks.
#[test]
fn painting_the_battlefield_with_its_artwork_does_not_change_the_battle() {
    let Some((assets, _platform)) = install() else {
        l2_testkit::skip!("no game install, so the placeholder test above is the whole check");
    };
    let (drawn, blind) = played(&assets, 1_500);
    same_battle(&drawn, &blind, "install");
}
