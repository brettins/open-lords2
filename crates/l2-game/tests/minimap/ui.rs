#![allow(unused_imports)]
use super::*;
use super::helpers::*;
use super::overlays::*;
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

/// **The four buttons are not radio buttons.** `Minimap_ModeButton` takes a
/// mode button only while the overlay is off, and once it is on the fourth
/// button turns the overlay off. So going from
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

    // The fourth button leaves the overlay.
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

    // Every pixel the two frames paint — a blitter copies only
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

