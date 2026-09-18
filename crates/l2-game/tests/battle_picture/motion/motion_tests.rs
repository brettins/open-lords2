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

/// `BattleMan_Step` (`0x0048F1DD`) is why. A man who is mid-crossing —
/// `stepFlags & 1` clear, figure `+0x34` — returns from it before the
/// direction, the melee search or `Cell_TryEnter` is reached, so his `dirc` is
/// fixed and his cell is already the one he is entering (`FUN_00491B1F` ran at
/// the commit). Ours counted first and entered last, and re-chose the
/// direction every tick: 1,317 jumps before C183 registered the palette and
/// moved the trail, 504 after it, 0 once the order matched.
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

/// **The walk frame is the phase before the step.** `Anim_WalkA2`
/// (`00480000.c:2754-2756`) reads `animPhase >> 2` and increments after, so the
/// pose drawn this tick is the counter the figure ended the last tick with.
#[test]
fn a_marching_man_is_drawn_at_the_phase_he_ended_the_last_tick_with() {
    let mut r = march();
    let mut carried: Vec<Option<u8>> = vec![None; r.fighters.len()];
    let mut checked = 0usize;
    for _ in 0..200 {
        r.step();
        for i in 0..r.fighters.len() {
            let f = &r.fighters[i];
            if f.anim != Motion::Walking {
                carried[i] = None;
                continue;
            }
            if let Some(before) = carried[i] {
                assert_eq!(l2_view::figures::pose_of(&r, i).phase, before, "figure {i}");
                checked += 1;
            }
            carried[i] = Some(f.phase);
        }
    }
    println!("{checked} consecutive marching ticks compared");
    assert!(checked > 100, "only {checked} marching ticks compared");
}

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

        let f = &live(&g).runner.fighters[man];
        let away = (f.x as i32 - start.0 as i32).abs().max((f.y as i32 - start.1 as i32).abs());
        let others = live(&g).runner.fighters.iter().enumerate().any(|(i, o)| {
            i != man
                && live(&g).runner.is_alive(i)
                && (o.x as i32 - start.0 as i32).abs().max((o.y as i32 - start.1 as i32).abs()) <= 2
        });
        if away >= 3 && !others {
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

/// `BattleFigure_Draw` (`0x004BDC31`) and the horse under a knight
/// (`FUN_004BE4DF`) both call `Clip_Horizontal(DAT_004E6564, DAT_004E5D54)` and
/// `Clip_Vertical(DAT_004E5D48, DAT_004E5D6C)` before the blit, and
/// `FUN_004BC020` has set those to 0, 480, 24 and 472. Three walks: north-east
/// across the top-right corner, south under the bottom, and east along a row.
///
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

/// **A green ablation, and that is the finding**, as it was for sound in C166:
///
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

/// `Anim_DrawBowA2` (`0x0048804A`) is `facing * N + 10 + pose`, poses 10 … 12,
/// and only crossbowmen and archers have the N = 13 that leaves room for it.
///
/// **Ablation**: put the base back to 6 (the old `WALK_BASE`) and the draw
/// collides with the strike band; index the band by `phase / 4` instead of the
/// `swingTimer` curve `g_drawBowArcher` (`0x004D9AC8`) and the archer shows one
/// frame for the first eight ticks and pose 12 forever after, so the count of
/// three fails.
#[test]
fn an_archer_drawing_a_bow_is_three_frames_of_its_own() {
    let Some((_assets, platform)) = install() else {
        eprintln!("no install; skipped");
        return;
    };
    let file = l2_view::figures::sprite_file(l2_view::figures::Colour::Red, Troop::Archers).expect("archers");
    let sheet = Sheet::new(platform.vfs.read(&file).expect("the archer sheet")).expect("a PL8");
    let stride = l2_view::figures::poses_per_facing(Troop::Archers) as usize;
    assert_eq!(stride, 13, "only N = 13 leaves room for the bow band");

    for facing in 0..8u8 {
        let stand = l2_view::figures::frame(Troop::Archers, Motion::Idle, facing, 0);
        let mut drawn = std::collections::BTreeSet::new();
        for swing in 0..80u16 {
            let pose = l2_view::figures::Pose { swing, ..Default::default() };
            let i = l2_view::figures::frame(Troop::Archers, Motion::Shooting, facing, pose);
            let pose = i - facing as usize * stride;
            assert!((10..13).contains(&pose), "facing {facing} swing {swing} pose {pose}");
            assert_ne!(i, stand, "the bow draw is not the standing pose");
            assert!(sheet.frame(i).is_some(), "frame {i} does not decode");
            drawn.insert(i);
        }
        assert_eq!(drawn.len(), 3, "the draw is three frames, facing {facing}");
    }
}


