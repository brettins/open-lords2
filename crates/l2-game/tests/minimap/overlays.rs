#![allow(unused_imports)]
use super::*;
use super::helpers::*;
use super::ui::*;
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

#[test]
fn the_happiness_overlay_recolours_the_minimap_from_the_rating_ramp() {
    let (mut game, assets, raster) = world!();
    let ids = hand_the_player_everything(&mut game);
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

    click(&mut screen, &mut game, &assets, 2);
    let happy = land_colours(&draw(&mut screen, &mut game, &assets), &raster, &mine);

    assert!(expect.len() >= 2, "the fixture must spread over several bands");
    assert_eq!(happy, expect, "mode 3 draws the rating ramp and nothing else");
    assert_ne!(owners, happy, "and the two modes are not the same picture");
    eprintln!("minimap: owners {owners:?}, happiness {happy:?}");
}

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

