//! **The minimap's four modes, counted in pixels.**
//!
//! ```text
//! LORDS2_DIR="F:\games\Lords of the Realm II" \
//! LORDS2_FIXTURES="E:\dev\lords2-fixtures" cargo test -p l2-game --test minimap
//! ```
//!
//! The claim these tests exist to settle is a visual one — *"the three
//! statistic modes recolour the minimap, and from a different table than the
//! ownership mode"* — so it is turned into a number the canvas can answer: **the
//! set of distinct palette indices drawn over the county land pixels of the
//! user's own `Map01.pl8`.**
//!
//! Sampling the *land* pixels rather than the whole 128 × 128 rectangle is what
//! makes the sets exact. The panel artwork behind the minimap, and the sea, are
//! full of the same greys the realm ramp uses, so a naive rectangle sweep
//! reports colours nothing in this code path drew — it did, and the first
//! version of this file failed on `0x2F` and `0x32` coming out of `Misc_cty`
//! frame 54 rather than out of any ramp.
//!
//! Shade 10 is skipped for the same reason: it is the shade the selected county
//! has replaced with `0x20`, and leaving it in would put a colour in every set
//! that has nothing to do with the mode.
//!
//! `Minimap_DrawOverlay` (`0x00410CBD`) and `FUN_00451BBA` are the two functions
//! under test; `docs/screens.md` §3.2 describes them.

use std::collections::BTreeSet;
use std::path::PathBuf;

use l2_game::game::Assets;
use l2_game::input::Event;
use l2_game::scenario;
use l2_game::screen::{Ctx, Screen};
use l2_game::screens::map::{MapScreen, MINIMAP_MODE_BUTTONS};
use l2_game::Game;
use l2_kingdom::county::LABOUR_NO_FLOOR;
use l2_kingdom::tables::Tables;
use l2_kingdom::tables::{JOB_COUNT, JOB_GRAIN_FARMING, JOB_IDLE_TOWNSFOLK};
use l2_mods::Platform;
use l2_view::chrome::{
    self, Minimap, MinimapMode, MINIMAP_DIM, MINIMAP_RATING_RAMP, MINIMAP_REALM_RAMP,
    MINIMAP_SELECTED, MINIMAP_X, MINIMAP_Y,
};
use l2_view::Canvas;

fn install() -> Option<PathBuf> {
    l2_testkit::install_dir()
}

macro_rules! world {
    () => {{
        let Some(dir) = install() else {
            l2_testkit::skip!("no game install, so there is no minimap raster to draw");
        };
        let platform = Platform::builder().base(&dir).build().expect("the install mounts");
        let assets = Assets::load(&platform.vfs).expect("assets load");
        let save = l2_testkit::england!();
        let game = scenario::from_save(&save, Tables::DEFAULT).expect("the fixture loads");
        let raster = assets.minimap(game.map_slot).expect("the slot has a MAPnn.PL8");
        (game, assets, raster)
    }};
}

fn draw(screen: &mut MapScreen, game: &mut Game, assets: &Assets) -> Canvas {
    let mut canvas = Canvas::screen();
    let ctx = Ctx { game, assets };
    screen.draw(&ctx, &mut canvas);
    canvas
}

/// Click one of the four buttons in the strip beside the minimap.
fn click(screen: &mut MapScreen, game: &mut Game, assets: &Assets, button: usize) {
    let r = MINIMAP_MODE_BUTTONS[button];
    let mut ctx = Ctx { game, assets };
    screen.handle(Event::Click { x: r.x + 2, y: r.y + 2 }, &mut ctx);
}

/// The distinct colours drawn over the land pixels of the counties in `want` —
/// shades 11..=13 only, so the selected county's `0x20` never enters.
fn land_colours(canvas: &Canvas, m: &Minimap, want: &BTreeSet<u8>) -> BTreeSet<u8> {
    let mut seen = BTreeSet::new();
    for y in 0..MINIMAP_DIM {
        for x in 0..MINIMAP_DIM {
            let i = (y * MINIMAP_DIM + x) as usize;
            if !(11..=13).contains(&m.shades[i]) || !want.contains(&m.counties[i]) {
                continue;
            }
            seen.insert(canvas.at((MINIMAP_X + x) as usize, (MINIMAP_Y + y) as usize));
        }
    }
    seen
}

