#![allow(unused_imports)]
use super::*;
use super::drawn::*;
use super::super::*;
use super::super::fog_tests::*;
use super::super::*;
use super::super::view_tests::*;
use super::super::interaction_tests::*;
use super::super::structures_tests::*;
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

/// `Map_DrawPathMarker` (`0x004081A6`):
#[test]
fn a_march_hovered_onto_an_enemy_town_ends_in_the_gold_ball_and_only_steps_out_of_reach_are_grey() {
    use l2_kingdom::map::{coords, flags, index, MAP_TILES};
    use l2_kingdom::{Unit, UnitKind};
    let (mut game, assets) = world!();
    let player = game.player;
    let map = game.kingdom.campaign.map.clone();
    let plain = |t: usize| {
        map.county[t] != 0
            && map.flags[t] & (flags::IMPASSABLE | flags::CASTLE | flags::PLOT | flags::SETTLEMENT) == 0
    };
    let cost = map.cost_map();
    let mut chosen = None;
    'search: for t in 0..MAP_TILES {
        let (x, y) = coords(t);
        if !(8..56).contains(&x) || !(8..56).contains(&y) || map.flags[t] & flags::CASTLE == 0 {
            continue;
        }
        let owner = game.kingdom.counties.get(map.county[t] as usize).map_or(0, |c| c.owner);
        if owner == 0 || owner == player {
            continue;
        }
        #[rustfmt::skip]
        let offsets = [(-3i32, 0i32), (-3, 3), (0, 3), (3, 3), (3, 0), (3, -3), (0, -3), (-3, -3),
                       (-2, 0), (-2, 2), (0, 2), (2, 2), (2, 0), (2, -2), (0, -2), (-2, -2)];
        for (dx, dy) in offsets {
            let (sx, sy) = ((x as i32 + dx) as u8, (y as i32 + dy) as u8);
            if !plain(index(sx, sy)) {
                continue;
            }
            let fill = l2_kingdom::movement::flood_fill(&cost, (sx, sy), l2_kingdom::movement::Routing::Direct);
            let Some(path) = l2_kingdom::movement::extract_path(&cost, &fill, (x, y)) else { continue };
            let steps: Vec<((u8, u8), i32)> = path
                .iter()
                .filter(|&&p| p != (x, y))
                .map(|&p| (p, fill.cost_to(p.0, p.1).unwrap_or(99)))
                .collect();
            if path.last() == Some(&(x, y))
                && !steps.is_empty()
                && steps.iter().all(|&(p, c)| plain(index(p.0, p.1)) && (1..=15).contains(&c))
            {
                chosen = Some(((x, y), (sx, sy), steps));
                break 'search;
            }
        }
    }
    let (town, stand, steps) = chosen.expect("an enemy county's town a short plain march from somewhere");

    let crowd: Vec<usize> = game
        .kingdom
        .campaign
        .units
        .iter()
        .filter(|(_, u)| (u.x as i32 - town.0 as i32).abs() <= 5 && (u.y as i32 - town.1 as i32).abs() <= 5)
        .map(|(id, _)| id)
        .collect();
    for id in crowd {
        game.kingdom.campaign.units.remove(id);
    }
    let mut army = Unit::new(UnitKind::Army, player, stand.0, stand.1);
    army.men = 300;
    army.troops[0] = 300;
    army.county = map.county_at(stand.0, stand.1);
    army.owner_is_human = true;
    let army = game.kingdom.campaign.units.spawn(army).expect("a free unit slot");

    // The map screen alone: a Machine winds the turn on update (Battle_Frame 0x004B99C0) and a
    // BattlePrompt covered its map mid-scroll [V] measured.
    let mut ruler = MapScreen::new();
    draw(&mut ruler, &mut game, &assets);
    let want = campaign::Viewport::centred_on_tile(town.0 as usize, town.1 as usize, ruler.zoom());
    for _ in 0..200 {
        let v = ruler.viewport();
        let key = if v.col < want.col {
            (639, 240)
        } else if v.col > want.col {
            (0, 240)
        } else if v.row + 1 < want.row {
            (320, 479)
        } else if v.row > want.row + 1 {
            (320, 0)
        } else {
            break;
        };
        edge_step(None, &mut ruler, &mut game, &assets, key);
        if ruler.viewport() == v {
            break;
        }
    }
    let (vp, zm) = (ruler.viewport(), *ruler.zoom());
    let centre = |t: (u8, u8)| {
        campaign::tile_centre(vp, &zm, t.0 as usize, t.1 as usize)
            .unwrap_or_else(|| panic!("{t:?} is in view"))
    };
    let (ax, ay) = centre(stand);
    let (hx, hy) = centre(town);
    send(&mut ruler, &mut game, &assets, Event::Click { x: ax, y: ay });
    send(&mut ruler, &mut game, &assets, Event::Pointer { x: hx, y: hy });

    let sheet = assets.map.flag_sheet(&campaign::NEAR).expect("Flags1a.pl8");
    let ball = |n: usize| sheet.frame(n).unwrap_or_else(|| panic!("Flags1a.pl8 frame {n:#x}"));
    let at = |t: (u8, u8)| {
        let (row, col) = campaign::tile_to_cell(t.0 as usize, t.1 as usize);
        let (x, y) = campaign::cell_to_screen(vp, &zm, row, col);
        (x + 20, y + 6)
    };
    const GREY: usize = 0x38;
    const GOLD: usize = 0x4E;

    let figure = assets.map.sprite_sheet(&campaign::NEAR, 0).and_then(|s| s.frame(0x48)).expect("an army frame");
    let (fx0, fy0) = at_rest(&ruler, stand, (0, -4), &figure);
    let clear = |t: (u8, u8)| {
        let ((bx, by), b) = (at(t), ball(GREY));
        bx + b.width as i32 <= fx0
            || bx >= fx0 + figure.width as i32
            || by + b.height as i32 <= fy0
            || by >= fy0 + figure.height as i32
    };
    let visible: Vec<((u8, u8), i32)> = steps.iter().copied().filter(|&(t, _)| clear(t)).collect();
    assert!(!visible.is_empty(), "at least one step's ball is clear of the army's figure: {steps:?}");

    let canvas = draw(&mut ruler, &mut game, &assets);
    assert!(ink_at(&canvas, &ball(GOLD), at(town)), "the enemy town {town:?} ends the route in frame 0x4E, at (+0x14, +6)");
    assert!(!ink_at(&canvas, &ball(GREY), at(town)), "and not in the grey ball");
    for &(t, c) in &visible {
        assert!(ink_at(&canvas, &ball(GREY + c as usize), at(t)), "the step at {t:?}, cost {c} of 15, is frame 0x38 + {c}");
        assert!(!ink_at(&canvas, &ball(GREY), at(t)), "and not grey: {t:?} is in reach");
    }

    game.kingdom.campaign.units.get_mut(army).expect("the army").moves_used = 15;
    let canvas = draw(&mut ruler, &mut game, &assets);
    for &(t, c) in &visible {
        assert!(ink_at(&canvas, &ball(GREY), at(t)), "with no moves left the step at {t:?}, cost {c}, is grey");
    }
    assert!(
        ink_at(&canvas, &ball(GOLD), at(town)),
        "and the town is still gold — `local_14` is tested before the cost is looked at"
    );
}

