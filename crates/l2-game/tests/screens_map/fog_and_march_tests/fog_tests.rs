#![allow(unused_imports)]
use super::*;
use super::march_tests::*;
use super::*;
use super::view_tests::*;
use super::interaction_tests::*;
use super::structures_tests::*;
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

/// **`Map_DrawTile` (`0x004063C1`): `if (g_optExploration == 1 && (bank & 0x20)
/// == 0) { bank = 0; frame = 0; }`**, and `Map_DrawTileApex` draws nothing.
#[test]
fn a_dark_tile_draws_the_base_banks_first_frame_until_it_is_seen_or_the_fog_is_off() {
    use l2_formats::maps::Plane;
    use l2_kingdom::map::{coords, index, MAP_TILES};

    let (mut game, assets) = world!();
    game.kingdom.options.exploration = true;
    let slot = assets.slot(game.map_slot).expect("the map slot");
    let map = game.kingdom.campaign.map.clone();
    let tile = (0..MAP_TILES)
        .find(|&t| {
            let (x, y) = coords(t);
            if !(16..48).contains(&x) || !(16..48).contains(&y) || map.county[t] == 0 {
                return false;
            }
            let own = (
                slot.at(Plane::GfxBank, x as usize, y as usize) & 0x1C,
                slot.at(Plane::GfxIndex, x as usize, y as usize),
            );
            own != (0, 0)
                && (-2..=2).all(|dy: i32| {
                    (-2..=2).all(|dx: i32| {
                        game.hides_tile(index((x as i32 + dx) as u8, (y as i32 + dy) as u8))
                    })
                })
                && game.kingdom.campaign.units.iter().all(|(_, u)| {
                    (u.x as i32 - x as i32).abs() > 3 || (u.y as i32 - y as i32).abs() > 3
                })
        })
        .expect("England at turn one is dark almost everywhere");
    let (x, y) = coords(tile);

    for season in 1..=4u8 {
        let near = assets.map.bank(&campaign::NEAR, season, 0).and_then(|s| s.frame(0));
        let near = near.expect("the near base bank has a frame 0");
        assert_eq!((near.width, near.height), (58, 30), "season {season}");
        assert_eq!(near.opaque.iter().filter(|&&o| o).count(), 0, "season {season}: near, blank");
        let far = assets.map.bank(&campaign::FAR, season, 0).and_then(|s| s.frame(0));
        let far = far.expect("the far base bank has a frame 0");
        assert_eq!((far.width, far.height), (10, 6), "season {season}");
        assert_eq!(far.opaque.iter().filter(|&&o| o).count(), 36, "season {season}: far, a diamond");
    }
    let background = assets.ink.background;
    let lit_pixels = |screen: &MapScreen, canvas: &Canvas| {
        let (cx, cy) = campaign::tile_centre(screen.viewport(), screen.zoom(), x as usize, y as usize)
            .expect("the tile is in view");
        let mut n = 0;
        for py in cy - 3..=cy + 3 {
            for px in cx - 6..=cx + 6 {
                if canvas.pixels[py as usize * canvas.width + px as usize] != background {
                    n += 1;
                }
            }
        }
        n
    };

    let (screen, dark) = paint_at(&mut game, &assets, x, y);
    let centre = campaign::tile_centre(screen.viewport(), screen.zoom(), x as usize, y as usize)
        .expect("the tile is in the view it was centred on");
    assert!(screen.map_clip().contains(centre.0, centre.1), "({x}, {y}) is inside the map viewport");
    assert_eq!(
        lit_pixels(&screen, &dark),
        0,
        "({x}, {y}) is unseen and must be drawn as base frame 0"
    );

    let mut off = game.clone();
    off.kingdom.options.exploration = false;
    let (screen, off_canvas) = paint_at(&mut off, &assets, x, y);
    assert!(lit_pixels(&screen, &off_canvas) > 0, "the fog off shows ({x}, {y})");
    let mut off_all_seen = off.clone();
    off_all_seen.kingdom.campaign.explored.reveal_square(off.player, 32, 32, 64);
    let (_, off_all) = paint_at(&mut off_all_seen, &assets, x, y);
    assert_eq!(off_canvas.diff_count(&off_all), 0, "with the option off, what was seen draws nothing different");

    let county = map.county[tile];
    game.kingdom.campaign.explored.reveal_county(game.player, &map, county);
    let (screen, lit) = paint_at(&mut game, &assets, x, y);
    assert!(lit_pixels(&screen, &lit) > 0, "seen, ({x}, {y}) is its own terrain");
}