/// The counties in the raster whose `band` — one of the three ratings — is `b`.
fn with_band(
    game: &Game,
    m: &Minimap,
    band: fn(l2_kingdom::county::MinimapBands) -> u8,
    b: u8,
) -> BTreeSet<u8> {
    counties_in_raster(m)
        .into_iter()
        .filter(|&c| band(game.kingdom.counties[c as usize].minimap_bands()) == b)
        .collect()
}

/// Every county id that actually has land pixels in this raster.
fn counties_in_raster(m: &Minimap) -> Vec<u8> {
    let mut ids: Vec<u8> = m
        .counties
        .iter()
        .zip(&m.shades)
        .filter(|(_, &s)| (11..=13).contains(&s))
        .map(|(&c, _)| c)
        .collect();
    ids.sort_unstable();
    ids.dedup();
    ids.retain(|&c| c != 0);
    ids
}

/// Give the local player every county, and put each one in a known state so a
/// band is reached on purpose rather than by whatever the fixture holds.
fn hand_the_player_everything(game: &mut Game) -> Vec<usize> {
    let ids: Vec<usize> = (1..=game.kingdom.county_count as usize).collect();
    for &id in &ids {
        let player = game.player;
        let c = &mut game.kingdom.counties[id];
        c.owner = player;
        // Fed, and neither short of workers nor carrying any slack, so every
        // rating but the one a test sets is the "draw nothing" band 6.
        c.ration_achieved = 3;
        c.ration_wanted = 3;
        c.labour = [0; JOB_COUNT];
        c.labour_wanted = [LABOUR_NO_FLOOR; JOB_COUNT];
        c.labour_useful = [0; JOB_COUNT];
    }
    ids
}

/// **Mode 0 against mode 3, colour for colour.**
///
/// With the player holding everything, the ownership tint over county land is
/// exactly one row of the realm ramp; the happiness tint is exactly the rating
/// ramp entries the counties' happiness bands select. Both sets are asserted
/// whole, not merely for overlap.
#[test]
fn the_happiness_overlay_recolours_the_minimap_from_the_rating_ramp() {
    let (mut game, assets, raster) = world!();
    let ids = hand_the_player_everything(&mut game);
    // 0, 20, 40, 60, 80, 100, 0, ... — bands 0..=5 and round again.
    for (n, &id) in ids.iter().enumerate() {
        game.kingdom.counties[id].happiness = ((n % 6) * 20) as i32;
    }
    let mine: BTreeSet<u8> = counties_in_raster(&raster).into_iter().collect();
    let expect: BTreeSet<u8> = mine
        .iter()
        .map(|&c| {
            let band = game.kingdom.counties[c as usize].minimap_bands().happiness;
            MINIMAP_RATING_RAMP[band as usize]
        })
        .collect();
    let mut screen = MapScreen::new();

    let owners = land_colours(&draw(&mut screen, &mut game, &assets), &raster, &mine);
    let row = chrome::realm_colour(game.realm_colour[game.player as usize]) as usize;
    let want: BTreeSet<u8> = MINIMAP_REALM_RAMP[row][1..].iter().copied().collect();
    assert_eq!(owners, want, "mode 0 draws realm ramp row {row} and nothing else");

    // Button 3 of four — `Minimap_ModeButton` hotspot id 3, the heart.
    click(&mut screen, &mut game, &assets, 2);
    let happy = land_colours(&draw(&mut screen, &mut game, &assets), &raster, &mine);

    assert!(expect.len() >= 2, "the fixture must spread over several bands");
    assert_eq!(happy, expect, "mode 3 draws the rating ramp and nothing else");
    assert_ne!(owners, happy, "and the two modes are not the same picture");
    eprintln!("minimap: owners {owners:?}, happiness {happy:?}");
}

