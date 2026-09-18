#![allow(unused_imports)]
use super::*;
use super::terrain_part::*;
use super::ui::*;
use super::sprites::*;
use super::*;
use super::corpus::*;
use super::oracle::*;
use std::{fs, path::{Path, PathBuf}};
use l2_sim::runner::{self as battle, BattleRunner};
use l2_sim::terrain;
use l2_sim::{Troop, SIDE_A, SIDE_B};
use l2_view::campaign;
use l2_view::canvas::Canvas;
use l2_view::chrome;
use l2_view::figures::{self, Anim, Colour};
use l2_view::scene::{self, BattleAssets, Camera};
use l2_view::sheet::Sheet;

#[test]
fn a_battle_on_the_sample_map_animates_rather_than_sitting_still() {
    let Some(dir) = asset_dir() else {
        l2_testkit::skip!("LORDS2_DIR not set - skipping");
    };
    let Some(skr_bytes) = read(&dir, "USER.SKR") else {
        l2_testkit::skip!("USER.SKR not present - skipping");
    };
    let assets = load_assets(&dir);
    let skr = l2_formats::Skr::parse(&skr_bytes).unwrap();
    let field = terrain::build(skr.terrain(0).unwrap(), 1);

    let att = skr.army(0, l2_formats::Side::Attacker).unwrap().counts;
    let def = skr.army(0, l2_formats::Side::Defender).unwrap().counts;
    assert_eq!(att[0], 200, "map 0's attacker should be the sample army");
    let army_a = battle::army_from_counts(&att, 16);
    let army_b = battle::army_from_counts(&def, 16);

    let raised = |army: &[(Troop, u16)]| -> usize { army.iter().map(|(_, n)| *n as usize).sum() };
    assert_eq!(raised(&army_a), 41, "attacker figures");
    assert_eq!(raised(&army_b), 33, "defender figures");

    let mut runner = BattleRunner::deploy(field, &army_a, &army_b);
    assert_eq!(runner.fighters.len(), 74, "USER.SKR map 0 should raise 74 figures");
    assert!(runner.units.live().count() >= 8, "the armies did not become units");

    let enemy = runner.home(SIDE_B);
    runner.order_side(SIDE_A, enemy.0, enemy.1);

    let mut shots: Vec<Canvas> = Vec::new();
    for frame in 0..6 {
        if frame > 0 {
            runner.run(60);
        }
        let cam = scene::follow(&runner);
        let mut canvas = Canvas::screen();
        let drawn = scene::draw(&mut canvas, &runner, &assets, scene::Ground::Field, cam, None);
        assert!(drawn > 0, "frame {frame} drew no figures at all");
        shots.push(canvas);
    }

    for w in shots.windows(2) {
        let d = w[0].diff_count(&w[1]);
        assert!(d > 200, "only {d} pixels changed between frames");
    }

    let cam = scene::follow(&runner);
    let mut a = Canvas::screen();
    let mut b = Canvas::screen();
    scene::draw(&mut a, &runner, &assets, scene::Ground::Field, cam, None);
    scene::draw(&mut b, &runner, &assets, scene::Ground::Field, cam, None);
    assert_eq!(a.diff_count(&b), 0, "the renderer is not deterministic");
    eprintln!(
        "battle: {} figures, {} ticks, {} alive",
        runner.fighters.len(),
        runner.tick,
        runner.fighters.iter().enumerate().filter(|(i, _)| runner.is_alive(*i)).count()
    );
}

#[test]
fn figures_are_visible_against_the_terrain_behind_them() {
    let Some(dir) = asset_dir() else {
        l2_testkit::skip!("LORDS2_DIR not set - skipping");
    };
    let assets = load_assets(&dir);
    let runner = BattleRunner::deploy(
        battle::blank_field(),
        &[(Troop::Knights, 6)],
        &[(Troop::Swordsmen, 6)],
    );
    let cam = scene::follow(&runner);

    let mut terrain_only = Canvas::screen();
    scene::draw_terrain(&mut terrain_only, &runner.field, assets.tiles(), None, cam);

    let mut with_men = terrain_only.clone();
    let drawn = scene::draw_figures(&mut with_men, &runner, &assets, cam);
    assert!(drawn > 0, "no figures drawn");

    let changed = terrain_only.diff_count(&with_men);
    assert!(changed > 500, "figures only changed {changed} pixels");
    eprintln!("figures: {drawn} drawn, {changed} pixels over the terrain");
}

#[test]
fn a_click_on_the_drawn_army_resolves_to_the_tile_it_stands_on() {
    let Some(dir) = asset_dir() else {
        eprintln!("skipping: no install");
        return;
    };
    let bytes = read(&dir, "Sprite1a.pl8").expect("Sprite1a.pl8");
    let sheet = Sheet::new(bytes).expect("parse");
    let frame = sheet.frame(0).expect("frame 0");
    assert_eq!(
        (frame.width, frame.height),
        (40, 32),
        "the near-zoom figure is 40 x 32, which is the number the 9 x 9 hit box was up against",
    );

    let zoom = &campaign::NEAR;
    let (nx, ny) = (0i32, -4i32);
    let origin_x = zoom.half_pitch + nx - frame.width as i32 / 2;
    let origin_y = zoom.half_pitch + ny - frame.height as i32;

    let (hw, hh) = (zoom.half_pitch, zoom.row_step);
    let (cx, cy) = (zoom.half_pitch, zoom.tile_h / 2);

    let (mut on_tile, mut off_tile) = (0usize, 0usize);
    for row in 0..frame.height as i32 {
        for col in 0..frame.width as i32 {
            let i = (row * frame.width as i32 + col) as usize;
            if !frame.opaque[i] {
                continue;
            }
            let (px, py) = (origin_x + col, origin_y + row);
            if (px - cx).abs() * hh + (py - cy).abs() * hw <= hw * hh {
                on_tile += 1;
            } else {
                off_tile += 1;
            }
        }
    }
    let total = on_tile + off_tile;
    assert!(total > 0, "the frame has pixels in it");
    assert!(
        on_tile * 2 > total,
        "most of the drawn figure has to pick its own tile: {on_tile} of {total}",
    );
    assert!(
        on_tile > 81,
        "and more of it than the 9 x 9 box that was there before could ever hold: {on_tile}",
    );
    eprintln!("army sprite {}x{}: {on_tile} of {total} opaque pixels pick their own tile", frame.width, frame.height);
}

