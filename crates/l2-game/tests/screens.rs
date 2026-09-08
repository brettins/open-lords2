//! The three screens, drawn and driven against a real install — headlessly.
//!
//! ```text
//! LORDS2_DIR="F:\games\Lords of the Realm II" cargo test -p l2-game
//! ```
//!
//! **No window is opened.** Every assertion is on the canvas's `Vec<u8>` of
//! palette indices, which is the same shape the eventual pixel diff against
//! `Lords2.exe`'s framebuffer will take. A screen that had to be looked at to
//! be checked would be a screen that stops being checked.
//!
//! Several tests below read text back off the canvas with [`find_text`], which
//! renders the string it is looking for and searches for that exact pattern of
//! ink. It is paired every time with a near-miss that must *not* be found, so
//! "the panel shows 435" cannot pass by finding some other number.

use std::env;
use std::path::PathBuf;

use l2_game::game::Assets;
use l2_game::input::{Event, Key};
use l2_game::scenario;
use l2_game::screen::{Ctx, Machine, Screen, ScreenId, Transition};
use l2_game::screens::county::CountyScreen;
use l2_game::screens::map::{self, MapScreen};
use l2_game::Game;
use l2_kingdom::tables::Tables;
use l2_mods::Platform;
use l2_view::{text, Canvas};

fn install() -> Option<PathBuf> {
    env::var("LORDS2_DIR").ok().map(PathBuf::from).filter(|d| d.is_dir())
}

macro_rules! world {
    () => {{
        let Some(dir) = install() else {
            eprintln!("LORDS2_DIR not set - skipping");
            return;
        };
        let platform = Platform::builder().base(&dir).build().expect("the install mounts");
        let assets = Assets::load(&platform.vfs).expect("assets load");
        let game =
            scenario::load(&platform.vfs, Tables::DEFAULT).expect("the shipped scenario loads");
        (game, assets)
    }};
}

fn draw<S: Screen>(screen: &mut S, game: &mut Game, assets: &Assets) -> Canvas {
    let mut canvas = Canvas::screen();
    let ctx = Ctx { game, assets };
    screen.draw(&ctx, &mut canvas);
    canvas
}

fn send<S: Screen>(screen: &mut S, game: &mut Game, assets: &Assets, e: Event) -> Transition {
    let mut ctx = Ctx { game, assets };
    screen.handle(e, &mut ctx)
}

/// Find a string drawn in `colour`, returning its top-left. Only the glyphs'
/// *set* pixels are matched; what is behind the letters is the panel's
/// business.
fn find_text(canvas: &Canvas, s: &str, colour: u8) -> Option<(i32, i32)> {
    let w = text::width(s);
    let h = text::GLYPH_H;
    let mut wanted: Vec<(i32, i32)> = Vec::new();
    let mut probe = Canvas::new(w.max(1) as usize, h as usize);
    text::draw(&mut probe, 0, 0, s, 1);
    for y in 0..h {
        for x in 0..w {
            if probe.at(x as usize, y as usize) == 1 {
                wanted.push((x, y));
            }
        }
    }
    if wanted.is_empty() {
        return None;
    }
    for oy in 0..(canvas.height as i32 - h) {
        for ox in 0..(canvas.width as i32 - w) {
            // Cheap rejection on the first ink pixel before the full compare.
            let (fx, fy) = wanted[0];
            if canvas.at((ox + fx) as usize, (oy + fy) as usize) != colour {
                continue;
            }
            if wanted.iter().all(|(x, y)| canvas.at((ox + x) as usize, (oy + y) as usize) == colour)
            {
                return Some((ox, oy));
            }
        }
    }
    None
}

/// Every pixel of the map area, and which county it belongs to.
fn pick_counts(screen: &MapScreen) -> [usize; 17] {
    let mut counts = [0usize; 17];
    for y in map::TOP_BAR..map::BOTTOM_BAR_Y {
        for x in 0..640 {
            let id = screen.county_at(x, y) as usize;
            if id < 17 {
                counts[id] += 1;
            }
        }
    }
    counts
}

