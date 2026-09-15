#![allow(unused_imports)]
use super::*;
use super::castles::*;
use super::*;
use super::view_tests::*;
use super::interaction_tests::*;
use super::fog_and_march_tests::*;
use common::*;
use l2_game::game::Assets;
use l2_game::input::Event;
use l2_game::input::Key;
use l2_game::screen::Ctx;
use l2_game::screen::Machine;
use l2_game::screen::Screen;
use l2_game::screen::ScreenId;
use l2_game::screen::Transition;
use l2_game::screens::county::Panel;
use l2_game::screens::map;
use l2_game::screens::map::MapScreen;
use l2_game::shell::font;
use l2_game::Game;
use l2_view::campaign;
use l2_view::chrome;
use l2_view::Canvas;

/// It also asserts the thing that makes the count mean anything: **no palette
/// index appears in two rows**, so "which row is this pixel from" has one
/// answer. C59.
#[test]
fn the_minimap_paints_one_ramp_row_per_owning_realm() {
    let (mut game, assets) = world!();
    let county = (1..=game.kingdom.county_count as u8)
        .find(|&id| game.is_players(id))
        .expect("the player holds a county");
    game.select(county);

    let mut row_of_index = [None::<usize>; 256];
    for (row, shades) in chrome::MINIMAP_REALM_RAMP.iter().enumerate() {
        for &i in shades {
            assert_eq!(
                row_of_index[i as usize], None,
                "palette index {i:#04x} is in two ramp rows, so a pixel cannot name its realm"
            );
            row_of_index[i as usize] = Some(row);
        }
    }

    let tinted = draw(&mut MapScreen::new(), &mut game, &assets);
    let owners: Vec<u8> =
        game.kingdom.county_ids().map(|id| game.kingdom.counties[id].owner).collect();
    for id in game.kingdom.county_ids() {
        game.kingdom.counties[id].owner = 0;
    }
    let flat = draw(&mut MapScreen::new(), &mut game, &assets);
    for (id, owner) in game.kingdom.county_ids().zip(owners) {
        game.kingdom.counties[id].owner = owner;
    }

    let mut seen = std::collections::BTreeSet::new();
    let mut moved = 0usize;
    let mut selected_pixels = 0usize;
    for y in 0..chrome::MINIMAP_DIM {
        for x in 0..chrome::MINIMAP_DIM {
            let (px, py) = ((chrome::MINIMAP_X + x) as usize, (chrome::MINIMAP_Y + y) as usize);
            let (a, b) = (tinted.at(px, py), flat.at(px, py));
            if a == chrome::MINIMAP_SELECTED {
                selected_pixels += 1;
            }
            if a == b {
                continue;
            }
            moved += 1;
            assert_eq!(
                row_of_index[b as usize],
                Some(0),
                "an unowned county must paint the raster's own shading, ramp row 0, \
                 and this pixel painted {b:#04x}"
            );
            let row = row_of_index[a as usize].unwrap_or_else(|| {
                panic!("owned land painted {a:#04x}, which is in no ramp row at all")
            });
            seen.insert(row);
        }
    }

    let mut wanted = std::collections::BTreeSet::new();
    for id in game.kingdom.county_ids() {
        let owner = game.kingdom.counties[id].owner as usize;
        if owner != 0 {
            wanted.insert(chrome::realm_colour(game.realm_colour[owner]) as usize);
        }
    }
    assert!(
        wanted.len() >= 2,
        "this fixture has {} owning colour(s), so it could not tell a tinted minimap \
         from an untinted one even if the tint were gone",
        wanted.len()
    );

    assert_eq!(
        seen, wanted,
        "the minimap's ramp rows must be exactly the ones the counties' owners ask for; \
         **one single row** is the shape of the failure to watch for — `realm_colour` \
         clamps a zero colour up to 1, so a tint that has lost its input does not go \
         blank, it goes uniformly red"
    );
    assert!(
        moved > 500,
        "only {moved} pixels changed when five counties changed hands, which is too few \
         to be five counties"
    );
    assert!(
        selected_pixels > 0,
        "the selected county's brightest shade is replaced by MINIMAP_SELECTED, and none was drawn"
    );
}


