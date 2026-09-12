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

/// A siege on the screen, its camera on the keep — for the palette below.
fn staged_siege(level: u8) -> (Game, Machine) {
    let runner = BattleRunner::deploy_siege(
        l2_sim::siege::our_castle(level),
        0x5EED,
        l2_sim::runner::Muster { troops: &[(Troop::Swordsmen, 400)], owner: 2, human: false },
        l2_sim::runner::Muster { troops: &[(Troop::Archers, 200)], owner: 1, human: true },
        level,
    );
    let first = runner.fighters.iter().find(|f| f.side == SIDE_A).expect("a garrison figure");
    let cam = (first.x as i32 - 7, first.y as i32 - 6);
    let mut live = LiveBattle::new(runner, 0, 0, 0, Some(level), 1, 1);
    live.paused = false;
    live.cam = cam;
    let mut g = Game::new(5);
    g.prefs.tip_screens = false;
    g.prefs.tool_tips = false;
    g.player = 1;
    g.battle = Some(Box::new(live));
    (g, Machine::new(ScreenId::Battlefield))
}

/// **The siege's own palette, and what it is worth.**
///
/// `Screen_DrawBattlefield`'s other arm is `Palette_Set(0x5675A0)`, which
/// `Res_LoadStatic` (`0x00499859`) fills from record 1 of `g_preloadTable` —
/// `t32_stn1.256`, spelled in the table's bytes at `0x004D9F5C`. Ours named
/// `t32_bat1.256` for every battle and `shell::PALETTES` loaded only that, so
/// a siege was shown in the field battle's colours.
///
/// **Measured, and the measurement is the finding.** The two files differ on
/// **3 of 256 entries** — 0, 115 and 172 in this install; 172 is field green
/// `(33, 49, 18)` against siege brown `(37, 13, 2)`. **No pixel of a painted
/// siege changes colour today**, because we draw a siege from
/// `T32_bat1.pl8` and none of its tiles uses those three indices. They are
/// `T32_stn1.pl8`'s — the sheet the original draws a siege from, which is not
/// ported. So this arm is correct, cheap, and worth nothing on screen until
/// the siege tileset lands; the two counts are printed every run rather than
/// asserted, because both come from the player's own files.
///
/// Ablation: delete `"T32_stn1.256"` from `shell::PALETTES` — red on the
/// lookup, which is the fall-through to `base01.256`.
#[test]
fn a_siege_is_shown_in_t32_stn1_and_that_changes_the_picture() {
    let Some((assets, platform)) = install() else {
        l2_testkit::skip!("no game install, so no t32_stn1.256 to read");
    };
    let stn1 = Palette::from_bytes(&platform.vfs.read("T32_stn1.256").expect("T32_stn1.256"))
        .expect("768 bytes");
    let bat1 = Palette::from_bytes(&platform.vfs.read("T32_bat1.256").expect("T32_bat1.256"))
        .expect("768 bytes");

    let (mut g, mut m) = staged_siege(3);
    let mut canvas = Canvas::screen();
    frame(&mut m, &mut g, &assets, &mut canvas);
    assert!(g.battle.as_ref().expect("a live battle").runner.siege.is_siege);

    let name = m.palette_name().expect("a battlefield names a palette");
    assert_eq!(name, "T32_stn1.256", "a siege is painted in the field battle's palette");
    let applied = assets.shell.palette(name).expect("and the shell loads it");
    for i in 0..=255u8 {
        assert_eq!(applied.rgb(i), stn1.rgb(i), "index {i} of the shell's copy");
    }

    let entries: Vec<u8> = (0..=255u8).filter(|&i| stn1.rgb(i) != bat1.rgb(i)).collect();
    let mut moved = 0usize;
    for y in FIELD_Y0..FIELD_Y1 {
        for x in FIELD_X0..FIELD_X1 {
            if entries.contains(&canvas.at(x as usize, y as usize)) {
                moved += 1;
            }
        }
    }
    // And the same count over the sheet the original would have drawn from,
    // which is where those indices live.
    let stn = Sheet::new(platform.vfs.read("T32_stn1.pl8").expect("T32_stn1.pl8")).expect("a sheet");
    let (mut stn_px, mut stn_hit) = (0usize, 0usize);
    for i in 0..stn.frame_count() {
        let Some(f) = stn.frame(i) else { continue };
        for (n, &idx) in f.indices.iter().enumerate() {
            if !f.opaque[n] {
                continue;
            }
            stn_px += 1;
            if entries.contains(&idx) {
                stn_hit += 1;
            }
        }
    }
    eprintln!(
        "t32_stn1.256 vs t32_bat1.256: {} of 256 entries differ ({entries:?}); \
         {moved} of {} painted siege pixels change colour today, and {stn_hit} of {stn_px} \
         pixels of T32_stn1.pl8 — the sheet we do not draw — would",
        entries.len(),
        (FIELD_X1 - FIELD_X0) * (FIELD_Y1 - FIELD_Y0),
    );
    assert!(!entries.is_empty(), "the two battle palettes are the same file");
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
/// pixels right of where it stood, moving monotonically on the way. **The
/// first sample is painted before any tick runs**: the commit *is* the move,
/// so by the end of tick 1 he is already on the next cell and drawn two
/// pixels along it.
///
/// Ablation: return the facing's neighbour with `walking = substep − 1` from
/// `l2_view::scene::drawn_cell` — red at once, a cell ahead of himself.
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
    // Macemen cross a cell in 16 ticks; five cells and a margin. Tick 0 is the
    // paint before the first update — where he stands, not where he commits.
    for t in 0..(5 * 16 + 40) {
        if t == 0 {
            paint(&mut m, &mut g, &assets, &mut canvas);
        } else {
            frame(&mut m, &mut g, &assets, &mut canvas);
        }
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

// ------------------------------------------------------------- the jump count

/// Two armies eight cells apart, marched at each other. 42 figures, no
/// install, no painter — [`l2_view::scene::figure_origin`] is the whole
/// picture and this counts how far it moves a live man in one tick.
fn march() -> BattleRunner {
    let human: &[(Troop, u16)] = &[(Troop::Swordsmen, 8), (Troop::Archers, 6), (Troop::Macemen, 7)];
    let ai: &[(Troop, u16)] = &[(Troop::Crossbowmen, 7), (Troop::Macemen, 8), (Troop::Peasants, 6)];
    let mut r = BattleRunner::deploy_armies(
        field(44),
        0x5EED,
        Army { troops: ai, owner: 2, human: false },
        Army { troops: human, owner: 1, human: true },
    );
    for u in 1..=l2_sim::unit::MAX_UNITS {
        if r.units.get(u).is_live() && r.units.get(u).human {
            r.order_unit(u, 40, 44);
        }
    }
    r
}

/// **A drawn man never teleports.** One tick moves him four pixels along one
/// axis — `g_walkOffset[dirc][walking]` is `±(32 − 2·walking)` and `walking`
/// climbs 1, 3 … 15 — or leaves him where he is. Sixteen pixels is half a
/// cell: nothing the original draws jumps that far.
///
/// `BattleMan_Step` (`0x0048F1DD`) is why. A man who is mid-crossing —
/// `stepFlags & 1` clear, figure `+0x34` — returns from it before the
/// direction, the melee search or `Cell_TryEnter` is reached, so his `dirc` is
/// fixed and his cell is already the one he is entering (`FUN_00491B1F` ran at
/// the commit). Ours counted first and entered last, and re-chose the
/// direction every tick: 1,317 jumps before C183 registered the palette and
/// moved the trail, 504 after it, 0 once the order matched.
///
/// Ablation: put the `next_step` / facing block back above
/// `Progress::tick` in `BattleRunner::step_one` — red, with the mid-crossing
/// turns back.
#[test]
fn no_drawn_man_ever_jumps_half_a_cell_in_one_tick() {
    let mut r = march();
    let cam = l2_view::scene::Camera { x: 24, y: 28 };
    let mut was: Vec<Option<(i32, i32)>> = vec![None; r.fighters.len()];
    let (mut jumps, mut walking_ticks) = (0usize, 0usize);
    let mut worst = (0, String::new());
    for tick in 0..1_200 {
        for i in 0..r.fighters.len() {
            let f = &r.fighters[i];
            if !r.is_alive(i) {
                was[i] = None;
                continue;
            }
            let now = l2_view::scene::figure_origin(f, cam);
            if f.anim == Motion::Walking {
                walking_ticks += 1;
            }
            if let Some(before) = was[i] {
                let d = (now.0 - before.0).abs().max((now.1 - before.1).abs());
                if d >= 16 {
                    jumps += 1;
                    if d > worst.0 {
                        worst = (d, format!("figure {i} at tick {tick}: {before:?} -> {now:?}"));
                    }
                }
            }
            was[i] = Some(now);
        }
        r.step();
    }
    println!("{} figures, 1200 ticks, {walking_ticks} walking figure-ticks, {jumps} jumps", r.fighters.len());
    assert_eq!(jumps, 0, "{jumps} drawn jumps of 16 px or more; worst {}", worst.1);
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
            l2_view::scene::draw_terrain(
                &mut ground,
                &live(&g).runner.field,
                art.tiles(),
                None,
                cam,
            );
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
/// the other never painted.
///
/// **The battle ends inside the loop, and that is the original's answer.**
/// `Battle_CheckOutcome` (`0x00477DFC`): `if (menA < 1) g_battleLoser =
/// g_battleArmyB` — no clock, no morale break. It then counts `DAT_00568470`
/// to 5000 behind the banner, and `Smk_OnFinished` sets that to 5001 when the
/// outcome film ends. So with the install's artwork `turn::finish_battle` takes
/// the battle (tick 902 here); with the placeholder no film opens and the
/// banner keeps its 5000 frames. Casualties are therefore counted while there
/// is still a battle to count them from, and the per-tick equality below is
/// what holds the two copies to the same end.
fn played(assets: &Assets, ticks: u32) -> (Game, Game, u32) {
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
    let mut killed: u32 = 0;
    let mut handed_back: Option<u32> = None;
    for t in 0..ticks {
        frame(&mut dm, &mut drawn, assets, &mut canvas);
        let mut ctx = Ctx { game: &mut blind, assets };
        bm.update(&mut ctx);
        assert!(drawn.battle == blind.battle, "painting changed the battle at tick {t}");
        match drawn.battle.as_deref() {
            Some(live) => {
                killed = l2_sim::ALL_TROOPS
                    .iter()
                    .map(|&troop| live.runner.sim.cues.melee_casualties(troop))
                    .sum()
            }
            None => handed_back = handed_back.or(Some(t)),
        }
    }
    eprintln!("{ticks} ticks: the battle was handed back to the turn at {handed_back:?}");
    (drawn, blind, killed)
}

/// Check the two plays agree to the byte, that the battle did something, and
/// print the fingerprint.
fn same_battle(drawn: &Game, blind: &Game, label: &str, killed: u32) {
    let (a, b) = (l2_game::save::encode(drawn), l2_game::save::encode(blind));
    assert_eq!(a, b, "the saved bytes differ between the painted and the unpainted battle");
    eprintln!("{label}: saved battle {} bytes, fnv1a {:016x}", a.len(), fnv1a(&a));
    assert!(killed > 0, "nobody fought, so the comparison is about a battle with nothing in it");
}

/// **Painting does not change the battle** — with the placeholder artwork, so
/// it runs everywhere.
///
/// Compared by the whole `LiveBattle` every tick — that is the half carrying
/// the battle. `save::encode` holds no `LiveBattle` (it is `l2_kingdom::save`
/// plus a ten-field prefix), so its bytes say only that the campaign around the
/// battle came through untouched.
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
    let (drawn, blind, killed) = played(&assets, 3_000);
    same_battle(&drawn, &blind, "placeholder", killed);
}

// ---------------------------------------------------------------- the minimap
//
// **A fourth report, on build `EE0CB9233`**: *"battle is still a blue mess
// where the grass should be, a black minimap"*. The blue was C183's palette;
// the black was a painter we did not have.

/// **The two 2 × 2 sheets, read straight out of the install** — an 8-byte
/// header, then 16-byte frame records with `dataOffset` at `+4`. Written out
/// here rather than taken from `l2_view::sheet` so the expectation is the
/// player's own file and not our decoder.
fn t2_pixels(bytes: &[u8], index: usize) -> [u8; 4] {
    let count = u16::from_le_bytes([bytes[2], bytes[3]]) as usize;
    assert!(index < count, "frame {index} of {count}");
    let rec = 8 + index * 16;
    assert_eq!(u16::from_le_bytes([bytes[rec], bytes[rec + 1]]), 2, "frame {index} is not 2 wide");
    assert_eq!(
        u16::from_le_bytes([bytes[rec + 2], bytes[rec + 3]]),
        2,
        "frame {index} is not 2 high"
    );
    let off = u32::from_le_bytes(bytes[rec + 4..rec + 8].try_into().unwrap()) as usize;
    bytes[off..off + 4].try_into().unwrap()
}

/// `FUN_004BC107(…, 0x1E0, 0x18, 2)` and `FUN_004BC51A`'s `g_drawX += 2` /
/// `g_drawY += 2`. Literals, not `l2_view::scene`'s constants.
const PANEL_X: usize = 0x1E0;
const PANEL_Y: usize = 0x18;
/// `FUN_004BC020(…, 0x50, 0x50, …)`.
const PANEL_CELLS: usize = 0x50;
/// `Battle_Frame`'s `FUN_004bc1d1(4)`.
const PANEL_ROWS_A_FRAME: usize = 4;

/// The colour `FUN_004BC51A` picks for the man standing on each cell:
/// `g_realms[man.owner].shieldIndex`, or `6` for owner 6, and `0` — no man —
/// everywhere else. The *cell* is `mapX`/`mapY`, which `FUN_00491B1F` moves at
/// the start of a crossing; [`l2_view::scene::drawn_cell`] is that rule, and is
/// not what these two tests are checking.
fn expected_occupants(g: &Game) -> Vec<u8> {
    let l = live(g);
    let mut occ = vec![0u8; PANEL_CELLS * PANEL_CELLS];
    for i in 0..l.runner.fighters.len() {
        if !l.runner.is_alive(i) {
            continue;
        }
        let f = &l.runner.fighters[i];
        let owner = l.runner.sim.figures[f.sim].owner;
        let colour = if owner == 6 { 6 } else { g.kingdom.realms[owner as usize].shield_index };
        if colour == 0 {
            continue;
        }
        let ((cx, cy), _) = l2_view::scene::drawn_cell(f);
        occ[cy as usize * PANEL_CELLS + cx as usize] = colour;
    }
    occ
}

/// The whole 160 × 160 panel as the original would have it this instant.
fn expected_panel(g: &Game, bat1: &[u8], spri: &[u8]) -> Vec<u8> {
    let occ = expected_occupants(g);
    let side = PANEL_CELLS * 2;
    let mut out = vec![0u8; side * side];
    for cy in 0..PANEL_CELLS {
        for cx in 0..PANEL_CELLS {
            let colour = occ[cy * PANEL_CELLS + cx];
            let px = if colour == 0 {
                t2_pixels(bat1, live(g).runner.field.at(cx, cy).gfx as usize)
            } else {
                t2_pixels(spri, colour as usize)
            };
            for j in 0..2 {
                for i in 0..2 {
                    out[(cy * 2 + j) * side + cx * 2 + i] = px[j * 2 + i];
                }
            }
        }
    }
    out
}

/// Which cell rows of the panel on `canvas` disagree with `want`.
fn stale_rows(canvas: &Canvas, want: &[u8]) -> Vec<usize> {
    let side = PANEL_CELLS * 2;
    (0..PANEL_CELLS)
        .filter(|&cy| {
            (0..2).any(|j| {
                let y = cy * 2 + j;
                (0..side).any(|x| canvas.at(PANEL_X + x, PANEL_Y + y) != want[y * side + x])
            })
        })
        .collect()
}

/// A staged battle whose two realms carry shields 2 and 5 — the byte
/// `FUN_004BC51A` indexes `t2_spri.pl8` with.
fn staged_with_shields(
    ai_row: usize,
    human: &[(Troop, u16)],
    ai: &[(Troop, u16)],
) -> (Game, Machine) {
    let (mut g, m) = staged(ai_row, human, ai, |(x, y)| (x as i32 - 7, y as i32 - 6));
    g.kingdom.realms[1].shield_index = 2;
    g.kingdom.realms[2].shield_index = 5;
    (g, m)
}

/// **The panel is `t2_bat1.pl8` under `t2_spri.pl8`, pixel for pixel, and there
/// is no rectangle on it.**
///
/// `Battle_LoadAssets` (`0x004987B7`) registers the painter with
/// `FUN_004BC107(t2_bat1, t2_bat2, t2_spri, 0x1E0, 0x18, 2)` — entries `0x0B`,
/// `0x0C` and `0x11` of the asset table at `0x004DA550` — and `FUN_004BC51A`
/// draws exactly two things a cell: frame `cell[+3]` of the tileset, or frame
/// `g_realms[owner].shieldIndex` of `t2_spri` over a cell holding a man. It
/// draws **no viewport rectangle**, and this compares every pixel of the panel,
/// so a rectangle anywhere on it is red.
///
/// The expectation is decoded from the install's own two files by [`t2_pixels`].
///
/// Ablation: the panel this replaced — `fill_rect(ink.background)`, a dot a
/// *side*, and a `widget::frame` round the camera — is stale on all eighty
/// rows.
#[test]
fn the_overview_panel_is_t2_bat1_under_t2_spri_with_no_rectangle_on_it() {
    let Some((assets, platform)) = install() else {
        l2_testkit::skip!("no game install, so no T2_bat1.pl8 to compare the panel against");
    };
    let bat1 = platform.vfs.read("T2_bat1.pl8").expect("T2_bat1.pl8");
    let spri = platform.vfs.read("T2_spri.pl8").expect("T2_spri.pl8");
    assert_eq!(
        u16::from_le_bytes([bat1[2], bat1[3]]),
        252,
        "t2_bat1 is one 2 x 2 frame per T32_bat1 tile"
    );
    assert_eq!(
        u16::from_le_bytes([spri[2], spri[3]]),
        7,
        "t2_spri is the erase tile and six colours"
    );

    let (mut g, mut m) = staged_with_shields(
        74,
        &[(Troop::Swordsmen, 3), (Troop::Archers, 3)],
        &[(Troop::Macemen, 3)],
    );
    let mut canvas = Canvas::screen();
    paint(&mut m, &mut g, &assets, &mut canvas);

    let want = expected_panel(&g, &bat1, &spri);
    let stale = stale_rows(&canvas, &want);
    assert!(stale.is_empty(), "the panel disagrees with the install's sheets on cell rows {stale:?}");

    // And the men are coloured by their **realm**, not by their side: shields 2
    // and 5 are different frames of `t2_spri`, so both colours are on the panel.
    let (a, b) = (t2_pixels(&spri, 2)[0], t2_pixels(&spri, 5)[0]);
    assert_ne!(a, b, "shields 2 and 5 are the same colour in this install's t2_spri.pl8");
    let count = |idx: u8| {
        (0..PANEL_CELLS * 2)
            .flat_map(|y| (0..PANEL_CELLS * 2).map(move |x| (x, y)))
            .filter(|&(x, y)| canvas.at(PANEL_X + x, PANEL_Y + y) == idx)
            .count()
    };
    assert!(count(a) >= 4, "realm 1's shield colour {a} is not on the panel");
    assert!(count(b) >= 4, "realm 2's shield colour {b} is not on the panel");
}

/// **Four rows a frame, and the whole panel in twenty** — `FUN_004BC1D1`
/// (`0x004BC1D1`), which `Battle_Frame` (`0x004B99C0`) calls with `4` once
/// `g_mapRedraw` has been spent:
///
/// ```c
/// DAT_004E5D74 += n;
/// if (0x50 - n < DAT_004E5D74) DAT_004E5D74 = 0;
/// if (DAT_004E5D58 == 2) FUN_004BC51A(DAT_004E5D74, n);
/// ```
///
/// `Screen_DrawBattlefield` (`0x004233F7`) enters with `g_mapRedraw = 1` and
/// `FUN_004bc1d1(0x50)`; `FUN_004BC142`, called straight after the schedule in
/// `Battle_Frame`, ends `if (g_mapRedraw != 0) g_mapRedraw--`. **So the full
/// pass happens once and the panel then lags the field by up to twenty
/// frames.** Without that decrement the obvious reading is that the full pass
/// runs every frame, and it does not.
///
/// Every cell's tile is changed under the panel, which makes all eighty cell
/// rows stale at once; one frame may then repaint four of them and no more.
///
/// Ablation: paint all eighty rows a frame — red, nothing is ever stale.
#[test]
fn the_overview_panel_is_repainted_four_cell_rows_a_frame() {
    let Some((assets, platform)) = install() else {
        l2_testkit::skip!("no game install, so no T2_bat1.pl8 to repaint the panel from");
    };
    let bat1 = platform.vfs.read("T2_bat1.pl8").expect("T2_bat1.pl8");
    let spri = platform.vfs.read("T2_spri.pl8").expect("T2_spri.pl8");

    let (mut g, mut m) = staged_with_shields(74, &[(Troop::Swordsmen, 3)], &[(Troop::Macemen, 3)]);
    // Paused, so the men stand still and the only thing that changes under the
    // panel is what this test changes. `Battle_Frame` schedules the panel
    // whether or not the battle is paused.
    g.battle.as_mut().expect("a live battle").paused = true;
    let mut canvas = Canvas::screen();
    // The entry pass: `Screen_DrawBattlefield`'s `FUN_004bc1d1(0x50)`.
    frame(&mut m, &mut g, &assets, &mut canvas);
    assert!(
        stale_rows(&canvas, &expected_panel(&g, &bat1, &spri)).is_empty(),
        "the entry pass did not paint the whole panel"
    );

    {
        let field = &mut g.battle.as_mut().unwrap().runner.field;
        for cell in field.cells.iter_mut() {
            let old = t2_pixels(&bat1, cell.gfx as usize);
            cell.gfx = (1..252)
                .map(|d| ((cell.gfx as usize + d) % 252) as u8)
                .find(|&n| t2_pixels(&bat1, n as usize) != old)
                .expect("some frame of t2_bat1 is drawn in different pixels");
        }
    }
    let want = expected_panel(&g, &bat1, &spri);
    assert_eq!(
        stale_rows(&canvas, &want).len(),
        PANEL_CELLS,
        "the field did not change under the panel"
    );

    // One frame repaints four rows, and they are the four the cursor lands on:
    // the entry pass left it at 0, so `0 + 4`.
    frame(&mut m, &mut g, &assets, &mut canvas);
    let stale = stale_rows(&canvas, &want);
    let fresh: Vec<usize> = (0..PANEL_CELLS).filter(|r| !stale.contains(r)).collect();
    assert_eq!(
        fresh,
        (PANEL_ROWS_A_FRAME..2 * PANEL_ROWS_A_FRAME).collect::<Vec<_>>(),
        "one frame repainted cell rows {fresh:?}"
    );

    // And the cursor wraps: eighty rows at four a frame is twenty frames.
    for _ in 1..(PANEL_CELLS / PANEL_ROWS_A_FRAME) {
        frame(&mut m, &mut g, &assets, &mut canvas);
    }
    let left = stale_rows(&canvas, &want);
    assert!(left.is_empty(), "twenty frames left cell rows {left:?} stale");
}

/// The same with the install's artwork, so the painter that runs is
/// `l2_view::scene::draw` — the one this file changed — rather than the
/// placeholder's blocks.
#[test]
fn painting_the_battlefield_with_its_artwork_does_not_change_the_battle() {
    let Some((assets, _platform)) = install() else {
        l2_testkit::skip!("no game install, so the placeholder test above is the whole check");
    };
    let (drawn, blind, killed) = played(&assets, 1_500);
    same_battle(&drawn, &blind, "install", killed);
}
