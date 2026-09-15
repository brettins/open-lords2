#![allow(unused_imports)]
use super::*;
use super::view_and_navigation_tests::*;
use super::selection_and_click_tests::*;
use super::chrome_tests::*;
use super::*;
use super::view_tests::*;
use super::structures_tests::*;
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

#[test]
fn every_county_town_is_re_stamped_off_the_quarry_frames_and_onto_a_village() {
    let (mut game, assets) = world!();
    let ctx = Ctx { game: &mut game, assets: &assets };
    let overrides = MapScreen::town_graphics(&ctx);
    assert!(!overrides.is_empty(), "fourteen towns were rewritten");

    let mut towns = 0;
    for id in ctx.game.kingdom.county_ids() {
        let tiles = MapScreen::town(&ctx, id as u8);
        assert_eq!(tiles.len(), 4, "county {id}'s town is a 2 x 2 block");
        let pop = ctx.game.kingdom.counties[id].population;
        let base: u8 = if pop < 801 {
            47
        } else if pop < 1201 {
            51
        } else {
            55
        };
        let mut frames = Vec::new();
        for tile in tiles {
            let (x, y) = l2_kingdom::map::coords(tile);
            let (bank, frame) = overrides.get(x as usize, y as usize).expect("a town tile");
            assert_eq!(bank, 0x0C, "the town stays in the Town1a bank");
            assert!(
                (base..base + 4).contains(&frame),
                "county {id} has {pop} people, so its town is frames {base}..{}; got {frame}",
                base + 4
            );
            frames.push(frame);
        }
        frames.sort_unstable();
        assert_eq!(frames, vec![base, base + 1, base + 2, base + 3], "one of each quadrant");
        towns += 1;
    }
    assert_eq!(towns, 14, "England has fourteen counties and fourteen towns");
}

#[test]
fn the_rewritten_town_actually_changes_what_is_drawn() {
    let (mut game, assets) = world!();
    let mut screen = MapScreen::new();
    let ctx = Ctx { game: &mut game, assets: &assets };
    let tile = *MapScreen::town(&ctx, 8).first().expect("county 8 has a town");
    let (tx, ty) = l2_kingdom::map::coords(tile);
    screen.centre_on_tile(tx as usize, ty as usize);

    let ctx = Ctx { game: &mut game, assets: &assets };
    let overrides = MapScreen::town_graphics(&ctx);
    let slot = assets.slot(game.map_slot).expect("the map slot");
    let lattice = l2_view::campaign::Lattice::build(&slot);
    let paint = |o: &l2_view::campaign::Overrides| {
        let mut canvas = Canvas::screen();
        let mut tags = l2_view::Tags::screen();
        l2_view::campaign::draw(
            &mut canvas,
            &slot,
            &lattice,
            &assets.map,
            screen.viewport(),
            screen.zoom(),
            &mut tags,
            o,
            game.kingdom.season,
            None,
        );
        canvas
    };
    let with = paint(&overrides);
    let without = paint(&l2_view::campaign::Overrides::new());

    let moved = with.diff_count(&without);
    assert!(moved > 500, "the town's tiles are painted from different frames: {moved} pixels");
    assert!(moved < 30_000, "and only the towns changed, not the whole viewport: {moved}");
}

#[test]
fn every_painted_pixel_of_a_mine_reaches_the_industry_toggle() {
    let (mut game, assets) = world!();
    let county = game
        .kingdom
        .county_ids()
        .find(|&id| game.is_players(id as u8) && game.kingdom.counties[id].industry[1].has_resource)
        .expect("the player starts with a mine somewhere");
    let ctx = Ctx { game: &mut game, assets: &assets };
    let tile = MapScreen::settlements_for_test(&ctx, county as u8)
        .into_iter()
        .find(|&t| ctx.game.kingdom.campaign.map.terrain[t] == 1)
        .expect("and that county has an iron site on the map");
    let (tx, ty) = l2_kingdom::map::coords(tile);

    let mut screen = MapScreen::new();
    game.select(county as u8);
    screen.centre_on_tile(tx as usize, ty as usize);
    draw(&mut screen, &mut game, &assets);

    let (row, col) = campaign::tile_to_cell(tx as usize, ty as usize);
    let (sx, sy) = campaign::cell_to_screen(screen.viewport(), screen.zoom(), row, col);
    let slot = assets.slot(game.map_slot).expect("the map slot");
    let frame = slot.at(l2_formats::maps::Plane::GfxIndex, tx as usize, ty as usize) as usize;
    let sheet = assets.map.bank(screen.zoom(), game.kingdom.season, 3).expect("the Town bank");
    let art = sheet.frame(frame).expect("the mine's frame");
    let overhang = art.height as i32 - screen.zoom().tile_h;
    assert!(overhang > 0, "the mine overhangs its tile; without that this test proves nothing");

    let mut painted = 0;
    let mut reached = 0;
    let mut off_the_art_and_off_the_tile = 0;
    for dy in 0..art.height as i32 {
        for dx in 0..art.width as i32 {
            let opaque = art.opaque[dy as usize * art.width as usize + dx as usize];
            let before = game.kingdom.counties[county].industry[1].enabled;
            let (x, y) = (sx + dx, sy - overhang + dy);
            send(&mut screen, &mut game, &assets, Event::Click { x, y });
            let toggled = game.kingdom.counties[county].industry[1].enabled != before;
            if opaque {
                painted += 1;
                reached += usize::from(toggled);
            } else if toggled && dy < overhang {
                off_the_art_and_off_the_tile += 1;
            }
        }
    }
    assert!(painted > 1_000, "the mine is a building, not a smudge: {painted} pixels");
    assert_eq!(reached, painted, "every painted pixel of the mine must switch it");
    assert_eq!(
        off_the_art_and_off_the_tile, 0,
        "the fallback is the frame's opacity mask, not its bounding box"
    );
}

