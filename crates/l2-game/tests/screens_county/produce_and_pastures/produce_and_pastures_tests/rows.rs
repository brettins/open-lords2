#![allow(unused_imports)]
use super::*;
use super::sovereign::*;
use super::super::*;
use super::super::*;
use super::super::strip_and_sidebar::*;
use super::super::panels::*;
use super::super::drawing_and_emboss::*;
use super::super::layout_and_labels::*;
use common::*;
use l2_game::game::Assets;
use l2_game::game::MAX_TAX_RATE;
use l2_game::input::Event;
use l2_game::input::Key;
use l2_game::screen::Ctx;
use l2_game::screen::Machine;
use l2_game::screen::Screen;
use l2_game::screen::ScreenId;
use l2_game::screen::Transition;
use l2_game::screens::county::{self as county};
use l2_game::screens::county::CountyScreen;
use l2_game::screens::county::Panel;
use l2_game::screens::map;
use l2_game::screens::map::MapScreen;
use l2_game::screens::village::{self as village_screen};
use l2_game::screens::village::VillageScreen;
use l2_game::shell::font;
use l2_game::Game;
use l2_view::campaign;
use l2_view::Canvas;

#[test]
fn the_produce_rows_map_to_labour_slots_by_column_and_pitch() {
    let mut c = l2_kingdom::county::County::new();
    assert!(county::farm_rows(&c).is_empty());
    assert_eq!(county::job_row_at(&c, 500, 0x140), None, "an empty column refuses");

    // Cattle, then grain, then reclamation - `FUN_0040FEC1`'s own order, which
    // is not the labour slots' order.
    c.fields_cattle = 1;
    c.fields_grain = 1;
    assert_eq!(county::farm_rows(&c), vec![1, 0]);
    assert_eq!(county::farm_pitch(2), 0x3C, "two rows keep the tall pitch");
    assert_eq!(county::job_row_at(&c, 500, 0x12E), Some(1), "row 0 is the dairy");
    assert_eq!(county::job_row_at(&c, 500, 0x12E + 0x3C), Some(0), "row 1 is grain");
    assert_eq!(county::job_row_at(&c, 500, 0x12E + 0x78), None, "row 2 is past the end");

    c.fields_reclaiming = 1;
    assert_eq!(county::farm_rows(&c), vec![1, 0, 2]);
    assert_eq!(county::farm_pitch(3), 0x2D, "three rows close up");
    assert_eq!(county::job_row_at(&c, 500, 0x12E + 0x5A), Some(2), "reclamation, tight pitch");

    for i in &mut c.industry {
        i.enabled = false;
    }
    c.industry[0].enabled = true;
    c.industry[1].enabled = true;
    assert_eq!(county::industry_rows(&c), vec![6, 4], "wood then iron, not the array order");
    assert_eq!(county::job_row_at(&c, 0x230, 0x12E), Some(6), "x 0x230 is the industry column");
    assert_eq!(county::job_row_at(&c, 0x22F, 0x12E), Some(1), "and 0x22F is still the farm one");
    c.industry[3].enabled = true;
    c.industry[2].enabled = true;
    c.castle_degraded = 1;
    assert_eq!(county::industry_rows(&c), vec![6, 4, 5, 7, 3], "five rows, the castle last");
    assert_eq!(county::industry_pitch(5), 0x1E, "five rows get the 30-pixel pitch");
    assert_eq!(county::industry_pitch(3), 0x2D);
    assert_eq!(county::industry_pitch(2), 0x3C);

    assert_eq!(county::job_row_at(&c, 500, 0x12D), None, "one row above the plate");
    assert_eq!(county::job_row_at(&c, 500, 0x1AE), None, "one row below it");
    assert_eq!(county::job_row_at(&c, 477, 0x140), None, "one column left of the sidebar");
}

