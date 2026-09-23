#![allow(unused_imports)]
use super::*;
use l2_game::screens::map::MapScreen;
use l2_kingdom::UnitKind;

fn figure_origin(screen: &MapScreen, game: &l2_game::game::Game, assets: &Assets, id: usize) -> (i32, i32) {
    let u = game.kingdom.campaign.units.get(id).expect("the unit");
    let sprite = campaign::UnitSprite {
        sheet: u.sprite_sheet(),
        frame: game.unit_frame(id, u),
        nudge: u.sprite_nudge(),
        walk: campaign::walk_offset(screen.zoom(), u.facing, u.sub_tile),
    };
    let (x, y, _) = campaign::unit_sprite_rect(
        &assets.map,
        screen.viewport(),
        screen.zoom(),
        (u.x as usize, u.y as usize),
        sprite,
    )
    .expect("the figure is drawn");
    (x, y)
}

fn found(canvas: &Canvas, assets: &Assets, screen: &MapScreen, frame: usize) -> Vec<(i32, i32)> {
    let art = assets.map.flag_sheet(screen.zoom()).and_then(|s| s.frame(frame)).expect("Flags1a");
    sprite_positions(canvas, &art).0
}

/// **An army flies its owner's banner; a peasant mob flies frame `0x79 + phase`**
/// — `Map_DrawArmies` (`0x00408438`), after the figure's blit:
///
/// ```c
/// if (kind == 1 && shield != 0) { frame = shield*8 - 8 + DAT_0057D390; x = x0 + 0x12; y = y0 - 0x15; }
/// else if (kind == 2)           { frame = DAT_0057D390 + 0x79;         x = x0 + 0x12; y = y0 - 0x12; }
/// ```
///
/// `(x0, y0)` is the figure's own top-left. **Ablated**: dropping the banner,
/// either offset, or the `shield != 0` guard turns this red.
#[test]
fn an_army_flies_its_shield_and_a_peasant_mob_its_own_banner() {
    let (mut game, assets) = world!();
    game.kingdom.options.exploration = false;
    let id = game
        .kingdom
        .campaign
        .units
        .iter()
        .find(|(_, u)| !u.is_garrisoned())
        .map(|(id, _)| id)
        .expect("the fixture has a unit in the field");
    // Staged: the fixture fields no army.
    let u = game.kingdom.campaign.units.get_mut(id).unwrap();
    (u.kind, u.shield) = (UnitKind::Army, 2);
    let (ux, uy) = {
        let u = game.kingdom.campaign.units.get(id).unwrap();
        (u.x as usize, u.y as usize)
    };
    let mut screen = MapScreen::new();
    draw(&mut screen, &mut game, &assets);
    screen.centre_on_tile(ux, uy);

    // No `update`, so `DAT_0057D390` is phase 0: shield 2 is frame 8.
    let canvas = draw(&mut screen, &mut game, &assets);
    let (x0, y0) = figure_origin(&screen, &game, &assets, id);
    assert!(found(&canvas, &assets, &screen, 8).contains(&(x0 + 0x12, y0 - 0x15)));

    game.kingdom.campaign.units.get_mut(id).unwrap().shield = 0;
    let bare = draw(&mut screen, &mut game, &assets);
    assert!(!found(&bare, &assets, &screen, 8).contains(&(x0 + 0x12, y0 - 0x15)), "shield 0 flies nothing");

    game.kingdom.campaign.units.get_mut(id).unwrap().kind = UnitKind::PeasantMob;
    let mob = draw(&mut screen, &mut game, &assets);
    let (x0, y0) = figure_origin(&screen, &game, &assets, id);
    assert!(found(&mob, &assets, &screen, 0x79).contains(&(x0 + 0x12, y0 - 0x12)));
}

/// **The menu bar is bevelled** — `Screen_DrawMenuBar` (`0x00419C78`) calls
/// `Ui_DrawBevelRect(0, 0, 0x280, 0x18)` (`0x00403FDD`): top and right in
/// `0x1F`, then bottom and left in `0x10`, so the left line wins the corner.
///
/// **Ablated**: dropping the call turns this red.
#[test]
fn the_menu_bar_is_bevelled() {
    let (mut game, assets) = world!();
    let mut screen = MapScreen::new();
    let c = draw(&mut screen, &mut game, &assets);
    assert_eq!((c.at(0x100, 0), c.at(0x27F, 12)), (0x1F, 0x1F), "top and right");
    assert_eq!((c.at(0x100, 0x17), c.at(0, 12)), (0x10, 0x10), "bottom and left");
    assert_eq!(c.at(0, 0), 0x10, "the left line is drawn last");
}