/// **`FUN_0043CAF4`'s extra step: the right button also selects a county.**
///
/// ```c
/// if ((g_pickedTileCounty != 0) && (g_pickedTileCounty != g_selectedCounty) &&
///     (g_pickedTileUnit == 0)) {
///   DAT_0053f0dc = g_counties[g_pickedTileCounty].townTile;
///   if (DAT_0053f0dc != 0) {
///     g_selectedCounty = g_pickedTileCounty;
///     Map_CentreOnTile(DAT_0053f0dc);
///   }
///   DAT_004eb260 = 1; FUN_004050c0();
/// }
/// FUN_0041b032();
/// ```
#[test]
fn right_clicking_another_countys_ground_selects_it_and_a_unit_on_it_does_not() {
    let (mut game, assets) = world!();
    let mut screen = MapScreen::new();
    draw(&mut screen, &mut game, &assets);
    let here = game.selected;
    assert!(here != 0, "setup: the game opens with a county selected");

    let empty_tile = |game: &Game, id: u8| {
        let map = &game.kingdom.campaign.map;
        (0..map.county.len()).find(|&t| {
            let (x, y) = l2_kingdom::map::coords(t);
            map.county[t] == id && game.kingdom.campaign.units.at(x, y).is_none()
        })
    };
    let (there, tile, town) = (1..=game.kingdom.county_count as u8)
        .filter(|&id| id != here)
        .find_map(|id| {
            let ctx = Ctx { game: &mut game, assets: &assets };
            let town = *MapScreen::town(&ctx, id).first()?;
            Some((id, empty_tile(&game, id)?, town))
        })
        .expect("some other county has a town and a tile with nobody on it");

    let (x, y) = on_screen(&mut screen, tile);
    let opened = send(&mut screen, &mut game, &assets, Event::RightClick { x, y });
    assert_eq!(
        opened,
        Transition::Push(ScreenId::Info(l2_game::screens::info::Target::Tile(tile))),
        "the right click still opens the information panel on the tile"
    );
    assert_eq!(game.selected, there, "and it selected the county the tile belongs to");
    let (tx, ty) = l2_kingdom::map::coords(town);
    assert!(
        campaign::tile_centre(screen.viewport(), screen.zoom(), tx as usize, ty as usize).is_some(),
        "the view moved to county {there}'s town square"
    );

    let (elsewhere, ground) = (1..=game.kingdom.county_count as u8)
        .filter(|&id| id != there)
        .find_map(|id| Some((id, empty_tile(&game, id)?)))
        .expect("a third county with an empty tile");
    let unit = game
        .kingdom
        .campaign
        .units
        .iter()
        .find(|(_, u)| u.kind == l2_kingdom::UnitKind::Merchant)
        .map(|(id, _)| id)
        .expect("the fixture ships six merchants");
    let (gx, gy) = l2_kingdom::map::coords(ground);
    {
        let u = game.kingdom.campaign.units.get_mut(unit).expect("the merchant");
        u.x = gx;
        u.y = gy;
        u.county = elsewhere;
    }
    let (x, y) = on_screen(&mut screen, ground);
    let opened = send(&mut screen, &mut game, &assets, Event::RightClick { x, y });
    assert_eq!(
        opened,
        Transition::Push(ScreenId::Info(l2_game::screens::info::Target::Unit(unit))),
        "the right click opens the unit half"
    );
    assert_eq!(game.selected, there, "and a unit under the cursor selects nothing");
}



