#![allow(unused_imports)]
use super::*;
use super::town::*;
use super::super::*;
use super::super::*;
use super::super::interaction_tests::*;
use super::super::structures_tests::*;
use super::super::fog_and_march_tests::*;
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

/// * **The offset.** `(+0x10, −0x12)` at the near zoom, not the banner's
///   `(+0x1A, −0x1C)`. Both literals below are read out of the decompilation and
///   **no expression in this test mentions `Zoom::mercenary_at`**, which is
///   `docs/agents.md`'s first way to ablate wrongly.
#[test]
fn a_mercenary_band_stands_on_the_town_blocks_third_tile_and_nowhere_else() {
    let (mut game, assets) = world!();
    for c in game.kingdom.counties.iter_mut() {
        c.mercenary_offer = 0;
    }
    let county = (1..=game.kingdom.county_count as u8)
        .find(|&id| game.is_players(id))
        .expect("the player holds a county");
    let mut screen = MapScreen::new();
    draw(&mut screen, &mut game, &assets);
    let town: Vec<usize> = {
        let ctx = Ctx { game: &mut game, assets: &assets };
        MapScreen::town(&ctx, county)
    };
    assert_eq!(town.len(), 4, "a county town is a 2 x 2 block");
    let (ox, oy) = l2_kingdom::map::coords(town[0]);
    screen.centre_on_tile(ox as usize, oy as usize);
    draw(&mut screen, &mut game, &assets);
    let without = draw(&mut screen, &mut game, &assets);
    assert_eq!(
        l2_kingdom::map::coords(town[2]),
        (ox, oy + 1),
        "index order puts part 2 - (x, y + 1) - third",
    );

    game.kingdom.counties[county as usize].mercenary_offer = 3;
    let with = draw(&mut screen, &mut game, &assets);

    let (row, col) = campaign::tile_to_cell(ox as usize, oy as usize + 1);
    let (sx, sy) = campaign::cell_to_screen(screen.viewport(), screen.zoom(), row, col);
    let mut reference = without.clone();
    let frame = assets
        .map
        .flag_sheet(screen.zoom())
        .and_then(|s| s.frame(campaign::MERCENARY_MARKER_FRAME))
        .expect("Flags1a.pl8 frame 0x81");
    reference.blit_clipped(&frame, sx + 0x10, sy - 0x12, screen.map_clip());

    let moved = |a: &Canvas, b: &Canvas| -> Vec<usize> {
        a.pixels
            .iter()
            .zip(b.pixels.iter())
            .enumerate()
            .filter(|(_, (p, q))| p != q)
            .map(|(i, _)| i)
            .collect()
    };
    let corner = |v: &[usize]| -> (usize, usize, usize, usize, usize) {
        let xs = v.iter().map(|i| i % with.width);
        let ys = v.iter().map(|i| i / with.width);
        (
            xs.clone().min().unwrap_or(0),
            ys.clone().min().unwrap_or(0),
            xs.max().unwrap_or(0),
            ys.max().unwrap_or(0),
            v.len(),
        )
    };
    let drawn = moved(&with, &without);
    let expected = moved(&reference, &without);
    assert!(!expected.is_empty(), "frame 0x81 writes nothing at the place the literals name");
    assert_eq!(
        corner(&drawn),
        corner(&expected),
        "the marker's ink (x0, y0, x1, y1, count) is not the 25 x 45 band frame 0x81 writes at \
         ({}, {}) — the tile is town()[2] and the offset is (+0x10, -0x12)",
        sx + 0x10,
        sy - 0x12,
    );
    assert_eq!(drawn, expected, "the marker's ink is the right size in the right place and is not the same pixels");

    game.kingdom.counties[county as usize].mercenary_offer = 0;
    let gone = draw(&mut screen, &mut game, &assets);
    assert_eq!(gone.diff_count(&without), 0, "no offer, no band");
}

/// **A merchant is drawn, and clicking one opens the merchant.** C50.
#[test]
fn a_merchant_is_drawn_and_opens_the_merchant_screen_from_the_county_it_is_in() {
    let (mut game, assets) = world!();
    let mine = (1..=game.kingdom.county_count as u8)
        .find(|&id| game.is_players(id))
        .expect("the player holds a county");
    let (merchant, _) = game
        .kingdom
        .campaign
        .units
        .iter()
        .find(|(_, u)| u.kind == l2_kingdom::UnitKind::Merchant)
        .map(|(id, u)| (id, u.owner))
        .expect("the fixture ships six merchants");
    assert_eq!(
        game.kingdom.campaign.units.get(merchant).map(|u| u.owner),
        Some(6),
        "every merchant in the game is ownerless, which is why the guard cannot be its owner",
    );

    let mut screen = MapScreen::new();
    draw(&mut screen, &mut game, &assets);

    // Stand it on a tile of the player's own county that is in shot. The
    // county's anchor is in shot because the map opened on it (C48).
    let (ax, ay) = {
        let c = &game.kingdom.counties[mine as usize];
        (c.anchor_x, c.anchor_y)
    };
    {
        let u = game.kingdom.campaign.units.get_mut(merchant).expect("the merchant");
        u.x = ax;
        u.y = ay;
        u.county = mine;
    }
    let (cx, cy) =
        campaign::tile_centre(screen.viewport(), screen.zoom(), ax as usize, ay as usize)
            .expect("the anchor is in shot");

    let with = draw(&mut screen, &mut game, &assets);
    let put_back = game.kingdom.campaign.units.remove(merchant).expect("the merchant");
    let without = draw(&mut screen, &mut game, &assets);
    assert!(with.diff_count(&without) > 0, "the merchant painted nothing");
    game.kingdom.campaign.units.put(merchant, put_back);

    // And clicking it opens screen 0x08 **carrying the unit**, because the
    // price is that merchant's own morale — `DAT_00553C64`.
    let t = send(&mut screen, &mut game, &assets, Event::Click { x: cx, y: cy });
    assert_eq!(t, Transition::Push(ScreenId::Merchant(merchant)), "the merchant screen");

    let theirs = (1..=game.kingdom.county_count as u8)
        .find(|&id| id != mine && !game.is_players(id))
        .expect("England has counties the player does not own");
    game.kingdom.campaign.units.get_mut(merchant).expect("the merchant").county = theirs;
    let t = send(&mut screen, &mut game, &assets, Event::Click { x: cx, y: cy });
    assert_eq!(t, Transition::Stay, "a merchant in a county you do not own opens nothing");
}