/// The map is drawn from the shipped tile sets and every one of the fourteen
/// counties ends up pickable, with a sane share of the screen each.
#[test]
fn the_campaign_map_draws_england_and_all_fourteen_counties_can_be_picked() {
    let (mut game, assets) = world!();
    let mut screen = MapScreen::new();
    let canvas = draw(&mut screen, &mut game, &assets);

    // Real artwork, not a flat fill: the shipped banks use a lot of the palette.
    let mut used = [false; 256];
    for &p in &canvas.pixels {
        used[p as usize] = true;
    }
    let distinct = used.iter().filter(|u| **u).count();
    assert!(distinct > 32, "only {distinct} palette entries in the whole frame");

    let counts = pick_counts(&screen);
    for id in 1..=14usize {
        // The smallest county on the England map holds 91 tiles, and a 10 x 6
        // diamond is about 30 pixels, so nothing real can come out under 1,500
        // even after the tiles drawn in front of it take their bites.
        assert!(counts[id] > 1_500, "county {id} covers only {} pixels", counts[id]);
    }
    for id in 15..17usize {
        assert_eq!(counts[id], 0, "there is no county {id} on this map");
    }
    // 1,675 of the map's 4,096 tiles carry a county id, so the counties cannot
    // cover more than about 1,675 x 30 = 50,250 pixels, and should not fall far
    // short of it: what is missing is the overlap of the tiles drawn in front.
    let land: usize = counts[1..=14].iter().sum();
    assert!((40_000..=50_250).contains(&land), "the counties cover {land} pixels");
    assert!(counts[0] > 100_000, "and most of the screen belongs to no county");
}

/// A click picks the county the player can actually see at that pixel, and a
/// second click on the same county opens it. The pixel is found through the
/// pick plane, so this exercises exactly the path the mouse takes.
#[test]
fn clicking_a_county_selects_it_and_clicking_it_again_opens_its_panel() {
    let (mut game, assets) = world!();
    let mut screen = MapScreen::new();
    draw(&mut screen, &mut game, &assets);

    // A pixel belonging to county 11, chosen by scanning rather than by
    // hard-coding a coordinate that a change in layout would invalidate.
    let target = 11u8;
    let (px, py) = (map::TOP_BAR..map::BOTTOM_BAR_Y)
        .flat_map(|y| (0..640).map(move |x| (x, y)))
        .find(|&(x, y)| screen.county_at(x, y) == target)
        .expect("county 11 is somewhere on the map");

    game.select(0);
    let t = send(&mut screen, &mut game, &assets, Event::Click { x: px, y: py });
    assert_eq!(t, Transition::Stay, "the first click only selects");
    assert_eq!(game.selected, target);

    let t = send(&mut screen, &mut game, &assets, Event::Click { x: px, y: py });
    assert_eq!(t, Transition::Push(ScreenId::County(target)), "the second click opens it");

    // A click on the sea clears the selection rather than picking a county at
    // random.
    let (sx, sy) = (map::TOP_BAR..map::BOTTOM_BAR_Y)
        .flat_map(|y| (0..640).map(move |x| (x, y)))
        .find(|&(x, y)| screen.county_at(x, y) == 0)
        .expect("there is sea");
    send(&mut screen, &mut game, &assets, Event::Click { x: sx, y: sy });
    assert_eq!(game.selected, 0);
}

/// The selection is visible: outlining a county changes the picture, and
/// outlining a different one changes it differently.
#[test]
fn the_selected_county_is_outlined_on_the_map() {
    let (mut game, assets) = world!();
    let mut screen = MapScreen::new();

    game.select(0);
    let none = draw(&mut screen, &mut game, &assets);
    game.select(8);
    let eight = draw(&mut screen, &mut game, &assets);
    game.select(11);
    let eleven = draw(&mut screen, &mut game, &assets);

    assert!(none.diff_count(&eight) > 100, "an outline must be visible");
    assert!(eight.diff_count(&eleven) > 100, "and it must follow the selection");
    assert!(
        eight.count(assets.ink.highlight) > none.count(assets.ink.highlight),
        "the outline is drawn in the highlight colour"
    );
}