/// `Gfx_LoadCountyMode` (`0x004984DC`) repoints all five near-zoom tile banks
/// at `(g_season - 1) * 8` in `g_resourceTable`, so the whole picture is
/// redrawn from different files. We hard-coded the `a` set and the map looked
/// the same in January and in August.
#[test]
fn a_real_turn_turns_the_season_and_the_map_is_repainted_from_other_files() {
    let (mut game, assets) = world!();
    let mut screen = MapScreen::new();
    let before_season = game.kingdom.season;
    let before = draw(&mut screen, &mut game, &assets);

    l2_game::turn::end_turn(&mut game).expect("the turn completes without asking");
    let after_season = game.kingdom.season;
    assert_ne!(after_season, before_season, "one turn is one season");

    let after = draw(&mut screen, &mut game, &assets);
    let moved = before.diff_count(&after);

    assert!(
        moved > 50_000,
        "the season turned from {before_season} to {after_season} and only {moved} pixels moved"
    );

    let histogram = |c: &Canvas| {
        let mut h = [0u32; 256];
        for &p in &c.pixels {
            h[p as usize] += 1;
        }
        h
    };
    let (a, b) = (histogram(&before), histogram(&after));
    let differing = (0..256).filter(|&i| a[i].abs_diff(b[i]) > 200).count();
    assert!(differing > 8, "only {differing} palette entries changed their share of the frame");
}

#[test]
fn a_towns_overridden_graphic_survives_every_season() {
    let (mut game, assets) = world!();
    let mut screen = MapScreen::new();
    let tile = {
        let ctx = Ctx { game: &mut game, assets: &assets };
        *MapScreen::town(&ctx, 8).first().expect("county 8 has a town")
    };
    let (tx, ty) = l2_kingdom::map::coords(tile);
    screen.centre_on_tile(tx as usize, ty as usize);

    let slot = assets.slot(game.map_slot).expect("the map slot");
    let lattice = campaign::Lattice::build(&slot);
    let overrides = {
        let ctx = Ctx { game: &mut game, assets: &assets };
        MapScreen::tile_graphics(&ctx)
    };

    let paint = |season: u8, o: &campaign::Overrides| {
        let mut canvas = Canvas::screen();
        let mut tags = l2_view::Tags::screen();
        campaign::draw(
            &mut canvas,
            &slot,
            &lattice,
            &assets.map,
            screen.viewport(),
            screen.zoom(),
            &mut tags,
            o,
            season,
            None,
        );
        canvas
    };
    let bare = campaign::Overrides::new();
    let mut seasons_that_differ = 0;
    for season in 1..=campaign::SEASONS as u8 {
        let with = paint(season, &overrides);
        let without = paint(season, &bare);
        let moved = with.diff_count(&without);
        assert!(
            moved > 500,
            "season {season}: the override changed only {moved} pixels, so the town is not \
             being restamped"
        );
        seasons_that_differ += 1;
    }
    assert_eq!(seasons_that_differ, 4, "all four seasons keep their towns");
}

/// `docs/arms.json` `0x0042FF10/minimap-closes-the-surface`.
#[test]
fn a_press_on_the_minimap_drops_whatever_is_open_over_the_map() {
    let (mut game, assets) = world!();
    game.select(8);
    let hit = chrome::minimap_hit_area();
    let (mx, my) = (hit.x0 + 40, hit.y0 + 40);

    for over in [
        ScreenId::County(8, Panel::Tax),
        ScreenId::Job(8, 0),
        ScreenId::Court,
        ScreenId::Ratings,
        ScreenId::Supplies(8),
    ] {
        let mut m = over_the_map(over);
        send_stack(&mut m, &mut game, &assets, Event::Click { x: mx, y: my });
        assert_eq!(m.top_id(), Some(ScreenId::Campaign), "{over:?} gave way to the minimap");
        assert_eq!(m.depth(), 1, "{over:?}: the map is revealed, not rebuilt");
    }

    // The ablation: a press just OUTSIDE the raster leaves everything alone. An
    // unconditional `Pass` would close on both.
    let mut m = over_the_map(ScreenId::Job(8, 0));
    send_stack(&mut m, &mut game, &assets, Event::Click { x: hit.x1 + 4, y: my });
    assert_eq!(m.top_id(), Some(ScreenId::Job(8, 0)), "outside the raster nothing happens");
}