/// A player with the build in front of him: *"why do the pastures not have cows
/// in them?"* Because `FUN_004071A0`'s overlay pass had three of its four arms
/// and not the farm one.
#[test]
fn the_pastures_have_cattle_in_them_and_the_herd_chooses_which() {
    let (mut game, assets) = world!();

    // A real pasture of the England position, and the county that grazes it —
    // **one no AI lord re-lays at the head of the season.** `Season_Advance`
    // (`0x00448440`) opens with `Ai_ManageFarmsAll` (`0x0049A990`), which runs
    // every AI realm's farming style, and on this Winter save the arable and
    // mixed styles clear the fields and sow grain over them. That is the
    // original's behaviour and it would put grain on the tile claim 3 watches,
    // which says nothing about the herd. A person's county or a lordless one is
    // not touched by that pass, so the tile is left to `Herd_UpdateCrowding`,
    // which is what claim 3 is about.
    let (county, tile) = {
        let k = &game.kingdom;
        (1..=k.county_count)
            .filter(|&id| {
                let owner = k.counties[id].owner;
                owner == 0 || k.owner_is_human(owner)
            })
            .find_map(|id| {
                (0..l2_kingdom::MAX_FIELDS)
                    .filter_map(|s| k.counties[id].field_tile(s))
                    .find(|&t| {
                        l2_kingdom::field::classify(k.campaign.map.terrain[t])
                            == l2_kingdom::field::FieldType::Pasture
                    })
                    .map(|t| (id, t))
            })
            .expect("the England position has pastures")
    };
    let terrain = game.kingdom.campaign.map.terrain[tile];
    assert!(
        (0x14..=0x16).contains(&terrain),
        "the original's own save already carries a stocked pasture here, not {terrain:#04X}",
    );

    let (fx, fy) = l2_kingdom::map::coords(tile);
    let mut screen = MapScreen::new();
    screen.centre_on_tile(fx as usize, fy as usize);
    let (frame_index, (dx, dy)) =
        campaign::herd_sprite(terrain, 0).expect("a stocked pasture draws animals");

    // The first version of this test computed the probe pixel *from*
    // `campaign::HERD_AT` and then asserted the sprite was there — so setting
    // that constant to `(0, 0)` moved both sides of the comparison and the test
    // stayed green. A check passing for an accidental reason, caught by
    // ablating the exact line it claims to be about. `FUN_004071A0`'s farm arm
    // is `local_30 = 4; local_34 = -4;` for content `0x14 … 0x16`, and that
    // literal has to be written here or nothing anchors it.
    assert_eq!((dx, dy), (4, -4), "FUN_004071A0 offsets a stocked pasture by (+4, -4)");
    let sheet = assets.map.flag_sheet(screen.zoom()).expect("Flags1a.pl8");
    let sprite = sheet.frame(frame_index).expect("the herd frame");

    let (row, col) = campaign::tile_to_cell(fx as usize, fy as usize);
    let (sx, sy) = campaign::cell_to_screen(screen.viewport(), screen.zoom(), row, col);
    let (ox, oy) = (sx + dx, sy + dy);

    let probe = (0..sprite.opaque.len())
        .find(|&i| {
            let (px, py) = (i % sprite.width as usize, i / sprite.width as usize);
            sprite.opaque[i]
                && px > 2
                && py > 2
                && px + 3 < sprite.width as usize
                && py + 3 < sprite.height as usize
        })
        .expect("the herd sprite has an interior");
    let (px, py) = (probe % sprite.width as usize, probe / sprite.width as usize);
    let want = sprite.indices[probe];
    let (tx, ty) = ((ox + px as i32) as usize, (oy + py as i32) as usize);

    let mut with = Canvas::screen();
    {
        let ctx = Ctx { game: &mut game, assets: &assets };
        screen.draw(&ctx, &mut with);
    }
    assert_eq!(
        with.at(tx, ty),
        want,
        "the herd sprite is not at the tile origin plus {:?}",
        (dx, dy),
    );

    let mut without = Canvas::screen();
    {
        let slot = assets.slot(game.map_slot).expect("the map slot");
        let lattice = campaign::Lattice::build(&slot);
        let overrides = {
            let ctx = Ctx { game: &mut game, assets: &assets };
            MapScreen::tile_graphics(&ctx)
        };
        let mut tags = l2_view::Tags::screen();
        campaign::draw(
            &mut without,
            &slot,
            &lattice,
            &assets.map,
            screen.viewport(),
            screen.zoom(),
            &mut tags,
            &overrides,
            game.kingdom.season,
            None,
        );
    }
    assert_ne!(
        without.at(tx, ty),
        want,
        "the bare meadow already had this pixel, so the assertion above measures the artwork",
    );

    game.kingdom.counties[county].herd = 0;
    game.kingdom.advance_season();
    assert_eq!(
        game.kingdom.campaign.map.terrain[tile],
        l2_kingdom::field::terrain::PASTURE,
        "an empty herd leaves bare pasture",
    );
    assert!(
        campaign::herd_sprite(l2_kingdom::field::terrain::PASTURE, 0).is_none(),
        "and nothing is drawn on it",
    );
    let mut empty = Canvas::screen();
    {
        let ctx = Ctx { game: &mut game, assets: &assets };
        screen.draw(&ctx, &mut empty);
    }
    assert_ne!(empty.at(tx, ty), want, "the cattle are still on the map with no herd to draw");
}

#[test]
fn the_grazing_clock_changes_the_picture_and_cannot_change_the_world() {
    let (mut game, assets) = world!();
    let tile = {
        let k = &game.kingdom;
        (1..=k.county_count)
            .find_map(|id| {
                (0..l2_kingdom::MAX_FIELDS)
                    .filter_map(|s| k.counties[id].field_tile(s))
                    .find(|&t| (0x14..=0x16).contains(&k.campaign.map.terrain[t]))
            })
            .expect("a stocked pasture")
    };
    let (fx, fy) = l2_kingdom::map::coords(tile);
    let terrain = game.kingdom.campaign.map.terrain[tile];

    let frames: Vec<usize> = (0..campaign::HERD_PHASES)
        .map(|p| campaign::herd_sprite(terrain, p).expect("stocked").0)
        .collect();
    let mut distinct = frames.clone();
    distinct.sort_unstable();
    distinct.dedup();
    assert_eq!(distinct.len(), 6, "the six phases are six frames: {frames:?}");
    assert_eq!(
        campaign::herd_sprite(terrain, campaign::HERD_PHASES).expect("stocked").0,
        frames[0],
        "and the seventh wraps to the first",
    );

    let mut screen = MapScreen::new();
    screen.centre_on_tile(fx as usize, fy as usize);
    if let Some(sheet) = assets.map.flag_sheet(screen.zoom()) {
        let mut shapes: Vec<Vec<u8>> = frames
            .iter()
            .map(|&f| sheet.frame(f).expect("the frame").indices.clone())
            .collect();
        let before = shapes.len();
        shapes.sort();
        shapes.dedup();
        assert_eq!(shapes.len(), before, "the six phases are one picture repeated");
    }

    let before = l2_kingdom::save::checksum(&game.kingdom);
    for _ in 0..campaign::HERD_PHASES {
        let mut canvas = Canvas::screen();
        let ctx = Ctx { game: &mut game, assets: &assets };
        screen.draw(&ctx, &mut canvas);
    }
    assert_eq!(
        l2_kingdom::save::checksum(&game.kingdom),
        before,
        "drawing the map moved the simulation",
    );
}

