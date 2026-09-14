#![allow(unused_imports)]
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

/// **The install's own sheets, drawn.** A level-4 siege and a field battle of
/// the same shape are painted through the whole screen stack; the two pictures
/// must differ over most of the viewport, and the siege's must have no holes.
///
/// The probe is the viewport `FUN_004BC020` stores — `x 0…480`, `y 24…472` —
/// written out
///
/// Ablation, run: `Ground::for_battle` answering `Field` always — red, 212,253
/// of 215,040 viewport pixels never painted.
#[test]
fn a_siege_and_a_field_battle_of_the_same_shape_are_different_pictures() {
    let dir = l2_testkit::install!();
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let a = Assets::load(&platform.vfs).expect("assets load");
    let art = a.battle.as_ref().expect("the install has the battle art");
    assert!(art.has_ground(Ground::Stone), "the install ships T32_stn1.pl8 and T32_stn2.pl8");
    assert!(art.has_ground(Ground::Wood), "the install ships T32_wod1.pl8 and T32_wod2.pl8");
    assert_eq!(art.ground(Ground::Stone).tiles.frame_count(), 256, "T32_stn1.pl8");
    assert_eq!(
        art.ground(Ground::Stone).tiles2.as_ref().expect("slot 1").frame_count(),
        247,
        "T32_stn2.pl8"
    );

    let (mut gs, mut ms) = staged(Some(4));
    let siege = paint(&mut ms, &mut gs, &a);
    let (mut gf, mut mf) = staged(None);
    let plain = paint(&mut mf, &mut gf, &a);

    let (x0, y0, x1, y1) = (0usize, 24usize, 480usize, 472usize);
    let mut differ = 0;
    let mut holes = 0;
    for y in y0..y1 {
        for x in x0..x1 {
            if siege.at(x, y) != plain.at(x, y) {
                differ += 1;
            }
            if siege.at(x, y) == 0 {
                holes += 1;
            }
        }
    }
    let total = (x1 - x0) * (y1 - y0);
    assert_eq!(holes, 0, "{holes} of {total} viewport pixels were never painted");
    assert!(
        differ * 2 > total,
        "only {differ} of {total} viewport pixels differ between a siege and a field battle"
    );
    eprintln!("siege picture: {differ} of {total} viewport pixels differ from the field's");
}

/// **A cell's selector decides which sheet its pixels come from**, at the
/// pixel. One cell is switched to slot 1 and back with everything else held
/// still, and the tile under it must change.
///
/// Ablation, run: `let sheet = Some(tiles)` in `draw_terrain` — red here, and
/// red on the picture test with 16,145 unpainted pixels.
#[test]
fn the_cell_selector_moves_a_tile_between_the_two_sheets() {
    let dir = l2_testkit::install!();
    let read = |n: &str| {
        std::fs::read(dir.join(n)).map_err(|e| format!("{n}: {e}"))
    };
    let one = l2_view::sheet::Sheet::new(read("T32_stn1.pl8").expect("stn1")).expect("stn1");
    let two = l2_view::sheet::Sheet::new(read("T32_stn2.pl8").expect("stn2")).expect("stn2");

    let mut field = l2_sim::siege::our_castle(4);
    let cam = scene::Camera { x: 30, y: 20 };
    let cell = (cam.y + 5) * DIM + (cam.x + 5);
    let (px, py) = (scene::ORIGIN_X + 5 * scene::TILE, scene::ORIGIN_Y + 5 * scene::TILE);

    // Frame 4 of each sheet, which the two files do not agree on.
    field.cells[cell].gfx = 4;
    field.cells[cell].elevation = 0;
    field.cells[cell].terrain = 0;

    field.cells[cell].flags2 = 0;
    let mut a = Canvas::screen();
    scene::draw_terrain(&mut a, &field, &one, Some(&two), cam);

    field.cells[cell].flags2 = tileset::SECOND;
    let mut b = Canvas::screen();
    scene::draw_terrain(&mut b, &field, &one, Some(&two), cam);

    let mut moved = 0;
    for y in 0..scene::TILE {
        for x in 0..scene::TILE {
            let (x, y) = ((px + x) as usize, (py + y) as usize);
            if a.at(x, y) != b.at(x, y) {
                moved += 1;
            }
        }
    }
    assert!(moved > 100, "only {moved} of 1024 pixels moved with the selector");
    assert_eq!(a.diff_count(&b), moved, "nothing outside that one cell changed");
}

