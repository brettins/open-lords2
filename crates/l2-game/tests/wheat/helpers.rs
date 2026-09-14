#![allow(unused_imports)]
use super::*;
use super::wheat_test::*;
use std::path::PathBuf;
use l2_formats::maps::Plane;
use l2_game::game::Assets;
use l2_game::input::{Event, Key};
use l2_game::scenario;
use l2_game::screen::{Ctx, Machine, Screen, ScreenId};
use l2_game::screens::map::MapScreen;
use l2_game::Game;
use l2_kingdom::field::FieldType;
use l2_kingdom::map::{coords, index, MAP_DIM, MAP_TILES};
use l2_kingdom::tables::Tables;
use l2_mods::Platform;
use l2_view::{campaign, Canvas};

/// **The original's variant for one county's grain tiles**, transcribed with
/// its literals.
///
/// ```c
/// /* Grain_SeasonTick, 0x0044C8AE */
/// if (g_season == 1 || g_season == 2 || g_season == 3)
///      band = FUN_0044CF6F(crop[1], (byte)county.field_0x206);
/// else band = FUN_0044CF6F(crop[2], (byte)county.field_0x206);   /* g_season == 4 */
/// variant = band < 3 ? 0 : (band - 3) / 4 + 1;
///
/// /* FUN_0044CF6F, 0x0044CF6F */
/// if (crop < 1) return 2;  if (fields < 1) return 2;
/// if (crop / fields < 0x29) return 3;  if (crop / fields < 0x51) return 7;  return 0xB;
///
/// /* FUN_00469D21, the shortfall arm: every grain tile after the first */
/// if (lo == 2 && county.field_0x1a7 != 0 && notFirst) Terrain_Set(tile, 2, 0);
/// ```
fn originals_variant(season: u8, crop: [i32; 3], standing: i32, shortfall: bool, first: bool) -> u8 {
    if shortfall && !first {
        return 0;
    }
    let word = if season == 4 { crop[2] } else { crop[1] };
    let fields = standing & 0xFF;
    let band: i32 = if word < 1 || fields < 1 {
        2
    } else if word / fields < 0x29 {
        3
    } else if word / fields < 0x51 {
        7
    } else {
        0xB
    };
    if band < 3 {
        0
    } else {
        ((band - 3) / 4 + 1) as u8
    }
}

/// What C124's fix drew: `crop[2]` over `fieldsGrain`, in every season —
/// **as it stood inside the tick**. `Grain_SeasonTick` clears `crop[2]` at its
/// top and only the harvest refills it before the repaint, so outside Winter
/// that word was zero when the band was taken. (After the turn it can hold
/// `Grain_LabourEstimate`'s harvest forecast, which the repaint never saw.)
fn c124_variant(season: u8, crop: [i32; 3], fields_grain: i32) -> u8 {
    let word = if season == 4 { crop[2] } else { 0 };
    let band = if word < 1 || fields_grain < 1 {
        2
    } else if word / fields_grain < 0x29 {
        3
    } else if word / fields_grain < 0x51 {
        7
    } else {
        0xB
    };
    if band < 3 {
        0
    } else {
        ((band - 3) / 4 + 1) as u8
    }
}

fn draw<S: Screen>(screen: &mut S, game: &mut Game, assets: &Assets) -> Canvas {
    let mut canvas = Canvas::screen();
    let ctx = Ctx { game, assets };
    screen.draw(&ctx, &mut canvas);
    canvas
}

/// **End Turn, as a player presses it**: the key on the campaign map, then
/// frames until the turn comes round, then the fade back up.
fn end_turn(machine: &mut Machine, game: &mut Game, assets: &Assets) {
    let before = game.kingdom.turn_count;
    {
        let mut ctx = Ctx { game: &mut *game, assets };
        machine.handle(Event::KeyDown(Key::letter('e')), &mut ctx);
    }
    for _ in 0..4_000 {
        if game.kingdom.turn_count > before {
            break;
        }
        let mut ctx = Ctx { game: &mut *game, assets };
        machine.update(&mut ctx);
    }
    assert!(game.kingdom.turn_count > before, "the turn never came round");
    for _ in 0..=l2_view::fade::PHASES {
        let mut ctx = Ctx { game: &mut *game, assets };
        machine.update(&mut ctx);
    }
    assert_eq!(machine.top_id(), Some(ScreenId::Campaign), "back on the campaign map");
}