/// **The food overlay is binary, and half of it draws nothing.**
///
/// `FUN_00451BBA` gives a county band 0 when the ration it achieved fell short
/// of the ration asked for, and **6 — outside the six-entry ramp — when it did
/// not**, so a fed county keeps the raster's own shade. That is the original's,
/// not a gap in ours, and this is the test that says so.
#[test]
fn the_food_overlay_marks_only_the_counties_that_went_short() {
    let (mut game, assets, raster) = world!();
    let ids = hand_the_player_everything(&mut game);
    for (n, &id) in ids.iter().enumerate() {
        game.kingdom.counties[id].ration_achieved = if n % 2 == 0 { 1 } else { 3 };
    }
    let short = with_band(&game, &raster, |b| b.food, 0);
    let fed = with_band(&game, &raster, |b| b.food, 6);
    assert!(!short.is_empty() && !fed.is_empty(), "both cases must occur");

    let mut screen = MapScreen::new();
    draw(&mut screen, &mut game, &assets);
    click(&mut screen, &mut game, &assets, 1);
    let canvas = draw(&mut screen, &mut game, &assets);

    assert_eq!(
        land_colours(&canvas, &raster, &short),
        BTreeSet::from([MINIMAP_RATING_RAMP[0]]),
        "a county that went short is the ramp's worst colour, flat"
    );
    assert_eq!(
        land_colours(&canvas, &raster, &fed),
        BTreeSet::from([11u8, 12, 13]),
        "a fed county is band 6 and keeps the raster's own shades"
    );
}

/// **The labour overlay only ever paints the two ends of the ramp.** Band 0 is
/// a county short of farm workers, band 5 one with idle townsfolk or a
/// over-staffed job, and band 6 — nothing drawn — one with neither.
#[test]
fn the_labour_overlay_paints_only_the_two_ends_of_the_ramp() {
    let (mut game, assets, raster) = world!();
    let ids = hand_the_player_everything(&mut game);
    for (n, &id) in ids.iter().enumerate() {
        let c = &mut game.kingdom.counties[id];
        match n % 3 {
            0 => {
                c.labour_wanted[JOB_GRAIN_FARMING] = 10;
                c.labour[JOB_GRAIN_FARMING] = 1;
            }
            1 => c.labour[JOB_IDLE_TOWNSFOLK] = 25,
            _ => {}
        }
    }
    // The three cases really are the three bands, so the picture below is a
    // statement about labour and not only about the ramp.
    let bands: BTreeSet<u8> = ids
        .iter()
        .map(|&id| game.kingdom.counties[id].minimap_bands().labour)
        .collect();
    assert_eq!(bands, BTreeSet::from([0u8, 5, 6]), "0 short, 5 slack, 6 neither");
    let by_band: Vec<BTreeSet<u8>> =
        [0u8, 5, 6].iter().map(|&b| with_band(&game, &raster, |x| x.labour, b)).collect();

    let mut screen = MapScreen::new();
    draw(&mut screen, &mut game, &assets);
    click(&mut screen, &mut game, &assets, 0);
    let canvas = draw(&mut screen, &mut game, &assets);

    assert_eq!(
        land_colours(&canvas, &raster, &by_band[0]),
        BTreeSet::from([MINIMAP_RATING_RAMP[0]])
    );
    assert_eq!(
        land_colours(&canvas, &raster, &by_band[1]),
        BTreeSet::from([MINIMAP_RATING_RAMP[5]])
    );
    assert_eq!(
        land_colours(&canvas, &raster, &by_band[2]),
        BTreeSet::from([11u8, 12, 13]),
        "band 6 draws nothing"
    );
}

