#![allow(unused_imports)]
use super::*;
use super::field_panel::*;
use common::*;
use l2_game::game::Assets;
use l2_game::input::Event;
use l2_game::input::Key;
use l2_game::screen::Ctx;
use l2_game::screen::Machine;
use l2_game::screen::ScreenId;
use l2_game::screens::map;
use l2_game::screens::map::MapScreen;
use l2_game::shell::font;
use l2_game::Game;
use l2_view::campaign;
use l2_view::Canvas;

/// One tile of the England position, rewritten to the plane bytes an arm of
/// `TileInfo_Draw` (`0x0041C208`) tests, with the panel drawn over the map.
///
/// The tile is real and in a county the player holds, so the county name and
/// the head-room the layout grants are the position's own.
fn tile_panel(
    game: &mut Game,
    assets: &Assets,
    flags: u8,
    terrain: u8,
    bank: u8,
) -> (Canvas, l2_game::screens::info::Layout) {
    use l2_game::screens::info::{InfoScreen, Target};
    let tile = (0..l2_kingdom::MAP_TILES)
        .find(|&t| {
            let c = game.kingdom.campaign.map.county[t] as usize;
            c != 0 && game.kingdom.counties.get(c).is_some_and(|c| c.owner == game.player)
        })
        .expect("the player holds a county");
    game.kingdom.campaign.map.flags[tile] = flags;
    game.kingdom.campaign.map.terrain[tile] = terrain;
    game.kingdom.campaign.map.bank[tile] = bank;
    let layout = {
        let screen = InfoScreen::new(Target::Tile(tile));
        let ctx = Ctx { game, assets };
        screen.layout(&ctx)
    };
    let mut m = over_the_map(ScreenId::Info(Target::Tile(tile)));
    (draw_stack(&mut m, game, assets), layout)
}

/// The heading at `(0x28, row * 0x10 + 0x40)` and the body's y at
/// `row * 0x10 + 100` — `TileInfo_Draw`'s own literals, not constants of
/// `screens/info.rs`.
fn tile_panel_says(
    canvas: &Canvas,
    assets: &Assets,
    l: l2_game::screens::info::Layout,
    h: &str,
    b: &str,
) {
    assert_eq!(
        find_heading(canvas, assets, h, font::TEXT),
        Some((0x28, l.row * 0x10 + 0x40)),
        "the heading {h:?} at (0x28, row*16 + 0x40)"
    );
    assert_eq!(
        find_body(canvas, assets, b, font::TEXT).map(|p| p.1),
        Some(l.row * 0x10 + 100),
        "the body {b:?} at row*16 + 100"
    );
}

/// **`TileInfo_Draw`'s first arm, `flags & 0x01`**: heading 30/1, body 30/23,
/// icon `0x18`. A player right-clicking a road got a blank panel.
///
/// Ablated: dropping the `Road` row from `TILE_LADDER` fails on the heading.
#[test]
fn a_road_tile_says_road() {
    let (mut game, assets) = world!();
    assert_eq!(assets.shell.text(30, 1), "Road.", "L2.eng 30/1");
    assert_eq!(
        assets.shell.text(30, 23),
        "Roads allow rapid movement of troops around the kingdom.",
        "L2.eng 30/23"
    );
    let (canvas, l) = tile_panel(&mut game, &assets, l2_kingdom::map::flags::ROAD, 0, 0);
    // `FUN_0041BEFE` has no `0x01` arm, so a road takes its fall-through.
    assert_eq!((l.row, l.headroom), (0x11, 2), "FUN_0041BEFE's fall-through");
    tile_panel_says(&canvas, &assets, l, "Road.", "Roads allow rapid movement");
}

/// **`flags & 0x04`**: heading 30/3, body 30/24, icon `0x1D` — and
/// `FUN_0041BEFE`'s only arm outside a dwelling that grants no head-room.
///
/// Ablated: removing the sea arm from `layout` draws the county name over the
/// sea and moves both lines two rows, failing on the y.
#[test]
fn a_sea_tile_says_sea_and_gets_no_head_room() {
    let (mut game, assets) = world!();
    assert_eq!(assets.shell.text(30, 3), "Sea.", "L2.eng 30/3");
    assert_eq!(assets.shell.text(30, 24), "Sea. Best left to the fish!", "L2.eng 30/24");
    let (canvas, l) = tile_panel(&mut game, &assets, l2_kingdom::map::flags::NO_COUNTY, 0, 0);
    assert_eq!((l.row, l.headroom), (0x11, 0), "sea is row 0x11 with no head-room");
    tile_panel_says(&canvas, &assets, l, "Sea.", "Best left to the fish!");
}