/// A field tile nothing is drawn *over*: inside the map, no town or castle or
/// industry site (plane-0 `0x40`/`0x80`, whose flags and animations are
/// overlays) within one tile, no unit within three, and no county anchor —
/// where our own owner marker goes — within one.
///
/// A neighbouring pasture's herd is **not** excluded, because [`tile_box`]
/// reads only the inner part of the tile's own diamond and a herd sprite is a
/// neighbour's diamond shifted `(+4, −4)` (`Sprite_TopIt`'s farm arm), which
/// never reaches it.
fn a_quiet_tile(game: &Game, candidates: &[usize]) -> usize {
    let map = &game.kingdom.campaign.map;
    let near = |t: usize, r: i32, test: &dyn Fn(u8, u8) -> bool| {
        let (x, y) = coords(t);
        (-r..=r).any(|dy| {
            (-r..=r).any(|dx| {
                let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                (0..MAP_DIM as i32).contains(&nx)
                    && (0..MAP_DIM as i32).contains(&ny)
                    && test(nx as u8, ny as u8)
            })
        })
    };
    let settled = |x: u8, y: u8| map.flags[index(x, y)] & 0xC0 != 0;
    let unit = |x: u8, y: u8| game.kingdom.campaign.units.iter().any(|(_, u)| (u.x, u.y) == (x, y));
    let anchor = |x: u8, y: u8| {
        game.kingdom.county_ids().any(|id| (game.anchor_x[id], game.anchor_y[id]) == (x, y))
    };
    let tests: [(&str, &dyn Fn(usize) -> bool); 4] = [
        ("inside the map", &|t| {
            let (x, y) = coords(t);
            (4..60).contains(&x) && (4..60).contains(&y)
        }),
        ("no settlement within 1", &|t| !near(t, 1, &settled)),
        ("no unit within 3", &|t| !near(t, 3, &unit)),
        ("no anchor within 1", &|t| !near(t, 1, &anchor)),
    ];
    if let Some(&tile) = candidates.iter().find(|&&t| tests.iter().all(|(_, test)| test(t))) {
        return tile;
    }
    let refused: Vec<(&str, usize)> =
        tests.iter().map(|(name, test)| (*name, candidates.iter().filter(|&&t| !test(t)).count())).collect();
    panic!("none of {} sown fields is quiet; refused by: {refused:?}", candidates.len());
}

/// The terrain pass alone, with this tile forced to `frame` — the picture the
/// original's `Map_DrawTile` (`0x004063C1`) makes of `tile.frame` through the
/// roads bank of the current zoom.
fn terrain_with(game: &mut Game, assets: &Assets, screen: &MapScreen, x: usize, y: usize, frame: u8) -> Canvas {
    let slot = assets.slot(game.map_slot).expect("the map slot");
    let lattice = campaign::Lattice::build(&slot);
    let ctx = Ctx { game: &mut *game, assets };
    let mut overrides = MapScreen::tile_graphics(&ctx);
    overrides.set(x, y, ROADS_BANK_BYTE, frame);
    let hidden = |tx: usize, ty: usize| ctx.game.hides_tile(index(tx as u8, ty as u8));
    let fog: campaign::Fog = if ctx.game.kingdom.options.exploration { Some(&hidden) } else { None };
    let mut canvas = Canvas::screen();
    canvas.clear(assets.ink.background);
    let mut tags = l2_view::Tags::screen();
    campaign::draw(
        &mut canvas,
        &slot,
        &lattice,
        &assets.map,
        screen.viewport(),
        screen.zoom(),
        &mut tags,
        &overrides,
        ctx.game.kingdom.season,
        fog,
    );
    canvas
}

/// The pixels of the inner three-fifths of one tile's diamond, clipped to the
/// map viewport: `|dx| / (w/2) + |dy| / (h/2) <= 3/5` about the tile's centre.
///
/// Inner, because the diamond's rim is shared with what its neighbours draw
/// after it — a herd shifted `(+4, −4)`, an apex row — and the claim is about
/// this tile's frame, not theirs.
fn tile_box(canvas: &Canvas, screen: &MapScreen, x: usize, y: usize) -> Vec<u8> {
    let zoom = screen.zoom();
    let (w, h) = (zoom.tile_w, zoom.tile_h);
    let (cx, cy) = campaign::tile_centre(screen.viewport(), zoom, x, y).expect("the tile is in view");
    let clip = screen.map_clip();
    let mut out = Vec::new();
    for py in cy - h / 2..=cy + h / 2 {
        for px in cx - w / 2..=cx + w / 2 {
            let (dx, dy) = ((px - cx).abs(), (py - cy).abs());
            if 10 * (dx * h + dy * w) <= 3 * w * h && clip.contains(px, py) {
                out.push(canvas.at(px as usize, py as usize));
            }
        }
    }
    out
}

/// Paint the campaign screen on `tile` at one zoom and return which of the four
/// wheat variants the tile shows, or `None` for none of them. Also returns
/// whether the four are four different pictures at this zoom.
fn drawn_variant(game: &mut Game, assets: &Assets, tile: usize, far: bool) -> (Option<u8>, bool) {
    let (x, y) = coords(tile);
    let (x, y) = (x as usize, y as usize);
    let stored = assets.slot(game.map_slot).expect("the map slot").at(Plane::GfxIndex, x, y);

    let mut screen = MapScreen::new();
    if far {
        let mut ctx = Ctx { game: &mut *game, assets };
        screen.handle(Event::KeyDown(Key::letter('z')), &mut ctx);
        assert_eq!(screen.zoom().id, campaign::FAR.id, "Z zooms out");
    } else {
        screen.centre_on_tile(x, y);
    }
    let drawn = draw(&mut screen, game, assets);
    let (cx, cy) = campaign::tile_centre(screen.viewport(), screen.zoom(), x, y)
        .expect("the field is in view at this zoom");
    assert!(screen.map_clip().contains(cx, cy), "({x}, {y}) is inside the map viewport");
    assert!(!game.hides_tile(tile), "({x}, {y}) is seen: the fog is on and the field is lit");

    let candidates: Vec<Vec<u8>> = (0..4u8)
        .map(|v| {
            let frame = WHEAT_BASE + (stored & 3) + 4 * v;
            tile_box(&terrain_with(game, assets, &screen, x, y, frame), &screen, x, y)
        })
        .collect();
    let distinct = (0..4).all(|a| (a + 1..4).all(|b| candidates[a] != candidates[b]));
    let got = tile_box(&drawn, &screen, x, y);
    (candidates.iter().position(|c| *c == got).map(|v| v as u8), distinct)
}

