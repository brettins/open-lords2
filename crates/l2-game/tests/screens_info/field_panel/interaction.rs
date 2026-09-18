#![allow(unused_imports)]
use super::*;
use super::labels_and_views::*;
use super::*;
use super::tile_panel_part::*;
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

/// A player: *"Clicking on a field still brings up placeholder … right click
/// and left click on fields in game."*
/// `Map_Click`'s farmland arm is `_DAT_005681CC = 3; g_screenId = 4;
/// FUN_0041B032();` and the right button's `FUN_0043CAF4` ends in the same two
/// statements, so both land on screen `0x04`'s tile half for the picked tile.
#[test]
fn a_left_click_on_your_own_field_opens_what_a_right_click_opens() {
    use l2_game::screens::info::Target;
    let (mut game, assets) = world!();
    let players: Vec<u8> =
        (1..=game.kingdom.county_count as u8).filter(|&id| game.is_players(id)).collect();
    let (county, tile, at) = players
        .into_iter()
        .find_map(|id| {
            visible_field(&mut game, &assets, id, |k| k == l2_kingdom::field::FieldType::Fallow)
                .map(|(t, at)| (id, t, at))
        })
        .expect("one of the player's fallow fields is in view at the opening viewport");
    let grain_before = game.kingdom.counties[county as usize].fields_grain;
    let panel = Some(ScreenId::Info(Target::Tile(tile)));

    let mut right = Machine::new(ScreenId::Campaign);
    draw_stack(&mut right, &mut game, &assets);
    send_stack(&mut right, &mut game, &assets, Event::RightClick { x: at.0, y: at.1 });
    assert_eq!(right.top_id(), panel, "the right button opens the information panel on that field");
    assert_eq!(right.depth(), 2);
    send_stack(&mut right, &mut game, &assets, Event::RightClick { x: at.0, y: at.1 });
    assert_eq!(right.ids(), vec![ScreenId::Campaign]);

    let mut left = Machine::new(ScreenId::Campaign);
    draw_stack(&mut left, &mut game, &assets);
    send_stack(&mut left, &mut game, &assets, Event::Click { x: at.0, y: at.1 });
    assert_eq!(left.top_id(), panel, "and the left button opens the same screen, on the same tile");
    assert_eq!(left.depth(), 2);

    send_stack(&mut left, &mut game, &assets, Event::Click { x: 328, y: 400 });
    assert_eq!(
        game.kingdom.counties[county as usize].fields_grain,
        grain_before + 1,
        "the button reached Field_SetType"
    );
    assert_eq!(game.kingdom.campaign.map.terrain[tile], l2_kingdom::field::terrain::GRAIN);
    assert_eq!(left.ids(), vec![ScreenId::Campaign], "FUN_00438B02 ends in g_screenId = 0");

    let other = (1..game.kingdom.realms.len() as u8)
        .find(|&r| r != game.player)
        .expect("there is another realm");
    game.kingdom.counties[county as usize].owner = other;
    send_stack(&mut left, &mut game, &assets, Event::Click { x: at.0, y: at.1 });
    assert_eq!(left.ids(), vec![ScreenId::Campaign], "a foreign field opens nothing on the left button");
    send_stack(&mut right, &mut game, &assets, Event::RightClick { x: at.0, y: at.1 });
    assert_eq!(right.top_id(), panel, "and the right button still opens the panel");
}

#[test]
fn another_lords_fields_are_not_yours_to_paint() {
    let (mut game, assets) = world!();
    let theirs = (1..=game.kingdom.county_count as u8)
        .find(|&id| !game.is_players(id) && game.kingdom.counties[id as usize].owner != 0)
        .expect("somebody else holds a county");
    let (tile, _) = game.kingdom.field_tiles(theirs as usize)[0];
    let before = game.kingdom.counties[theirs as usize].clone();

    let mut screen = MapScreen::new();
    let (x, y) = on_screen(&mut screen, tile);
    send(&mut screen, &mut game, &assets, Event::Click { x, y });
    send(&mut screen, &mut game, &assets, Event::Click { x: 328, y: 208 });
    assert_eq!(game.kingdom.counties[theirs as usize], before);
}

#[test]
fn clicking_a_mine_switches_that_industry_off_and_on_again() {
    let (mut game, assets) = world!();
    let county = (1..=game.kingdom.county_count as u8)
        .find(|&id| game.is_players(id))
        .expect("the player holds a county");
    game.select(county);

    let map = &game.kingdom.campaign.map;
    let sites: Vec<(usize, l2_kingdom::industry::MapToggle)> = (0..map.terrain.len())
        .filter(|&t| {
            map.county[t] == county && map.flags[t] & l2_kingdom::map::flags::SETTLEMENT != 0
        })
        .filter_map(|t| l2_kingdom::industry::map_toggle_for_graphic(map.terrain[t]).map(|w| (t, w)))
        .collect();
    assert!(sites.len() >= 4, "one site per industry: {sites:?}");

    let mut screen = MapScreen::new();
    for (tile, what) in sites {
        let l2_kingdom::industry::MapToggle::Industry(c) = what else { continue };
        let slot = c.index();
        let before = game.kingdom.counties[county as usize].industry[slot].enabled;
        let (x, y) = on_screen(&mut screen, tile);
        send(&mut screen, &mut game, &assets, Event::Click { x, y });
        assert_ne!(
            game.kingdom.counties[county as usize].industry[slot].enabled, before,
            "{c:?} at tile {tile} did not switch"
        );
        send(&mut screen, &mut game, &assets, Event::Click { x, y });
        assert_eq!(
            game.kingdom.counties[county as usize].industry[slot].enabled, before,
            "{c:?} did not switch back"
        );
    }
}