/// **The six surround arms of `Map_RenderIso`, `Map_RenderAlignedRow` and
/// `Map_RenderOffsetRow`**: `frame = g_optExploration == 1 ? 0 : cell -
/// 0x0FFF0000`. Everything seen, the corner of the map in view: turning the
/// option on changes the picture, and it changes only because of the surround —
/// An unseen tile is left.
#[test]
fn with_the_fog_on_the_sea_round_the_map_is_the_base_banks_first_frame() {
    let (mut game, assets) = world!();
    game.kingdom.campaign.explored.reveal_square(game.player, 32, 32, 64);
    game.kingdom.options.exploration = false;
    let (screen, off) = paint_at(&mut game, &assets, 0, 0);
    game.kingdom.options.exploration = true;
    let (_, on) = paint_at(&mut game, &assets, 0, 0);
    assert!(
        map_pixels_differ(&off, &on, screen.map_clip()) > 0,
        "the off-map surround is drawn from the lattice with the option off and as frame 0 with it on"
    );
}

#[test]
fn a_county_in_the_dark_gives_nothing_away_on_the_map() {
    use l2_kingdom::map::{flags, MAP_TILES};

    let (mut game, assets) = world!();
    game.kingdom.options.exploration = true;
    let county = game
        .kingdom
        .county_ids()
        .map(|id| id as u8)
        .find(|&id| {
            let c = &game.kingdom.counties[id as usize];
            c.owner != 0
                && c.owner != game.player
                && (0..MAP_TILES)
                    .all(|t| game.kingdom.campaign.map.county[t] != id || game.hides_tile(t))
        })
        .expect("an AI county the person has not seen");
    let town = {
        let ctx = Ctx { game: &mut game, assets: &assets };
        *MapScreen::town(&ctx, county).first().expect("the county has a town")
    };
    let (tx, ty) = l2_kingdom::map::coords(town);
    // A field of the county's that has **no herd on it yet**, so that making
    // it a crowded pasture is an appearance and not a change of picture. The
    // first version took any field, found one already at `0x16`, and the herd
    // gate's ablation stayed green: the herd was in both renders.
    let pasture = (0..MAP_TILES).find(|&t| {
        let m = &game.kingdom.campaign.map;
        m.county[t] == county
            && m.flags[t] & flags::FARMLAND != 0
            && campaign::herd_sprite(m.terrain[t], 0).is_none()
    });
    assert!(pasture.is_some(), "county {county} has a field with no herd on it");

    let give_away = |g: &mut Game, with_herd: bool| {
        let owner = g.kingdom.counties[county as usize].owner as usize;
        g.kingdom.realms[owner].shield_index = 0; // arm 1: the banner goes
        g.kingdom.counties[county as usize].owner = 0; // our owner marker's colour
        g.kingdom.counties[county as usize].mercenary_offer = 1; // arm 2: a band appears
        if let Some(t) = pasture.filter(|_| with_herd) {
            g.kingdom.campaign.map.terrain[t] = 0x16; // arm 4: a crowded herd
        }
        g.kingdom
            .campaign
            .units
            .spawn(l2_kingdom::Unit::new(l2_kingdom::UnitKind::Army, 2, tx, ty))
            .expect("a free unit slot");
    };

    let mut lit = game.clone();
    lit.kingdom.options.exploration = false;

    let (screen, before) = paint_at(&mut game, &assets, tx, ty);
    give_away(&mut game, true);
    let (_, after) = paint_at(&mut game, &assets, tx, ty);
    assert_eq!(
        map_pixels_differ(&before, &after, screen.map_clip()),
        0,
        "county {county} is unseen, and something of it was drawn on the map"
    );

    let (screen, before) = paint_at(&mut lit, &assets, tx, ty);
    give_away(&mut lit, false);
    let (_, after) = paint_at(&mut lit, &assets, tx, ty);
    assert!(
        map_pixels_differ(&before, &after, screen.map_clip()) > 0,
        "the control: with the fog off the same changes are on the map"
    );
}