/// **The four buttons are not radio buttons.** `Minimap_ModeButton` takes a
/// mode button only while the overlay is off, and once it is on the fourth
/// button turns the overlay off instead of toggling the zoom. So going from
/// food to happiness takes three clicks, not one — and the pixels say which
/// mode is up.
#[test]
fn a_mode_button_does_nothing_while_another_mode_is_up() {
    let (mut game, assets, raster) = world!();
    hand_the_player_everything(&mut game);
    let mine: BTreeSet<u8> = counties_in_raster(&raster).into_iter().collect();
    let mut screen = MapScreen::new();

    let owners = land_colours(&draw(&mut screen, &mut game, &assets), &raster, &mine);
    let zoom_at_rest = *screen.zoom();

    // Into food, where every county is fed: the whole map is band 6, so the
    // land goes back to the bare raster.
    click(&mut screen, &mut game, &assets, 1);
    let food = land_colours(&draw(&mut screen, &mut game, &assets), &raster, &mine);
    assert_eq!(food, BTreeSet::from([11u8, 12, 13]), "the owner tint is gone");
    assert_ne!(food, owners);

    // Pressing happiness now must do nothing at all.
    click(&mut screen, &mut game, &assets, 2);
    assert_eq!(
        land_colours(&draw(&mut screen, &mut game, &assets), &raster, &mine),
        food,
        "a mode button is ignored while a mode is up"
    );

    // The fourth button leaves the overlay instead of zooming.
    click(&mut screen, &mut game, &assets, 3);
    assert_eq!(
        land_colours(&draw(&mut screen, &mut game, &assets), &raster, &mine),
        owners,
        "button 4 returns to the ownership tint"
    );
    assert_eq!(*screen.zoom(), zoom_at_rest, "and does not toggle the zoom on the way");

    // Now that the overlay is off, the same button *is* the zoom toggle.
    click(&mut screen, &mut game, &assets, 3);
    assert_ne!(*screen.zoom(), zoom_at_rest, "button 4 in mode 0 is the zoom");
}

/// The selected county's brightest shade is `0x20` **in every mode**: the
/// original tests the selection before it dispatches on the mode, so the marker
/// survives an overlay.
#[test]
fn the_selected_county_keeps_its_marker_in_every_mode() {
    let (mut game, assets, raster) = world!();
    hand_the_player_everything(&mut game);
    let with_land = counties_in_raster(&raster);
    game.select(with_land[0]);
    let selected = with_land[0];

    let mut screen = MapScreen::new();
    for button in [None, Some(0usize), Some(1), Some(2)] {
        if let Some(b) = button {
            click(&mut screen, &mut game, &assets, b);
        }
        let canvas = draw(&mut screen, &mut game, &assets);
        // Shade 10 of the selected county, which `land_colours` deliberately
        // skips, is the one the marker replaces.
        let mut marked = 0;
        for y in 0..MINIMAP_DIM {
            for x in 0..MINIMAP_DIM {
                let i = (y * MINIMAP_DIM + x) as usize;
                if raster.shades[i] == 10 && raster.counties[i] == selected {
                    assert_eq!(
                        canvas.at((MINIMAP_X + x) as usize, (MINIMAP_Y + y) as usize),
                        MINIMAP_SELECTED,
                        "mode {button:?}: the selection marker"
                    );
                    marked += 1;
                }
            }
        }
        assert!(marked > 0, "county {} has shade-10 pixels", with_land[0]);
        if button.is_some() {
            click(&mut screen, &mut game, &assets, 3);
        }
    }
}

/// The mode strip and the mode badge are the original's own `Misc_cty` frames
/// and they are drawn: `0x5C` at (611, 32) with no overlay, `0x5B` in its place
/// with one, and the mode's badge at (485, 30).
#[test]
fn the_mode_strip_and_badge_are_drawn_beside_the_minimap() {
    let (mut game, assets, _raster) = world!();
    hand_the_player_everything(&mut game);
    let art = assets.chrome.as_ref().expect("the install has Misc_cty.pl8");
    let mut screen = MapScreen::new();

    let before = draw(&mut screen, &mut game, &assets);
    click(&mut screen, &mut game, &assets, 2);
    let after = draw(&mut screen, &mut game, &assets);

    // Every pixel the two frames actually paint — a blitter copies only
    // non-zero indices, so "painted" is "not 0" on a blank canvas.
    let mut alone = Canvas::screen();
    assert!(art.draw_minimap_side(&mut alone, MinimapMode::Happiness));
    assert!(art.draw_minimap_badge(&mut alone, MinimapMode::Happiness));

    let mut checked = 0;
    let mut changed = 0;
    for y in 0..480usize {
        for x in 0..640usize {
            let want = alone.at(x, y);
            if want == 0 {
                continue;
            }
            assert_eq!(after.at(x, y), want, "({x}, {y}) is the frame's own pixel");
            checked += 1;
            if before.at(x, y) != want {
                changed += 1;
            }
        }
    }
    assert!(checked > 1000, "both frames drew: {checked} pixels");
    assert!(changed > 100, "switching mode redrew the strip: {changed} pixels differ");
}