/// The top bar reads the clock and the treasury out of the world, and the
/// bottom bar reads the selected county. Checked by finding the actual digits.
#[test]
fn the_map_bars_show_the_clock_the_treasury_and_the_selected_county() {
    let (mut game, assets) = world!();
    let mut screen = MapScreen::new();
    game.select(8);
    let canvas = draw(&mut screen, &mut game, &assets);
    let ink = &assets.ink;

    assert!(find_text(&canvas, "WINTER 1268", ink.text).is_some(), "the season and the year");
    assert!(find_text(&canvas, "TURN 1", ink.text).is_some());
    assert!(find_text(&canvas, "GOLD 1000", ink.text).is_some());
    assert!(find_text(&canvas, "COUNTIES 1/14", ink.dim).is_some());
    assert!(find_text(&canvas, "COUNTY 8", ink.text).is_some());
    assert!(find_text(&canvas, "POP 435", ink.text).is_some());

    // The near-misses. If the search could match anything it would match these.
    assert!(find_text(&canvas, "WINTER 1269", ink.text).is_none());
    assert!(find_text(&canvas, "GOLD 1001", ink.text).is_none());
    assert!(find_text(&canvas, "POP 436", ink.text).is_none());
}

/// The county panel shows the county's own numbers — the ones the season
/// pipeline will read next turn, off the same record.
#[test]
fn the_county_panel_shows_the_numbers_the_save_holds() {
    let (mut game, assets) = world!();
    let mut screen = CountyScreen::new(8);
    let canvas = draw(&mut screen, &mut game, &assets);
    let ink = &assets.ink;

    let c = &game.kingdom.counties[8];
    assert_eq!((c.population, c.happiness, c.herd), (435, 72, 101));

    assert!(find_text(&canvas, "COUNTY 8", ink.highlight).is_some());
    assert!(find_text(&canvas, "YOUR COUNTY", ink.text).is_some());
    // The population is up 18 on last season, so it is drawn in the "moved the
    // right way" colour rather than the plain one — which is itself part of
    // what the panel is for.
    assert!(find_text(&canvas, "435", ink.good).is_some(), "the population, and it rose");
    assert!(find_text(&canvas, "435", ink.text).is_none(), "so it is not the plain colour");
    assert!(find_text(&canvas, "417", ink.text).is_some(), "last season's population");
    assert!(find_text(&canvas, "101", ink.text).is_some(), "the herd");
    assert!(find_text(&canvas, "NORMAL", ink.text).is_some(), "the ration level");
    assert!(find_text(&canvas, "0%", ink.highlight).is_some(), "the focused tax row");

    assert!(find_text(&canvas, "436", ink.good).is_none(), "a near miss must not match");
    assert!(find_text(&canvas, "DOUBLE", ink.text).is_none());
}

/// Setting the tax rate changes the record *and* what is on the screen. Both
/// halves matter: a panel that showed a number it did not set, or set a number
/// it did not show, would pass one of them alone.
#[test]
fn setting_the_tax_rate_changes_the_county_and_the_picture() {
    let (mut game, assets) = world!();
    let mut screen = CountyScreen::new(8);
    let before = draw(&mut screen, &mut game, &assets);

    for _ in 0..7 {
        send(&mut screen, &mut game, &assets, Event::KeyDown(Key::Right));
    }
    assert_eq!(game.kingdom.counties[8].tax_rate, 7);

    let after = draw(&mut screen, &mut game, &assets);
    assert!(before.diff_count(&after) > 0);
    assert!(find_text(&after, "7%", assets.ink.highlight).is_some());
    assert!(find_text(&after, "0%", assets.ink.highlight).is_none());

    // Down moves to the rations row, and Right there does not touch the tax.
    send(&mut screen, &mut game, &assets, Event::KeyDown(Key::Down));
    send(&mut screen, &mut game, &assets, Event::KeyDown(Key::Right));
    assert_eq!(game.kingdom.counties[8].tax_rate, 7);
    assert_eq!(game.kingdom.counties[8].ration_wanted, 4);
}

