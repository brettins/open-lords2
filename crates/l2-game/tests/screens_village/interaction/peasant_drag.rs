#![allow(unused_imports)]
use super::*;
use super::job_popups::*;
use super::navigation::*;
use super::*;
use super::render::*;
use super::animation::*;
use common::*;
use l2_game::input::Event;
use l2_game::input::Key;
use l2_game::screen::Ctx;
use l2_game::screen::Machine;
use l2_game::screen::ScreenId;
use l2_game::screen::Transition;
use l2_game::screens::map;
use l2_game::screens::village::{self as village_screen};
use l2_game::screens::village::VillageScreen;
use l2_view::chrome;
use l2_view::village;
use l2_view::Canvas;

#[test]
fn a_drag_across_the_real_drop_grid_moves_the_county_s_peasants() {
    let (mut game, assets) = world!();
    let county = (1..=game.kingdom.county_count as u8)
        .find(|&id| game.is_players(id))
        .expect("the player holds a county");
    let mut screen = VillageScreen::new(county);

    let c = &game.kingdom.counties[county as usize];
    let slots = VillageScreen::slots(c);
    let icons = VillageScreen::icons(c);
    let from = (0..village::CLUSTER_COUNT)
        .max_by_key(|&i| icons[i].iter().filter(|&&v| v != 0).count())
        .unwrap();
    let to = (0..village::CLUSTER_COUNT).find(|&i| slots[i] != slots[from]).unwrap();
    let (before_from, before_to) = (c.labour[slots[from]], c.labour[slots[to]]);
    assert!(before_from > 0, "cluster {from} has people in it");

    let (bx0, by0, bx1, by1) = village::cluster_band_box(from, village::SCENE_Y);
    send(&mut screen, &mut game, &assets, Event::Click { x: bx0, y: by0 });
    send(&mut screen, &mut game, &assets, Event::Pointer { x: bx1, y: by1 });
    send(&mut screen, &mut game, &assets, Event::Release { x: bx1, y: by1 });
    assert_eq!(screen.phase(), village_screen::Phase::Carry, "released holding a selection");
    let carried = screen.drag_count();
    assert!(carried > 0);

    let (ox, oy) = village::cluster_origin(to, village::SCENE_Y);
    send(&mut screen, &mut game, &assets, Event::Click { x: ox + 36, y: oy + 24 });
    assert_eq!(screen.phase(), village_screen::Phase::Idle, "and put it down");

    let c = &game.kingdom.counties[county as usize];
    let moved = before_from - c.labour[slots[from]];
    assert!(moved > 0, "somebody moved");
    assert_eq!(c.labour[slots[to]] - before_to, moved, "and they arrived");
    assert_eq!(
        moved,
        (carried * c.pop_band).min(before_from),
        "an icon is popBand people, clamped to what the job held"
    );
    assert_eq!(
        c.labour.iter().sum::<i32>(),
        game.kingdom.counties[county as usize].population,
        "and the nine still sum to the population"
    );
}

/// **A right click during the village's drag gesture does not leave the
/// village.** `docs/arms.json` `0x0042FF10/carry-right-cancels`.
#[test]
fn a_right_click_during_a_peasant_drag_cancels_the_drag_and_not_the_village() {
    let (mut game, assets) = world!();
    game.select(8);
    let top = 64;

    let mut m = over_the_map(ScreenId::Village(8));
    send_stack(&mut m, &mut game, &assets, Event::Click { x: 100, y: top + 60 });
    send_stack(&mut m, &mut game, &assets, Event::Pointer { x: 160, y: top + 110 });
    send_stack(&mut m, &mut game, &assets, Event::RightClick { x: 160, y: top + 110 });
    assert_eq!(
        m.top_id(),
        Some(ScreenId::Village(8)),
        "0x05 has no right-button arm at all, so the click is swallowed"
    );

    send_stack(&mut m, &mut game, &assets, Event::Release { x: 160, y: top + 110 });
    send_stack(&mut m, &mut game, &assets, Event::RightClick { x: 160, y: top + 110 });

    // The idle village, by contrast, leaves on the right button - which is the
    // ablation: if the fix were "the village never leaves on a right click",
    // this would fail.
    let mut idle = over_the_map(ScreenId::Village(8));
    send_stack(&mut idle, &mut game, &assets, Event::RightClick { x: 160, y: top + 110 });
    assert_eq!(
        idle.top_id(),
        Some(ScreenId::Campaign),
        "the idle village really does leave on a right click"
    );
}


