#![allow(unused_imports)]
use super::*;
use l2_game::screens::map::MapScreen;

/// Where any of the sixteen damage frames sits on `canvas`, as `(frame, x, y)`.
fn damage_at(canvas: &Canvas, assets: &Assets, screen: &MapScreen) -> Vec<(usize, i32, i32)> {
    let Some(sheet) = assets.map.flag_sheet(screen.zoom()) else { return Vec::new() };
    let mut out = Vec::new();
    for k in 0..campaign::DAMAGE_FRAMES as usize {
        let frame = campaign::DAMAGE_FIRST_FRAME + k;
        let Some(decoded) = sheet.frame(frame) else { continue };
        out.extend(sprite_positions(canvas, &decoded).0.into_iter().map(|(x, y)| (frame, x, y)));
    }
    out
}

fn expected(screen: &MapScreen, tile: usize) -> (i32, i32) {
    let (x, y) = l2_kingdom::map::coords(tile);
    let (row, col) = campaign::tile_to_cell(x as usize, y as usize);
    let (sx, sy) = campaign::cell_to_screen(screen.viewport(), screen.zoom(), row, col);
    // `local_30 = 0xc; local_34 = -0xe;` — the literals, not our constant.
    (sx + 0x0C, sy - 0x0E)
}

/// **A burnt dwelling burns** — `Sprite_TopIt` (`0x004071A0`) arm 3: plane 0
/// `0x10`, `content == 0x13`, `DAT_005C9288 = DAT_0053E9A4 + 0x28` at
/// `(+0xC, −0xE)`, and nothing at `g_mapZoom == 2`.
///
/// **Ablated**: dropping `draw_damage`, moving `DAMAGE_AT`, or testing any
/// content but `0x13` turns this red.
#[test]
fn a_burnt_dwelling_carries_the_damage_animation_at_its_offset() {
    let (mut game, assets) = world!();
    // No `0x10` plot in this fixture: stage one on a bare tile of a held county.
    let map = &game.kingdom.campaign.map;
    let plot = (0..map.terrain.len())
        .find(|&t| map.flags[t] == 0 && map.county[t] != 0 && game.is_players(map.county[t]))
        .expect("the player holds a bare tile");
    game.kingdom.campaign.map.flags[plot] = l2_kingdom::map::flags::PLOT;
    let (px, py) = l2_kingdom::map::coords(plot);
    let mut screen = MapScreen::new();
    draw(&mut screen, &mut game, &assets);
    screen.centre_on_tile(px as usize, py as usize);

    game.kingdom.campaign.map.terrain[plot] = 0x12;
    let whole = draw(&mut screen, &mut game, &assets);
    assert!(damage_at(&whole, &assets, &screen).is_empty(), "a standing dwelling does not burn");

    game.kingdom.campaign.map.terrain[plot] = 0x13;
    let burnt = draw(&mut screen, &mut game, &assets);
    let at = expected(&screen, plot);
    let found = damage_at(&burnt, &assets, &screen);
    assert!(
        found.iter().any(|&(_, x, y)| (x, y) == at),
        "frame 0x28 + counter at {at:?}; found {found:?}"
    );

    send(&mut screen, &mut game, &assets, Event::KeyDown(Key::Char('Z')));
    assert_eq!(screen.zoom().id, campaign::FAR.id);
    let far = draw(&mut screen, &mut game, &assets);
    assert!(damage_at(&far, &assets, &screen).is_empty(), "arm 3 returns at g_mapZoom == 2");
}

/// **A wrecked industry shut three seasons burns** — arm 5c:
/// `if ((int)(char)industry[k].disabledSeasons < 3) return;` before the
/// same `0x28 + counter` at `(+0xC, −0xE)`.
///
/// **Ablated**: `>= 3` to `>= 2`, or dropping the arm, turns this red.
#[test]
fn a_wrecked_industry_burns_from_its_third_shut_season() {
    let (mut game, assets) = world!();
    let mut screen = MapScreen::new();
    screen.update(&mut Ctx { game: &mut game, assets: &assets });
    let (tile, county, commodity, _) = *screen
        .industry_sites_for_test()
        .iter()
        .find(|s| !game.hides_tile(s.0))
        .expect("the fixture has an industry site");
    let (x, y) = l2_kingdom::map::coords(tile);
    draw(&mut screen, &mut game, &assets);
    screen.centre_on_tile(x as usize, y as usize);
    let wrecked = l2_kingdom::map::terrain::INDUSTRY_IDLE[commodity] + 2;
    game.kingdom.campaign.map.terrain[tile] = wrecked;

    game.kingdom.counties[county as usize].industry[commodity].disabled_seasons = 2;
    let two = draw(&mut screen, &mut game, &assets);
    assert!(damage_at(&two, &assets, &screen).is_empty(), "two seasons shut: no fire");

    game.kingdom.counties[county as usize].industry[commodity].disabled_seasons = 3;
    let three = draw(&mut screen, &mut game, &assets);
    let at = expected(&screen, tile);
    let found = damage_at(&three, &assets, &screen);
    assert!(found.iter().any(|&(_, x, y)| (x, y) == at), "at {at:?}; found {found:?}");
}
