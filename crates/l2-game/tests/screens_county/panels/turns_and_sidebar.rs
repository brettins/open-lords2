#![allow(unused_imports)]
use super::*;
use super::population::*;
use super::tax::*;
use super::rations::*;
use super::*;
use super::strip_and_sidebar::*;
use super::drawing_and_emboss::*;
use super::produce_and_pastures::*;
use super::layout_and_labels::*;
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
fn another_realms_county_can_be_looked_at_and_not_ordered() {
    let (mut game, assets) = world!();
    assert_eq!(game.kingdom.counties[1].owner, 5);

    let mut screen = CountyScreen::new(1, Panel::Tax);
    let canvas = draw(&mut screen, &mut game, &assets);
    // The name at (480, 180)
    // 165, then group 15's two lines and the owner at 240 / 260 / 280 — all
    // four in the body font and in the owning realm's own colour.
    //
    // **The pen is the realm's shield colour**, `g_realmColour[shield]`, which
    // is what `CountyStrip_Draw` passes for all three lines. Not `Ink::realm`
    // and not the realm id — see `sovereign_lines` below and C62.
    let shield = game.kingdom.realms[5].shield_index;
    let realm5 = l2_view::chrome::realm_pen(shield).expect("realm 5 flies a shield");
    let lord5 = game.player_names[5].as_str().to_owned();
    let lord4 = game.player_names[4].as_str().to_owned();
    assert!(!lord5.is_empty() && lord5 != lord4, "the fixture names its lords: {lord5:?}");
    let name = county::county_name(&Ctx { game: &mut game, assets: &assets }, 1);
    assert_eq!(
        find_body(&canvas, &assets, &name, STRIP_INK).map(|p| p.1),
        Some(180),
        "the name sits lower on the 162 x 274 plate"
    );
    // **The third line is the lord, out of the game's own two sources.** It
    // read `REALM 5` — ours, invented — until the strip was put on
    // `message::lord_name` with the court and the battle prompt: the save's
    // player table (`Game::player_names`) first, then `L2.eng` group 7 by the
    // realm's **lord**, which is the pair `Game_NewGame` seeds the array from.
    let lord = l2_game::screens::message::lord_name(&Ctx { game: &mut game, assets: &assets }, 5);
    assert_eq!(lord, lord5, "the shared accessor answers realm 5 out of the save's table");
    assert!(!lord.starts_with("REALM "), "realm 5's lord is named, and it said {lord:?}");
    assert!(find_body(&canvas, &assets, &lord, realm5).is_some(), "the strip names {lord:?}");
    assert!(find_body(&canvas, &assets, &lord4, realm5).is_none(), "a near miss");
    assert!(find_body(&canvas, &assets, "REALM 5", realm5).is_none(), "and not a name of ours");
    assert!(find_body(&canvas, &assets, "693", STRIP_INK).is_none(), "no numbers at all");

    send(&mut screen, &mut game, &assets, Event::KeyDown(Key::Right));
    let up = Panel::Tax.increase_button().expect("the arrows are still drawn");
    send(&mut screen, &mut game, &assets, Event::Click { x: up.centre_x(), y: up.y + 4 });
    assert_eq!(game.kingdom.counties[1].tax_rate, 0, "no order lands on another realm's county");

    screen.open(Panel::Ration);
    let track = county::split_track();
    send(&mut screen, &mut game, &assets, Event::Click { x: track.x + 40, y: track.y + 8 });
    assert_eq!(game.kingdom.counties[1].ration_split, 0, "and neither does the slider");
}

#[test]
fn ending_the_turn_from_the_map_moves_the_numbers_and_the_screen_follows() {
    let (mut game, assets) = world!();
    game.prefs.debug_overlay = true;
    let mut screen = MapScreen::new();
    game.select(8);
    let before = draw(&mut screen, &mut game, &assets);
    assert!(find_body(&before, &assets, " 1268 ", font::TEXT).is_some());
    assert!(find_body(&before, &assets, "Winter", font::TEXT).is_some());
    assert!(find_text(&before, "TURN 1", assets.ink.dim).is_some());

    let population = game.kingdom.counties[8].population;
    let gold = game.gold();

    let b = map::END_TURN_BUTTON;
    send(&mut screen, &mut game, &assets, Event::Click { x: b.centre_x(), y: b.y + 4 });
    assert_eq!(
        game.kingdom.turn_count, 1,
        "the click starts the turn; it does not run it inside the click",
    );
    run_turn(&mut screen, &mut game, &assets);

    assert_eq!(game.kingdom.turn_count, 2, "one season ran");
    assert_eq!(game.kingdom.season, 1, "Winter gave way to Spring");
    assert_ne!(game.kingdom.counties[8].population, population, "the county changed");
    assert_eq!(game.kingdom.counties[8].pop_last, population);
    assert_eq!(game.gold_last[game.player as usize], gold, "and the treasury is remembered");

    let after = draw(&mut screen, &mut game, &assets);
    assert!(find_body(&after, &assets, "Spring", font::TEXT).is_some(), "the clock moved");
    assert!(find_text(&after, "TURN 2", assets.ink.dim).is_some());
    assert!(find_body(&after, &assets, "Winter", font::TEXT).is_none());
    assert!(before.diff_count(&after) > 0);

    let shown = game.kingdom.counties[8].population.to_string();
    assert!(
        find_strip(&after, &assets, &shown, STRIP_INK).is_some(),
        "the right column shows the new population, {shown}"
    );
    assert!(
        find_strip(&after, &assets, &population.to_string(), STRIP_INK).is_none(),
        "and not the old one"
    );
}