/// **`flags & 0x10`, both halves**: graphic `0x10` is 30/48 + 30/49 and
/// anything else is 30/50 + 30/51, icon `0x1B` for both, and `FUN_0041BEFE`
/// puts a dwelling on row `0x10` with no head-room.
///
/// **Graphic 0 is neither**: `Map_ResolvePick` (`0x0046D5FE`) blanks the flags
/// of an empty plot, so it falls to scrubland — which is the England position's
/// own state for all fifty-six plots.
///
/// Ablated: dropping the `terrain == 0x10` test draws the ruin's words on the
/// village; dropping the blanking draws *"Ruined village."* on an empty plot.
#[test]
fn a_dwelling_plot_says_village_ruined_village_or_nothing() {
    let (mut game, assets) = world!();
    for (i, w) in [(48, "Village"), (50, "Ruined village."), (0, "Scrubland.")] {
        assert_eq!(assets.shell.text(30, i), w, "L2.eng 30/{i}");
    }
    let plot = l2_kingdom::map::flags::PLOT;
    let (canvas, l) = tile_panel(&mut game, &assets, plot, 0x10, 0);
    assert_eq!((l.row, l.headroom), (0x10, 0), "a dwelling is row 0x10 with no head-room");
    tile_panel_says(&canvas, &assets, l, "Village", "A peaceful hamlet");

    let (canvas, l) = tile_panel(&mut game, &assets, plot, 0x11, 0);
    tile_panel_says(&canvas, &assets, l, "Ruined village.", "A once peaceful hamlet");

    // The blanked pick: no bits, so the scrubland fall-through and its layout.
    let (canvas, l) = tile_panel(&mut game, &assets, plot, 0, 0);
    assert_eq!((l.row, l.headroom), (0x11, 2), "a blanked pick takes the fall-through");
    tile_panel_says(&canvas, &assets, l, "Scrubland.", "Empty land that may be crossed");
}

/// **`flags & 0x08` splits on the bank plane and on nothing else.**
/// `Map_ResolvePick` sets `DAT_005651BC` on `(tile.bank & 0x1C) == 4` and
/// `TileInfo_Draw` reads that global: 30/4 + 30/25 + icon `0x19` for a
/// mountain, 30/5 + 30/26 + icon `0x1A` for a wood. The two tiles below differ
/// in **one byte**.
///
/// Ablated: making `is_mountain` always false calls the mountain a wood and
/// fails on the heading.
#[test]
fn a_rough_tile_says_mountain_or_woodland_by_its_bank() {
    let (mut game, assets) = world!();
    for (i, w) in [(4, "Mountain."), (5, "Woodland.")] {
        assert_eq!(assets.shell.text(30, i), w, "L2.eng 30/{i}");
    }
    let rough = l2_kingdom::map::flags::ROUGH;
    let (canvas, l) = tile_panel(&mut game, &assets, rough, 0, 0x04);
    tile_panel_says(&canvas, &assets, l, "Mountain.", "Mountains are impassable.");
    let (canvas, l) = tile_panel(&mut game, &assets, rough, 0, 0x00);
    tile_panel_says(&canvas, &assets, l, "Woodland.", "Vital to the timber industry!");
}

/// **The bottom of the ladder**: no bit set at all is 30/0 + 30/22, icon
/// `0x17`. It is also where `Map_ResolvePick`'s two blanked picks land — a bare
/// castle plot (`0x80` with graphic `0x14`) among them, which used to draw the
/// castle arm's *"Castle."* over open ground in every castleless county.
///
/// Ablated: removing the `graphic == 0x14` blanking from `picked_flags` fails
/// the second half on *"Scrubland."* being absent.
#[test]
fn an_unflagged_tile_and_a_bare_castle_plot_both_say_scrubland() {
    let (mut game, assets) = world!();
    assert_eq!(
        assets.shell.text(30, 22),
        "Empty land that may be crossed, but is unusable for farming.",
        "L2.eng 30/22"
    );
    let (canvas, l) = tile_panel(&mut game, &assets, 0, 0, 0);
    tile_panel_says(&canvas, &assets, l, "Scrubland.", "Empty land that may be crossed");

    // `Map_ResolvePick`: `flags & 0x80` with graphic `0x14` blanks the pick.
    let settlement = l2_kingdom::map::flags::SETTLEMENT;
    let (canvas, l) = tile_panel(&mut game, &assets, settlement, 0x14, 0);
    tile_panel_says(&canvas, &assets, l, "Scrubland.", "Empty land that may be crossed");
    assert_eq!(find_heading(&canvas, &assets, "Castle.", font::TEXT), None, "no castle arm");
}

