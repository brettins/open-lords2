#![allow(unused_imports)]
use super::*;
use super::picture_comparison::*;
use super::cell_selection::*;
use super::*;
use super::tables_and_sheets::*;
use l2_game::battlefield::LiveBattle;
use l2_game::game::Assets;
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::Game;
use l2_sim::runner::{Army, BattleRunner};
use l2_sim::siege::{code, frames_with_code, STRUCTURE_STONE, STRUCTURE_WOOD};
use l2_sim::terrain::{tileset, DIM};
use l2_sim::Troop;
use l2_view::scene::{self, Ground};
use l2_view::Canvas;

/// **The keep flies the garrison's banner** — `BattleBanner_Draw`
/// (`FUN_004BD574`, `0x004BD574`), reached from the overlap pass
/// `FUN_004BD355` on cell byte `+2` bit `0x80` with the frame byte zero.
///
/// Ablation, run: drop the `cell.gfx == 0` half of `scene::banner_cell` — red,
/// *"285 of 570 changed pixels moved outside the keep cell"*: the decoy cell
/// below carries the bit with a frame byte of its own, which in the original
/// takes `FUN_004BD759` instead.
#[test]
fn the_keep_flies_the_garrisons_banner() {
    let dir = l2_testkit::install!();
    let read = |n: &str| {
        std::fs::read(dir.join(n)).map_err(|e| format!("{n}: {e}"))
    };
    let assets = scene::BattleAssets::load(read, l2_view::figures::Colour::Red, l2_view::figures::Colour::Blue)
        .expect("battle assets");
    let miss = assets.missiles().expect("A2_miss.pl8");
    for shield in 0..6u8 {
        for phase in 0..scene::BANNER_PHASES {
            let f = scene::banner_frame(shield, phase);
            assert!(miss.frame(f).is_some(), "A2_miss.pl8 frame {f} (shield {shield})");
        }
    }
    assert_eq!(scene::banner_frame(5, 7), miss.frame_count() - 1, "the block ends the sheet");

    let mut field = l2_sim::siege::our_castle(4);
    let keep = field
        .cells
        .iter()
        .position(|c| c.flags2 & 0x80 != 0 && c.gfx == 0)
        .expect("a castle has a keep cell");
    let (kx, ky) = (keep % DIM, keep / DIM);
    // A neighbour carrying the same bit with a frame byte of its own — the
    // original's `gfx ? FUN_004bd759(gfx) : FUN_004bd574()`. It must not fly a
    // banner, and it is what holds the `gfx == 0` half of the gate.
    let masked = keep + 2;
    assert_ne!(field.cells[masked].gfx, 0, "the decoy cell needs a frame byte");
    field.cells[masked].flags2 |= 0x80;
    let cam = scene::Camera::clamped(kx as i32 - 5, ky as i32 - 5);
    let runner = BattleRunner::deploy_armies(
        field,
        0x5EED,
        Army { troops: &[(Troop::Pikemen, 1)], owner: 2, human: false },
        Army { troops: &[(Troop::Swordsmen, 1)], owner: 1, human: true },
    );

    let mut bare = Canvas::screen();
    scene::draw_overlay_and_missiles(&mut bare, &runner, &assets, cam, None);
    let mut flown = Canvas::screen();
    scene::draw_overlay_and_missiles(&mut flown, &runner, &assets, cam, Some((1, 0)));

    let moved = bare.diff_count(&flown);
    assert!(moved > 0, "the banner painted nothing on the keep");

    let (px, py) = (
        scene::ORIGIN_X + (kx as i32 - cam.x as i32) * scene::TILE,
        scene::ORIGIN_Y + (ky as i32 - cam.y as i32) * scene::TILE,
    );
    let frame = miss.frame(scene::banner_frame(1, 0)).expect("the banner frame");
    let (w, h) = (frame.width as i32, frame.height as i32);
    let ox = px + (scene::TILE / 2 - w / 2);
    let oy = py + (scene::TILE / 2 - w / 2);
    let mut outside = 0;
    for y in 0..l2_view::canvas::HEIGHT as i32 {
        for x in 0..l2_view::canvas::WIDTH as i32 {
            if bare.at(x as usize, y as usize) == flown.at(x as usize, y as usize) {
                continue;
            }
            if x < ox || x >= ox + w || y < oy || y >= oy + h {
                outside += 1;
            }
        }
    }
    assert_eq!(outside, 0, "{outside} of {moved} changed pixels moved outside the keep cell");

    let mut seen: Vec<Canvas> = Vec::new();
    for phase in 0..scene::BANNER_PHASES {
        let mut c = Canvas::screen();
        scene::draw_overlay_and_missiles(&mut c, &runner, &assets, cam, Some((1, phase)));
        seen.push(c);
    }
    let still = seen.windows(2).filter(|w| w[0].diff_count(&w[1]) == 0).count();
    assert!(still < 4, "{still} of seven phase steps drew the same picture");
    eprintln!("banner: {moved} pixels over the keep at ({kx}, {ky})");
}