#[test]
fn four_turns_run_through_the_machine_and_the_panel_keeps_up() {
    let (mut game, assets) = world!();
    let mut m = Machine::new(ScreenId::Campaign);
    let mut ctx_seasons = Vec::new();
    for _ in 0..4 {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        m.handle(Event::KeyDown(Key::Char('E')), &mut ctx);
        let before = game.kingdom.turn_count;
        let mut done_at = None;
        for n in 1..2_000u32 {
            let mut ctx = Ctx { game: &mut game, assets: &assets };
            m.update(&mut ctx);
            if done_at.is_none() && game.kingdom.turn_count > before {
                done_at = Some(n);
            }
            if done_at.is_some_and(|t| n >= t + l2_view::fade::PHASES as u32) {
                break;
            }
        }
        ctx_seasons.push((game.kingdom.season, game.kingdom.year));
    }
    assert_eq!(ctx_seasons, vec![(1, 1268), (2, 1268), (3, 1268), (4, 1269)]);
    assert_eq!(game.turns_played, 4);

    let mut panel = CountyScreen::new(8, Panel::Tax);
    let canvas = draw(&mut panel, &mut game, &assets);
    let shown = game.kingdom.counties[8].population.to_string();
    assert!(
        find_strip(&canvas, &assets, &shown, STRIP_INK).is_some()
            || find_text(&canvas, &shown, assets.ink.good).is_some()
            || find_text(&canvas, &shown, assets.ink.bad).is_some(),
        "the panel shows the population it now has ({shown})"
    );
}

/// The second half is the half a person would never think to check, and it is
/// The reason this is a test: the `0x05`
/// (banding) and `0x06` (carrying) arms test **no sidebar guard at all**, so
/// the sidebar is dead for as long as a peasant is in the air. Making
/// all three behave alike would look like a tidy-up and would be wrong. C59.
#[test]
fn the_sidebar_slider_still_works_with_the_village_open_but_not_mid_drag() {
    let (mut game, assets) = world!();
    let county = (1..=game.kingdom.county_count as u8)
        .find(|&id| game.is_players(id))
        .expect("the player holds a county");
    game.select(county);
    let share = |g: &Game| g.kingdom.counties[county as usize].industry_share;

    let mut m = Machine::new(ScreenId::Campaign);
    m.push(ScreenId::Village(county));
    assert_eq!(m.depth(), 2, "the village is an inset over the map, not a replacement");

    {
        let mut c = Ctx { game: &mut game, assets: &assets };
        m.handle(Event::Click { x: 561, y: 270 }, &mut c);
    }
    assert_eq!(share(&game), 60, "the sidebar's slider is dead with the village open");
    assert_eq!(m.ids(), vec![ScreenId::Campaign, ScreenId::Village(county)], "and it stayed open");

    // Held and moved, still through the village: `FUN_00439122` runs on the
    // level and the movement, so this is the drag continuing.
    {
        let mut c = Ctx { game: &mut game, assets: &assets };
        m.handle(Event::Pointer { x: 541, y: 270 }, &mut c);
    }
    assert_eq!(share(&game), 20, "the drag tracks the pointer over the village too");

    let before = game.selected;
    {
        let mut c = Ctx { game: &mut game, assets: &assets };
        m.handle(Event::Click { x: 200, y: 200 }, &mut c);
    }
    assert_eq!(game.selected, before, "a click on the map round the inset is not a map click");

    let mut screen = VillageScreen::new(county);
    let top = {
        let ctx = Ctx { game: &mut game, assets: &assets };
        VillageScreen::top_y(&ctx)
    };
    send(&mut screen, &mut game, &assets, Event::Click { x: 100, y: top + 60 });
    send(&mut screen, &mut game, &assets, Event::Pointer { x: 140, y: top + 100 });
    assert_eq!(screen.phase(), village_screen::Phase::Band, "the band is up");
    assert_eq!(
        send(&mut screen, &mut game, &assets, Event::Click { x: 561, y: 270 }),
        Transition::Stay,
        "screen 0x05 tests no sidebar guard, so the click must not pass to the map"
    );
}