/// County 1 belongs to realm 5. The panel opens, shows its numbers, and refuses
/// every order — by the mouse as well as by the keyboard.
#[test]
fn another_realms_county_can_be_looked_at_and_not_ordered() {
    let (mut game, assets) = world!();
    assert_eq!(game.kingdom.counties[1].owner, 5);

    let mut screen = CountyScreen::new(1);
    let canvas = draw(&mut screen, &mut game, &assets);
    assert!(find_text(&canvas, "REALM 5", assets.ink.text).is_some());
    assert!(find_text(&canvas, "NOT YOURS", assets.ink.bad).is_some());

    send(&mut screen, &mut game, &assets, Event::KeyDown(Key::Right));
    let more = CountyScreen::more_button(0);
    send(&mut screen, &mut game, &assets, Event::Click { x: more.centre_x(), y: more.y + 4 });
    assert_eq!(game.kingdom.counties[1].tax_rate, 0, "no order lands on another realm's county");
}

/// End turn, from the map, with the mouse — and the numbers move on screen.
///
/// This is the whole slice in one test: a shipped scenario, a real map, a click
/// on a button, `l2-kingdom`'s season pipeline, and the changed numbers read
/// back off the canvas.
#[test]
fn ending_the_turn_from_the_map_moves_the_numbers_and_the_screen_follows() {
    let (mut game, assets) = world!();
    let mut screen = MapScreen::new();
    game.select(8);
    let before = draw(&mut screen, &mut game, &assets);
    assert!(find_text(&before, "WINTER 1268", assets.ink.text).is_some());
    assert!(find_text(&before, "TURN 1", assets.ink.text).is_some());

    let population = game.kingdom.counties[8].population;
    let gold = game.gold();

    let b = map::END_TURN_BUTTON;
    send(&mut screen, &mut game, &assets, Event::Click { x: b.centre_x(), y: b.y + 4 });

    assert_eq!(game.kingdom.turn_count, 2, "one season ran");
    assert_eq!(game.kingdom.season, 1, "Winter gave way to Spring");
    assert_ne!(game.kingdom.counties[8].population, population, "the county changed");
    assert_eq!(game.kingdom.counties[8].pop_last, population);
    assert_eq!(game.gold_last[game.player as usize], gold, "and the treasury is remembered");

    let after = draw(&mut screen, &mut game, &assets);
    assert!(find_text(&after, "SPRING 1268", assets.ink.text).is_some(), "the clock moved");
    assert!(find_text(&after, "TURN 2", assets.ink.text).is_some());
    assert!(find_text(&after, "WINTER 1268", assets.ink.text).is_none());
    assert!(before.diff_count(&after) > 0);

    let shown = format!("POP {}", game.kingdom.counties[8].population);
    assert!(find_text(&after, &shown, assets.ink.text).is_some(), "the bottom bar shows {shown}");
}

/// The turn also runs through the machine, from the keyboard, with the county
/// panel's own numbers following. Four turns, so a season wrap is included.
#[test]
fn four_turns_run_through_the_machine_and_the_panel_keeps_up() {
    let (mut game, assets) = world!();
    let mut m = Machine::new(ScreenId::Campaign);
    let mut ctx_seasons = Vec::new();
    for _ in 0..4 {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        m.handle(Event::KeyDown(Key::Char('E')), &mut ctx);
        ctx_seasons.push((game.kingdom.season, game.kingdom.year));
    }
    assert_eq!(ctx_seasons, vec![(1, 1268), (2, 1268), (3, 1268), (4, 1269)]);
    assert_eq!(game.turns_played, 4);

    let mut panel = CountyScreen::new(8);
    let canvas = draw(&mut panel, &mut game, &assets);
    let shown = game.kingdom.counties[8].population.to_string();
    assert!(
        find_text(&canvas, &shown, assets.ink.text).is_some()
            || find_text(&canvas, &shown, assets.ink.good).is_some()
            || find_text(&canvas, &shown, assets.ink.bad).is_some(),
        "the panel shows the population it now has ({shown})"
    );
}