/// **The raised-ground overlay** — `Battlefield_Draw32`'s second pass, frame
/// `terrain + 0x8B` out of slot 1, over any cell at elevation 1, 2 or 3.
///
/// Ablation, run: gate the overlay block on `false` — red, *"the overlay
/// painted nothing"*.
#[test]
fn a_raised_cell_takes_a_second_blit_from_the_second_sheet() {
    let dir = l2_testkit::install!();
    let read = |n: &str| std::fs::read(dir.join(n)).expect("a sheet");
    let one = l2_view::sheet::Sheet::new(read("T32_stn1.pl8")).expect("stn1");
    let two = l2_view::sheet::Sheet::new(read("T32_stn2.pl8")).expect("stn2");

    let mut field = l2_sim::siege::our_castle(4);
    let cam = scene::Camera { x: 30, y: 20 };
    let cell = (cam.y + 6) * DIM + (cam.x + 6);
    field.cells[cell].flags2 = 0;
    field.cells[cell].gfx = 0xAC;
    field.cells[cell].terrain = 1;

    let mut flat = Canvas::screen();
    field.cells[cell].elevation = 0;
    scene::draw_terrain(&mut flat, &field, &one, Some(&two), cam);

    let mut raised = Canvas::screen();
    field.cells[cell].elevation = 2;
    scene::draw_terrain(&mut raised, &field, &one, Some(&two), cam);

    let moved = flat.diff_count(&raised);
    assert!(moved > 0, "the overlay painted nothing");
    // Terrain 1 → frame 0x8C, and it is drawn over the cell and nowhere else.
    assert!(
        moved <= (scene::TILE * scene::TILE) as usize,
        "{moved} pixels moved for a one-cell overlay"
    );
    assert!(two.frame(0x8C).is_some(), "T32_stn2.pl8 carries the overlay frame");
}

/// **The keep flies the garrison's banner** — `BattleBanner_Draw`
/// (`FUN_004BD574`, `0x004BD574`), reached from the overlap pass
/// `FUN_004BD355` on cell byte `+2` bit `0x80` with the frame byte zero.
///
/// The gate closes on itself: `Battlefield_BuildCastle`'s code-6 arm is the
/// only writer of that bit on a castle, and frame byte 0 is the only byte
/// either structure table files under code 6 — so the cell the pass sends to
/// the banner is the keep and nothing else.
///
/// Two halves,
///
/// * `shield * 8 + counter + 0x21` over six shields and eight phases must land
///   inside `A2_miss.pl8` and end on its last frame, 80;
/// * the keep cell's pixels must move when the banner is passed, and nothing
///   else on the screen may.
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

    // It is one sprite over one cell: everything that moved must lie in the
    // 32-pixel box the blit is centred in, plus the sprite's own overhang.
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

    // And the eight phases are eight pictures, not one drawn eight times.
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
///
/// ```c
/// if (cell.elevation < 4) { cell.terrain++; if (0xF < cell.terrain) Wall_Collapse(cell); }
/// ```
///
/// **Cell byte `+0` is not a terrain id on a castle — it is the damage
/// counter**, and it is the *same byte* the overlay draws with at
/// `frame = cell[+0] + 0x8B` out of slot 1. `T32_stn2.pl8` frames `0x8C … 0x9A`
/// are one rubble pile growing from a speck to a full tile.
///
/// This crate kept the count in a `wall_hits` vector beside the field: the
/// arithmetic was right, the wall came down on the right shot, and **nothing
/// between the first shot and the last changed on screen**. A filling ditch
/// already animated, for the one reason that `fill_moat_tick` writes
/// `cell.terrain`.
///
/// Ablation, run: put the increment back in a side vector — red, *"the damage
/// overlay did not move"*.
#[test]
fn a_wall_under_the_catapult_breaks_up_in_the_picture() {
    let dir = l2_testkit::install!();
    let read = |n: &str| std::fs::read(dir.join(n)).expect("a sheet");
    let one = l2_view::sheet::Sheet::new(read("T32_stn1.pl8")).expect("stn1");
    let two = l2_view::sheet::Sheet::new(read("T32_stn2.pl8")).expect("stn2");

    use l2_sim::proving;
    let mut r = proving::deploy();
    // The proving castle's far wall stands at 4, which `Missile_Step` refuses
    // to count against. Three is the tallest a shot may chip.
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
    // The frame is the byte, so the picture walks with every further shot.
    assert!(
        two.frame(scene::OVERLAY_BASE + damaged as usize).is_some(),
        "T32_stn2.pl8 carries frame {:#X}",
        scene::OVERLAY_BASE + damaged as usize
    );
    eprintln!("wall damage: byte +0 {seed} -> {damaged}, {moved} pixels");
}

