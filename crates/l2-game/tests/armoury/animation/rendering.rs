#![allow(unused_imports)]
use super::*;
use super::sheet_validation::*;
use super::walker_logic::*;
use super::*;
use super::hit_map::*;
use super::rack::*;
use super::screenshots::*;
use std::path::PathBuf;
use l2_game::game::Assets;
use l2_game::input::{Event, Rect};
use l2_game::scenario;
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::armoury;
use l2_game::Game;
use l2_kingdom::tables::{Tables, WEAPON_TYPE_COUNT};
use l2_view::Canvas;

#[test]
fn the_erase_strip_is_the_original_s_four_and_the_soldier_overhangs_it() {
    let (_g, assets) = world!();

    for &(x, w) in &armoury::STRIPS {
        let r = armoury::walker_strip(x);
        assert_eq!((r.x, r.w, r.y, r.h), (x, w, armoury::WALKER_Y, armoury::STRIP_H));
    }
    assert_eq!(armoury::walker_strip(armoury::WALKER_START_X).x, 0, "he starts in the first");
    assert_eq!(armoury::walker_strip(0x9F).x, 0);
    assert_eq!(armoury::walker_strip(0xA0).x, 0xA0);
    assert_eq!(armoury::walker_strip(0x13F).x, 0xA0);
    assert_eq!(armoury::walker_strip(0x140).x, 0x140);
    assert_eq!(armoury::walker_strip(0x1DF).x, 0x140);
    assert_eq!(armoury::walker_strip(0x1E0).x, 0x1E0);
    assert_eq!(armoury::walker_strip(0x27F).x, 0x1E0);

    let sheet = assets.shell.sheet(armoury::walker_sheet(1, 1)).expect("Trp_xb_r.pl8");
    let mut widest = 0;
    for f in 0..=armoury::CARRY_FIRST + armoury::WALK_PHASES as usize - 1 {
        let frame = sheet.frame(f).expect("a walk frame");
        for (i, &o) in frame.opaque.iter().enumerate() {
            if o {
                widest = widest.max(i % frame.width as usize + 1);
            }
        }
    }
    assert!(widest > 0, "the sheet decoded to nothing");

    let last = 0xA0 - armoury::WALKER_STEP;
    let strip = armoury::walker_strip(last);
    let over = (last + widest as i32) - (strip.x + strip.w);
    assert!(
        over > 0,
        "he was expected to overhang the strip he is erased with; widest opaque \
         column {widest}, overhang {over}",
    );
    assert!(over < 16, "and only by a sliver, not by half a man: {over}");
}

#[test]
fn the_soldier_and_the_torches_are_painted_where_the_animation_says() {
    let (mut g, a) = world!();
    let county = own_county(&g);
    g.selected = county;
    g.open_levy(county);

    let realm = g.player as usize;
    let shield = g.kingdom.realms[realm].shield_index;
    let slot = (0..WEAPON_TYPE_COUNT)
        .find(|&s| g.kingdom.realms[realm].weapons[s] > 0)
        .map(|s| s as u8 + 1)
        .expect("the fixture's realm has an armoury");

    g.levy.anim.walker.latch(slot, 0);
    assert!(g.levy.anim.walker.start(slot, 1));
    for _ in 0..200 {
        g.levy.anim.tick();
    }
    let w = g.levy.anim.walker;
    let torch_phase = g.levy.anim.torch;
    assert!(w.active && w.x > 0, "he is on the floor at x {}", w.x);
    assert!(torch_phase > 0, "and the torches have moved");

    let expect = |c: &Canvas, sheet: &str, index: usize, ox: i32, oy: i32, what: &str| {
        let f = a.shell.sheet(sheet).expect("the sheet").frame(index).expect("the frame");
        let (mut hits, mut misses) = (0usize, 0usize);
        for y in 0..f.height as i32 {
            for x in 0..f.width as i32 {
                let i = y as usize * f.width as usize + x as usize;
                if !f.opaque[i] {
                    continue;
                }
                let (px, py) = (ox + x, oy + y);
                if px < 0 || py < 0 || px >= 640 || py >= 480 {
                    continue;
                }
                if c.at(px as usize, py as usize) == f.indices[i] {
                    hits += 1;
                } else {
                    misses += 1;
                }
            }
        }
        assert!(hits > 200, "{what}: only {hits} of its pixels are on the canvas");
        assert_eq!(misses, 0, "{what}: {misses} pixels differ from {sheet} frame {index}");
    };

    for (screen, what) in [
        (ScreenId::Armoury(county), "the armoury"),
        (ScreenId::Rack(county, slot), "the rack panel over it"),
    ] {
        let mut m = Machine::new(ScreenId::Armoury(county));
        if screen != ScreenId::Armoury(county) {
            m.push(screen);
        }
        assert_eq!(m.top_id(), Some(screen));
        let c = frame(&mut m, &mut g, &a);
        expect(
            &c,
            armoury::walker_sheet(shield, w.slot),
            w.frame,
            w.x,
            armoury::WALKER_Y,
            &format!("{what}: the walking soldier"),
        );
        for (i, &(tx, ty)) in armoury::TORCH_AT.iter().enumerate() {
            expect(
                &c,
                armoury::TORCH_SHEET,
                torch_phase as usize + i * armoury::TORCH_SECOND,
                tx,
                ty,
                &format!("{what}: torch {i}"),
            );
        }

        // And the weapon turning in the panel's own well, third
        // animation and the one that was drawing frame 0 for ever. **This was
        // added because ablating it found nothing**: every assertion above is
        // about the room, and the well is on the panel.
        if screen != ScreenId::Armoury(county) {
            expect(
                &c,
                armoury::WEAPON_SHEETS[slot as usize - 1],
                g.levy.anim.weapon as usize,
                armoury::WEAPON_AT.0,
                armoury::WEAPON_AT.1,
                "the rack panel: the weapon in the well",
            );
        }
    }
}

#[test]
fn a_hundred_ticks_of_the_armoury_leave_the_kingdom_byte_identical() {
    let (mut g, a) = world!();
    let county = own_county(&g);
    g.selected = county;
    g.open_levy(county);

    let mut m = Machine::new(ScreenId::Armoury(county));
    g.levy.anim.walker.latch(1, 0);
    g.levy.anim.walker.start(1, 1);

    let before = l2_kingdom::save::encode(&g.kingdom);
    for _ in 0..100 {
        let mut ctx = Ctx { game: &mut g, assets: &a };
        m.update(&mut ctx);
    }
    assert!(g.levy.anim.walker.x > armoury::WALKER_START_X, "the soldier moved");
    assert_eq!(before, l2_kingdom::save::encode(&g.kingdom), "and the world did not");
}



