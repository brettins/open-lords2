#![allow(unused_imports)]
use super::*;
use super::helpers::*;
use super::*;
use super::render::*;
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
    let mut cell_was: Vec<Option<(u8, u8)>> = vec![None; r.fighters.len()];
    let (mut jumps, mut walking_ticks) = (0usize, 0usize);
    let mut worst = (0, String::new());
    for tick in 0..1_200 {
        for i in 0..r.fighters.len() {
            let f = &r.fighters[i];
            if !r.is_alive(i) {
                was[i] = None;
                cell_was[i] = None;
                continue;
            }
            let now = l2_view::scene::figure_origin(f, cam);
            if f.anim == Motion::Walking {
                walking_ticks += 1;
            }
            if let Some(before) = was[i] {
                let d = (now.0 - before.0).abs().max((now.1 - before.1).abs());
                // **One exception, and it is the original's.**
                // `BattleMen_SwapPlaces` (`0x0049005F`) exchanges the two men's
                // `mapX`, `mapY` and `cellOffset` outright and touches neither
                // `walking` nor `dirc`; its caller `BattleMan_Step`
                // (`0x0048F1DD`) delays the other man and returns 0. So both
                // men change cell in one frame with no walk offset and **the
                // original draws the jump too.** Its signature is exactly that:
                // one cell moved, with somebody else standing in the cell just
                // left. Anything else still fails.
                let swapped = cell_was[i].is_some_and(|(cx, cy)| {
                    let step =
                        (f.x as i32 - cx as i32).abs().max((f.y as i32 - cy as i32).abs());
                    // Either half of the pair: the man now standing in the cell
                    // just left, or — for the man swapped *out* of his cell —
                    // the delay that arm gave him, which nothing else writes.
                    step == 1
                        && (f.delay > 0
                            || (0..r.fighters.len()).any(|j| {
                                j != i
                                    && r.is_alive(j)
                                    && (r.fighters[j].x, r.fighters[j].y) == (cx, cy)
                            }))
                });
                if d >= 16 && !swapped {
                    jumps += 1;
                    if d > worst.0 {
                        worst = (d, format!("figure {i} at tick {tick}: {before:?} -> {now:?}"));
                    }
                }
            }
            was[i] = Some(now);
            cell_was[i] = Some((f.x, f.y));
        }
        r.step();
    }
    println!("{} figures, 1200 ticks, {walking_ticks} walking figure-ticks, {jumps} jumps", r.fighters.len());
    assert_eq!(jumps, 0, "{jumps} drawn jumps of 16 px or more; worst {}", worst.1);
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
/// screen repaints those pixels,
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
/// terrain pass repaints every cell every frame
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

/// **Painting does not change the battle** — with the placeholder artwork, so
/// it runs everywhere.
///
/// Compared by the whole `LiveBattle` every tick — that is the half carrying
/// the battle. `save::encode` holds no `LiveBattle` (it is `l2_kingdom::save`
/// plus a ten-field prefix), so its bytes say only that the campaign around the
/// battle came through untouched.
///
/// **A green ablation, and that is the finding**, as it was for sound in C166:
/// `Screen::draw` receives `&Ctx`, whose `game` it can only read
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


