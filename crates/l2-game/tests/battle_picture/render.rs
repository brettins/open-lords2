#![allow(unused_imports)]
use super::*;
use super::motion::*;
use super::panel::*;
use super::entities::*;
use super::ui::*;
use l2_formats::{DecodedFrame, Palette};
use l2_game::battlefield::{self as bf, LiveBattle};
use l2_game::game::Assets;
use l2_game::input::Event;
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::menubar;
use l2_game::Game;
use l2_sim::runner::{Army, BattleRunner};
use l2_sim::terrain::DIM;
use l2_sim::{Motion, Troop, SIDE_A, SIDE_B};
use l2_view::sheet::Sheet;
use l2_view::Canvas;

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
/// It stops at the resolved palette because the
/// presenter is being moved into the library by the concurrent overlay-palette
/// branch, as `Machine::present`; once that has landed, the loop below belongs
/// on `m.present(…)`'s bytes, and the answer must not change.
///
/// The discrimination half is what keeps it from passing on a palette that
/// agrees: the same pixels through `base01.256` must differ, and on the
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
/// the siege tileset lands; the two counts are printed every run
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

/// What fraction of a frame's opaque pixels sit on the canvas at `(x, y)`.
/// For a sprite something else is drawn over — [`locate`] wants every pixel.
fn matched(canvas: &Canvas, frame: &DecodedFrame, (x, y): (i32, i32)) -> f64 {
    let pts = opaque(frame);
    let (mut hit, mut seen) = (0usize, 0usize);
    for &(dx, dy, idx) in &pts {
        let (px, py) = (x + dx, y + dy);
        if px < FIELD_X0 || px >= FIELD_X1 || py < FIELD_Y0 || py >= FIELD_Y1 {
            continue;
        }
        seen += 1;
        hit += usize::from(canvas.at(px as usize, py as usize) == idx);
    }
    if seen == 0 {
        return 0.0;
    }
    hit as f64 / seen as f64
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

