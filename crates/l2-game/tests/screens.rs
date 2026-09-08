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

use std::path::PathBuf;

use l2_game::game::{Assets, MAX_TAX_RATE};
use l2_game::input::{Event, Key};
use l2_game::scenario;
use l2_game::screen::{Ctx, Machine, Screen, ScreenId, Transition};
use l2_game::screens::county::{self as county, CountyScreen, Panel};
use l2_game::screens::map::{self, MapScreen};
use l2_game::Game;
use l2_kingdom::tables::Tables;
use l2_mods::Platform;
use l2_view::chrome;
use l2_view::{text, Canvas};

fn install() -> Option<PathBuf> {
    l2_testkit::install_dir()
}

macro_rules! world {
    () => {{
        let Some(dir) = install() else {
            l2_testkit::skip!("no game install, so there are no assets to draw with");
        };
        let platform = Platform::builder().base(&dir).build().expect("the install mounts");
        let assets = Assets::load(&platform.vfs).expect("assets load");
        // The **assets** come from the install and the **position** comes from
        // the named fixture. They used to come from the same place, and every
        // number below - fourteen counties, the treasury, the selected county -
        // is the England turn-one position's rather than any save's.
        let save = l2_testkit::england!();
        let game = scenario::from_save(&save, Tables::DEFAULT).expect("the fixture loads");
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

/// Every pixel of the map **viewport**, and which county it belongs to.
///
/// The viewport is the zoom's, not the screen's: `docs/screens.md` §1.4 — x
/// stops at 478 where the right panel starts, and y at 474 (near) or 408 (far).
fn pick_counts(screen: &MapScreen) -> [usize; 17] {
    let mut counts = [0usize; 17];
    let clip = screen.map_clip();
    for y in clip.y0..clip.y1 {
        for x in clip.x0..clip.x1 {
            let id = screen.county_at(x, y) as usize;
            if id < 17 {
                counts[id] += 1;
            }
        }
    }
    counts
}

/// Find a pixel belonging to a county, by scanning the pick plane rather than
/// hard-coding a coordinate a layout change would invalidate.
fn pixel_of(screen: &MapScreen, county: u8) -> Option<(i32, i32)> {
    let clip = screen.map_clip();
    (clip.y0..clip.y1)
        .flat_map(|y| (clip.x0..clip.x1).map(move |x| (x, y)))
        .find(|&(x, y)| screen.county_at(x, y) == county)
}

fn visible_counties(screen: &MapScreen) -> usize {
    pick_counts(screen)[1..=14].iter().filter(|&&c| c > 0).count()
}

/// **The screen the original draws is a window, not the whole map.**
///
/// This is the test the previous painter could not have passed: it drew all
/// 4,096 tiles at once, so every one of the fourteen counties was on screen at
/// once. `Map_SetZoom` gives the near view eight of the lattice's 65 columns,
/// so only a few counties can be — and the ones that are fill it.
#[test]
fn the_near_view_is_a_window_of_england_and_not_the_whole_map() {
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

    // `Map_InitMode` pins the opening viewport at row 0x4A, col 0x14, so this
    // is a fixed number rather than a range: **two** of England's fourteen
    // counties are on screen when the game opens.
    let counts = pick_counts(&screen);
    assert_eq!(visible_counties(&screen), 2, "eight lattice columns hold two counties, not 14");
    for (id, n) in counts.iter().enumerate().skip(15) {
        assert_eq!(*n, 0, "there is no county {id} on this map");
    }

    // Nothing outside the viewport is pickable, whatever the tag plane holds.
    assert_eq!(screen.county_at(map::PANEL.x, 200), 0, "the panel is not the map");
    assert_eq!(screen.county_at(200, map::TOP_BAR - 1), 0, "nor is the menu bar");
    assert_eq!(screen.county_at(200, 474), 0, "nor below the near viewport");
}

/// Zooming out reaches the rest of the map, and scrolling moves the near view.
/// Both halves matter: a viewport that could not move would be the minimap the
/// user complained about, in a smaller rectangle.
#[test]
fn zooming_out_shows_more_of_the_map_and_scrolling_moves_the_near_view() {
    let (mut game, assets) = world!();
    let mut screen = MapScreen::new();
    let before = draw(&mut screen, &mut game, &assets);
    let near_visible = visible_counties(&screen);

    send(&mut screen, &mut game, &assets, Event::KeyDown(Key::Char('Z')));
    let far = draw(&mut screen, &mut game, &assets);
    let far_visible = visible_counties(&screen);
    assert!(
        far_visible > near_visible,
        "the far view shows {far_visible} counties and the near one {near_visible}"
    );
    // At the far zoom's pinned origin all fourteen are reachable, which is why
    // the original disables scrolling there rather than leaving it stranded.
    assert_eq!(far_visible, 14);
    assert!(before.diff_count(&far) > 10_000, "and it is a different picture");

    // Back in, and now scroll. One step is one map tile, so the origin moves by
    // exactly one lattice column and the picture must change.
    send(&mut screen, &mut game, &assets, Event::KeyDown(Key::Char('Z')));
    let home = screen.viewport();
    let a = draw(&mut screen, &mut game, &assets);
    send(&mut screen, &mut game, &assets, Event::KeyDown(Key::Right));
    assert_eq!(screen.viewport().col, home.col + 1);
    let b = draw(&mut screen, &mut game, &assets);
    assert!(a.diff_count(&b) > 10_000, "scrolling one column must repaint the map");
}

/// A click picks the county the player can actually see at that pixel, and a
/// second click on the same county opens it. The pixel is found through the
/// pick plane, so this exercises exactly the path the mouse takes.
#[test]
fn clicking_a_county_selects_it_and_clicking_it_again_opens_its_panel() {
    let (mut game, assets) = world!();
    let mut screen = MapScreen::new();
    draw(&mut screen, &mut game, &assets);

    // Whichever county the opening viewport happens to show most of.
    let counts = pick_counts(&screen);
    let target = (1..=14u8).max_by_key(|&id| counts[id as usize]).expect("a county is visible");
    assert!(counts[target as usize] > 0, "the opening view shows at least one county");
    let (px, py) = pixel_of(&screen, target).expect("and it has a pixel");

    game.select(0);
    let t = send(&mut screen, &mut game, &assets, Event::Click { x: px, y: py });
    assert_eq!(t, Transition::Stay, "the first click only selects");
    assert_eq!(game.selected, target);

    let t = send(&mut screen, &mut game, &assets, Event::Click { x: px, y: py });
    assert_eq!(t, Transition::Push(ScreenId::County(target)), "the second click opens it");

    // A click on the sea clears the selection rather than picking a county at
    // random.
    let (sx, sy) = pixel_of(&screen, 0).expect("there is sea");
    send(&mut screen, &mut game, &assets, Event::Click { x: sx, y: sy });
    assert_eq!(game.selected, 0);
}

/// The minimap is the original's own raster out of `MAPnn.PL8`, and clicking it
/// selects the county under the pixel *and* moves the viewport onto it.
///
/// This is the one place we use the original's algorithm and not just reach its
/// answer: `Minimap_Click` reads the same county byte out of the same file.
#[test]
fn clicking_the_minimap_selects_that_county_and_brings_it_into_view() {
    let (mut game, assets) = world!();
    let minimap = assets.minimap(game.map_slot).expect("Map01.pl8 holds slot 0");

    let mut screen = MapScreen::new();
    draw(&mut screen, &mut game, &assets);
    let visible = pick_counts(&screen);

    // A minimap pixel of a county that is *not* in the opening view, so "the
    // map moved onto it" cannot pass by accident.
    let (mx, my) = (0..128)
        .flat_map(|y| (0..128).map(move |x| (x, y)))
        .map(|(x, y)| (x + chrome::MINIMAP_HIT_X, y + chrome::MINIMAP_HIT_Y))
        .find(|&(x, y)| {
            let c = minimap.county_at(x, y);
            c != 0 && (c as usize) < 17 && visible[c as usize] == 0
        })
        .expect("some county is off screen at the opening viewport");
    let county = minimap.county_at(mx, my);

    let before = screen.viewport();
    send(&mut screen, &mut game, &assets, Event::Click { x: mx, y: my });
    assert_eq!(game.selected, county, "the raster decides which county");
    assert_ne!(screen.viewport(), before, "and the map moves");

    // Having moved, that county is now on screen — which is what "centred"
    // means, and is not implied by the origin merely changing.
    draw(&mut screen, &mut game, &assets);
    assert!(pick_counts(&screen)[county as usize] > 0, "county {county} is now in view");
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

/// The menu bar reads the clock and the treasury out of the world, and the
/// right column reads the selected county. Checked by finding the actual
/// digits, at the coordinates `Screen_DrawMenuBar` puts them.
#[test]
fn the_map_chrome_shows_the_clock_the_treasury_and_the_selected_county() {
    let (mut game, assets) = world!();
    let mut screen = MapScreen::new();
    game.select(8);
    let canvas = draw(&mut screen, &mut game, &assets);
    let ink = &assets.ink;

    let clock = find_text(&canvas, "WINTER 1268", ink.text).expect("the season and the year");
    assert_eq!(clock, (360, 6), "the original draws the clock at x 360, y 6");
    let gold = find_text(&canvas, "GOLD 1000", ink.text).expect("the treasury");
    assert_eq!(gold, (500, 6), "and the treasury at x 500");
    assert!(find_text(&canvas, "TURN 1", ink.dim).is_some());
    assert!(find_text(&canvas, "COUNTIES 1/14", ink.dim).is_some());

    // The right column, inside Misc_cty frame 55.
    let county = find_text(&canvas, "COUNTY 8", ink.highlight).expect("the selected county");
    assert!(county.0 >= map::PANEL.x, "it is drawn in the right column, not on the map");
    assert!(find_text(&canvas, "435", ink.text).is_some(), "the population");

    // The near-misses. If the search could match anything it would match these.
    assert!(find_text(&canvas, "WINTER 1269", ink.text).is_none());
    assert!(find_text(&canvas, "GOLD 1001", ink.text).is_none());
    assert!(find_text(&canvas, "436", ink.text).is_none());
}

/// The county strip shows what `CountyStrip_Draw` puts in the 162 × 94 plate,
/// at the coordinates it puts them: population at (508, 189), happiness ending
/// at 602 on the same line, and the tax rate at (506, 226).
///
/// The exact coordinates are the point. A panel that merely *contained* the
/// right digits somewhere would pass a looser test and still be laid out
/// wrongly, which is the mistake this whole task exists to correct.
#[test]
fn the_county_strip_shows_the_saves_numbers_where_the_original_puts_them() {
    let (mut game, assets) = world!();
    let mut screen = CountyScreen::new(8);
    let canvas = draw(&mut screen, &mut game, &assets);
    let ink = &assets.ink;

    let c = &game.kingdom.counties[8];
    assert_eq!((c.population, c.happiness, c.ration_achieved), (435, 72, 3));

    assert_eq!(
        find_text(&canvas, "435", ink.text),
        Some((508, 189)),
        "the population, at Ui_DrawNumber(pop, ' ', ..., 0x1FC, 0xBD)"
    );
    assert_eq!(
        find_text(&canvas, "72", ink.text),
        Some((602 - text::width("72"), 189)),
        "the happiness, right-anchored at 0x25A on the same line"
    );
    assert_eq!(find_text(&canvas, "0%", ink.text), Some((506, 226)), "the tax rate at 0x1FA");
    assert!(find_text(&canvas, "COUNTY 8", ink.text).is_some());
    assert!(find_text(&canvas, "TAX", ink.dim).is_some(), "L2.eng group 61 index 0");
    assert!(find_text(&canvas, "RATION", ink.dim).is_some(), "and index 1");
    // rationAchieved == rationWanted, so it is drawn plain rather than red.
    assert!(find_text(&canvas, "NORMAL", ink.text).is_some());
    assert!(find_text(&canvas, "NORMAL", ink.bad).is_none());

    // Near misses, one per number, so none of the three can match by accident.
    assert!(find_text(&canvas, "436", ink.text).is_none());
    assert!(find_text(&canvas, "73", ink.text).is_none());
    assert!(find_text(&canvas, "DOUBLE", ink.text).is_none());
}

/// The population panel is `Ui_DrawBox(0x10, 0x30, 0x1C, 0x17)` with its rows
/// at the y coordinates `Panel_Population` draws them, and it is reached from
/// the strip's top-left quadrant and from nowhere else.
#[test]
fn the_population_panel_opens_from_its_own_quadrant_and_lays_out_where_it_should() {
    let (mut game, assets) = world!();
    let mut screen = CountyScreen::new(8);
    let ink = &assets.ink;

    let hot = Panel::Population.strip_hotspot();
    send(&mut screen, &mut game, &assets, Event::Click { x: hot.centre_x(), y: hot.y + 4 });
    assert_eq!(screen.panel(), Panel::Population);

    let canvas = draw(&mut screen, &mut game, &assets);
    assert_eq!(
        find_text(&canvas, "LAST SEASON", ink.text),
        Some((48, 266)),
        "group 73 index 1, at Eng_DrawString(0x49, 1, 0x30, 0x10A)"
    );
    assert_eq!(
        find_text(&canvas, "417", ink.text),
        Some((336 - text::width("417"), 266)),
        "and its value right-anchored at 0x150"
    );
    assert_eq!(find_text(&canvas, "BIRTHS", ink.dim), Some((48, 298)), "0x12A");
    assert_eq!(find_text(&canvas, "DEATHS", ink.dim), Some((48, 314)), "0x13A");
    assert_eq!(find_text(&canvas, "ARMY", ink.dim), Some((48, 330)), "0x14A");
    assert_eq!(find_text(&canvas, "THIS SEASON", ink.highlight), Some((48, 386)), "0x182");
    assert!(find_text(&canvas, "435", ink.highlight).is_some(), "this season's population");

    // The graph is a labelled stub, because g_countyHistory is not simulated.
    assert!(find_text(&canvas, "NOT SIMULATED", ink.bad).is_some());

    // And the tax panel is gone, which is what "one panel at a time" means.
    assert!(find_text(&canvas, "TAX RATE", ink.dim).is_none());
}

/// A zero row draws **no number at all** — `Ui_DrawDelta(value, 0, ...)`
/// returns before it formats anything. This is the detail a reimplementation
/// gets wrong by printing `0`, so it is asserted both ways round.
#[test]
fn a_zero_delta_row_draws_its_label_and_no_number() {
    let (mut game, assets) = world!();
    let ink = &assets.ink;
    let mut screen = CountyScreen::new(8);
    screen.open(Panel::Population);

    game.kingdom.counties[8].births = 0;
    let blank = draw(&mut screen, &mut game, &assets);
    assert!(find_text(&blank, "BIRTHS", ink.dim).is_some(), "the label is still drawn");
    assert!(find_text(&blank, "+0", ink.good).is_none(), "and nothing beside it");
    assert!(find_text(&blank, "0", ink.good).is_none());

    game.kingdom.counties[8].births = 63;
    let filled = draw(&mut screen, &mut game, &assets);
    assert_eq!(
        find_text(&filled, "+63", ink.good),
        Some((336 - text::width("+63"), 298)),
        "a non-zero row draws a signed number in the value column"
    );
    assert!(blank.diff_count(&filled) > 0, "and the two frames differ");
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

    // Down moves to the ration panel, and Right there does not touch the tax.
    send(&mut screen, &mut game, &assets, Event::KeyDown(Key::Down));
    assert_eq!(screen.panel(), Panel::Ration);
    send(&mut screen, &mut game, &assets, Event::KeyDown(Key::Right));
    assert_eq!(game.kingdom.counties[8].tax_rate, 7);
    assert_eq!(game.kingdom.counties[8].ration_wanted, 4);
}

/// **The tax ceiling is 50, and it is the original's.**
///
/// `Tax_Increase` guards `taxRate < 0x32`; `g_taxHappinessOther` has exactly
/// 51 entries. `docs/screens-county.md` §6.3. The screen stops there and the
/// number on it stops there too — a clamp that the picture disagreed with
/// would be a clamp the player cannot see.
#[test]
fn the_tax_rate_stops_at_the_originals_own_ceiling_of_fifty() {
    let (mut game, assets) = world!();
    let mut screen = CountyScreen::new(8);
    let up = Panel::Tax.increase_button().expect("the tax panel has an up arrow");

    for _ in 0..60 {
        send(&mut screen, &mut game, &assets, Event::Click { x: up.centre_x(), y: up.y + 4 });
    }
    assert_eq!(game.kingdom.counties[8].tax_rate, MAX_TAX_RATE);
    assert_eq!(MAX_TAX_RATE, 50, "and the constant is the reading, not a round number");

    let canvas = draw(&mut screen, &mut game, &assets);
    assert!(find_text(&canvas, "50%", assets.ink.highlight).is_some());
    assert!(find_text(&canvas, "51%", assets.ink.highlight).is_none(), "a near miss");
    assert!(find_text(&canvas, "60%", assets.ink.highlight).is_none());
}

/// The ration panel's slider is the third order the original's county panels
/// give, and the one we never had: `Ration_SliderClick` jumps the split to
/// `mouseX - 224`, and the two caps step it by one.
#[test]
fn the_ration_split_slider_sets_the_field_the_original_sets() {
    let (mut game, assets) = world!();
    let mut screen = CountyScreen::new(8);
    screen.open(Panel::Ration);

    let track = county::split_track();
    send(&mut screen, &mut game, &assets, Event::Click { x: track.x + 37, y: track.y + 8 });
    assert_eq!(game.kingdom.counties[8].ration_split, 37, "the track jumps to mouseX - 224");

    let down = county::split_down_button();
    send(&mut screen, &mut game, &assets, Event::Click { x: down.centre_x(), y: down.y + 8 });
    assert_eq!(game.kingdom.counties[8].ration_split, 36, "the left cap steps down one");

    let up = county::split_up_button();
    for _ in 0..3 {
        send(&mut screen, &mut game, &assets, Event::Click { x: up.centre_x(), y: up.y + 8 });
    }
    assert_eq!(game.kingdom.counties[8].ration_split, 39);

    // The knob follows: two different splits must not draw the same picture.
    let a = draw(&mut screen, &mut game, &assets);
    game.kingdom.counties[8].ration_split = 90;
    let b = draw(&mut screen, &mut game, &assets);
    assert!(a.diff_count(&b) > 0, "the knob moves with the value");
}

/// County 1 belongs to realm 5. The strip says so, all four panels still open,
/// and every order is refused — by the mouse as well as by the keyboard.
#[test]
fn another_realms_county_can_be_looked_at_and_not_ordered() {
    let (mut game, assets) = world!();
    assert_eq!(game.kingdom.counties[1].owner, 5);

    let mut screen = CountyScreen::new(1);
    let canvas = draw(&mut screen, &mut game, &assets);
    assert!(find_text(&canvas, "REALM 5", assets.ink.text).is_some());
    assert!(find_text(&canvas, "NOT YOURS", assets.ink.bad).is_some());
    assert!(find_text(&canvas, "REALM 4", assets.ink.text).is_none(), "a near miss");

    send(&mut screen, &mut game, &assets, Event::KeyDown(Key::Right));
    let up = Panel::Tax.increase_button().expect("the arrows are still drawn");
    send(&mut screen, &mut game, &assets, Event::Click { x: up.centre_x(), y: up.y + 4 });
    assert_eq!(game.kingdom.counties[1].tax_rate, 0, "no order lands on another realm's county");

    screen.open(Panel::Ration);
    let track = county::split_track();
    send(&mut screen, &mut game, &assets, Event::Click { x: track.x + 40, y: track.y + 8 });
    assert_eq!(game.kingdom.counties[1].ration_split, 0, "and neither does the slider");
}

/// End turn, from the map, with the mouse — and the numbers move on screen.
///
/// This is the whole slice in one test: a England turn-one scenario, a real map, a click
/// on a button, `l2-kingdom`'s season pipeline, and the changed numbers read
/// back off the canvas.
#[test]
fn ending_the_turn_from_the_map_moves_the_numbers_and_the_screen_follows() {
    let (mut game, assets) = world!();
    let mut screen = MapScreen::new();
    game.select(8);
    let before = draw(&mut screen, &mut game, &assets);
    assert!(find_text(&before, "WINTER 1268", assets.ink.text).is_some());
    assert!(find_text(&before, "TURN 1", assets.ink.dim).is_some());

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
    assert!(find_text(&after, "TURN 2", assets.ink.dim).is_some());
    assert!(find_text(&after, "WINTER 1268", assets.ink.text).is_none());
    assert!(before.diff_count(&after) > 0);

    let shown = game.kingdom.counties[8].population.to_string();
    assert!(
        find_text(&after, &shown, assets.ink.text).is_some(),
        "the right column shows the new population, {shown}"
    );
    assert!(
        find_text(&after, &population.to_string(), assets.ink.text).is_none(),
        "and not the old one"
    );
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

/// Not a test: a way to look at the screen. `cargo test -p l2-game --test
/// screens shoot -- --ignored` writes raw RGBA into `out/`, which `.gitignore`
/// excludes. Renders of the game's own artwork are derived assets and must
/// never be committed (CLAUDE.md rule 1).
#[test]
#[ignore]
fn shoot() {
    let (mut game, assets) = world!();
    std::fs::create_dir_all("out").unwrap();
    let mut screen = MapScreen::new();
    for (name, zoomed) in [("near", false), ("far", true)] {
        if zoomed {
            send(&mut screen, &mut game, &assets, Event::KeyDown(Key::Char('Z')));
        }
        let canvas = draw(&mut screen, &mut game, &assets);
        let mut rgba = vec![0u8; 640 * 480 * 4];
        canvas.to_rgba(&assets.palette, &mut rgba);
        let rgb: Vec<u8> = rgba.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect();
        std::fs::write(format!("out/campaign_{name}.rgb"), &rgb).unwrap();
    }
}