/// **A wall being shot visibly breaks up** — `Missile_Step`'s class-3 arm
/// (`0x00492C8B`) into `Battlefield_Draw32`'s second pass (`0x004BCBDC`).
#[test]
fn a_wall_under_the_catapult_breaks_up_in_the_picture() {
    let dir = l2_testkit::install!();
    let read = |n: &str| std::fs::read(dir.join(n)).expect("a sheet");
    let one = l2_view::sheet::Sheet::new(read("T32_stn1.pl8")).expect("stn1");
    let two = l2_view::sheet::Sheet::new(read("T32_stn2.pl8")).expect("stn2");

    use l2_sim::proving;
    let mut r = proving::deploy();
    for x in proving::HIGH_WALL_X {
        r.field.cells[proving::HIGH_WALL_Y as usize * DIM + x as usize].elevation = 3;
    }
    let seeds: Vec<u8> = proving::HIGH_WALL_X
        .map(|x| r.field.cells[proving::HIGH_WALL_Y as usize * DIM + x as usize].terrain)
        .collect();

    for _ in 0..600 {
        proving::orders(&mut r);
        r.step();
        if r.sim.cues.walls_struck() >= 3 {
            break;
        }
    }
    assert!(r.sim.cues.walls_struck() >= 3, "the catapult never hit the wall");

    let (hit, seed) = proving::HIGH_WALL_X
        .zip(seeds)
        .map(|(x, s)| (proving::HIGH_WALL_Y as usize * DIM + x as usize, s))
        .find(|&(i, s)| r.field.cells[i].terrain > s)
        .expect("a shot raised some wall cell's byte +0");
    let damaged = r.field.cells[hit].terrain;
    assert!(damaged > seed, "cell byte +0 rose from {seed} to {damaged}");
    assert!(damaged <= l2_sim::missile::WALL_DAMAGE_MAX, "and it has not collapsed yet");
    assert!(
        scene::OVERLAY_ELEVATIONS.contains(&r.field.cells[hit].elevation),
        "the overlay draws on this cell"
    );

    let (hx, hy) = (hit % DIM, hit / DIM);
    let cam = scene::Camera::clamped(hx as i32 - 5, hy as i32 - 5);
    let mut after = Canvas::screen();
    scene::draw_terrain(&mut after, &r.field, &one, Some(&two), cam);
    let mut before = r.field.clone();
    before.cells[hit].terrain = seed;
    let mut undamaged = Canvas::screen();
    scene::draw_terrain(&mut undamaged, &before, &one, Some(&two), cam);

    let moved = undamaged.diff_count(&after);
    assert!(moved > 0, "the damage overlay did not move");
    assert!(
        moved <= (scene::TILE * scene::TILE) as usize,
        "{moved} pixels moved for a one-cell overlay"
    );
    assert!(
        two.frame(scene::OVERLAY_BASE + damaged as usize).is_some(),
        "T32_stn2.pl8 carries frame {:#X}",
        scene::OVERLAY_BASE + damaged as usize
    );
    eprintln!("wall damage: byte +0 {seed} -> {damaged}, {moved} pixels");
}