/// `Map_ResolvePick` (`0x0046D5FE`) has no such problem: `g_pickedTileUnit =
/// g_tiles[t].unit`, so the whole tile is the merchant. This sweeps both — the
/// tile's diamond and the figure's opaque pixels — with an unrelated county
/// selected throughout, and requires every one of them to reach the merchant.
#[test]
fn a_merchant_is_clickable_over_its_whole_tile_whichever_county_is_selected() {
    let (mut game, assets) = world!();
    let mine = (1..=game.kingdom.county_count as u8)
        .find(|&id| game.is_players(id))
        .expect("the player holds a county");
    let elsewhere = (1..=game.kingdom.county_count as u8)
        .find(|&id| id != mine)
        .expect("England has more than one county");
    let merchant = game
        .kingdom
        .campaign
        .units
        .iter()
        .find(|(_, u)| u.kind == l2_kingdom::UnitKind::Merchant)
        .map(|(id, _)| id)
        .expect("the fixture ships six merchants");

    let mut screen = MapScreen::new();
    draw(&mut screen, &mut game, &assets);
    let (ax, ay) = {
        let c = &game.kingdom.counties[mine as usize];
        (c.anchor_x, c.anchor_y)
    };
    {
        let u = game.kingdom.campaign.units.get_mut(merchant).expect("the merchant");
        u.x = ax;
        u.y = ay;
        u.county = mine;
    }
    game.select(elsewhere);
    draw(&mut screen, &mut game, &assets);

    let (cx, cy) =
        campaign::tile_centre(screen.viewport(), screen.zoom(), ax as usize, ay as usize)
            .expect("the anchor is in shot");
    let zoom = *screen.zoom();
    let (hw, hh) = (zoom.tile_w / 2, zoom.tile_h / 2);

    let hits = |screen: &MapScreen, game: &mut Game, pts: &[(i32, i32)]| {
        let mut ok = 0;
        for &(x, y) in pts {
            let ctx = Ctx { game, assets: &assets };
            if screen.unit_at(&ctx, x, y) == Some(merchant) {
                ok += 1;
            }
        }
        ok
    };

    let mut ground = Vec::new();
    for dy in -hh + 2..hh - 1 {
        let span = hw - (dy.abs() * hw) / hh;
        for dx in [-span + 2, 0, span - 2] {
            ground.push((cx + dx, cy + dy));
        }
    }
    assert!(ground.len() > 60, "the diamond is 58 x 30 and this samples it");
    assert_eq!(
        hits(&screen, &mut game, &ground),
        ground.len(),
        "every pixel of the merchant's own tile is the merchant"
    );

    let sprite = {
        let u = game.kingdom.campaign.units.get(merchant).expect("the merchant");
        campaign::UnitSprite {
            sheet: u.sprite_sheet(),
            frame: game.unit_frame(merchant, u),
            nudge: u.sprite_nudge(),
            walk: campaign::walk_offset(&zoom, u.facing, u.sub_tile),
        }
    };
    let rect = campaign::unit_sprite_rect(
        &assets.map,
        screen.viewport(),
        &zoom,
        (ax as usize, ay as usize),
        sprite,
    );
    if let Some((ox, oy, art)) = rect {
        assert!(art.height as i32 > zoom.tile_h, "the figure is taller than its tile");
        let mut figure = Vec::new();
        for dy in 0..art.height as i32 {
            for dx in 0..art.width as i32 {
                if art.opaque[dy as usize * art.width as usize + dx as usize] {
                    figure.push((ox + dx, oy + dy));
                }
            }
        }
        assert!(figure.len() > 200, "the merchant is a figure, not a dot");
        assert_eq!(
            hits(&screen, &mut game, &figure),
            figure.len(),
            "every painted pixel of the merchant is the merchant"
        );
    }

    assert_eq!(game.selected, elsewhere, "hit-testing selects nothing");
    let t = send(&mut screen, &mut game, &assets, Event::Click { x: cx, y: cy });
    assert_eq!(t, Transition::Push(ScreenId::Merchant(merchant)), "and it opens the merchant");
}



