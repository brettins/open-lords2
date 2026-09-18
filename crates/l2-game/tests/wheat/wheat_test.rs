#![allow(unused_imports)]
use super::*;
use super::helpers::*;
use std::path::PathBuf;
use l2_formats::maps::Plane;
use l2_game::game::Assets;
use l2_game::input::{Event, Key};
use l2_game::scenario;
use l2_game::screen::{Ctx, Machine, Screen, ScreenId};
use l2_game::screens::map::MapScreen;
use l2_game::Game;
use l2_kingdom::field::FieldType;
use l2_kingdom::map::{coords, index, MAP_DIM, MAP_TILES};
use l2_kingdom::tables::Tables;
use l2_mods::Platform;
use l2_view::{campaign, Canvas};

#[test]
fn a_sown_field_draws_the_originals_wheat_frame_in_every_season_of_a_year() {
    let (mut game, assets) = world!();
    game.kingdom.options.exploration = true;
    let county = game.selected;
    assert!(game.is_players(county), "the fixture opens on the player's county");

    // Sow: every fallow field to grain, through the brush's own handler
    // (`Field_SetType`, `0x00438BEC`).
    let fallow: Vec<usize> = game
        .kingdom
        .field_tiles(county as usize)
        .into_iter()
        .filter(|(_, kind)| *kind == FieldType::Fallow)
        .map(|(tile, _)| tile)
        .collect();
    assert!(!fallow.is_empty(), "the player's county has fallow to sow");
    // Everything the picture reads — the crop words, `+0x206`, the terrain
    // byte — is still written by the brush and the four End Turns below.
    let store = game.kingdom.counties[county as usize].grain;
    let imported = game.kingdom.counties[county as usize].fields_grain_standing;
    game.kingdom.counties[county as usize].grain = 10_000;
    for &tile in &fallow {
        game.kingdom.paint_field(county as usize, tile, FieldType::Grain).expect("a fallow field takes grain");
    }
    eprintln!(
        "sown: the fixture's store was {store} and its +0x206 {imported}; {} fallow fields to grain; {} on the grain fields",
        fallow.len(),
        game.kingdom.counties[county as usize].labour[l2_kingdom::tables::JOB_GRAIN_FARMING],
    );
    let probe = a_quiet_tile(&game, &fallow);
    let first_grain = (0..MAP_TILES)
        .find(|&t| {
            let map = &game.kingdom.campaign.map;
            map.county[t] == county && map.flags[t] & 0x20 != 0 && (2..=0x0E).contains(&map.terrain[t])
        })
        .expect("a grain tile");

    let elsewhere = game.kingdom.county_ids().find(|&id| !game.is_players(id as u8)).expect("a county not the player's");

    let mut machine = Machine::new(ScreenId::Campaign);
    let mut year = Vec::new();
    for _ in 0..4 {
        end_turn(&mut machine, &mut game, &assets);
        let season = game.kingdom.season;
        let c = &game.kingdom.counties[county as usize];
        // **`+0x206` is pinned from what this file sowed, not read back.** The
        // sowing arm writes `fieldsGrain`, or `1` on a shortfall, and nothing in
        // this year destroys a field. Reading our own `fields_grain_standing`
        // here was the first version, and deleting the line that writes it left
        // this test green: the expectation fell to variant 0 with the picture.
        let sown = if c.sow_shortfall { 1 } else { fallow.len() as i32 };
        assert_eq!(c.fields_grain_standing, sown, "season {season}: +0x206 is the fields sown");
        let want = originals_variant(season, c.crop, sown, c.sow_shortfall, probe == first_grain);
        let was = c124_variant(season, c.crop, c.fields_grain);
        eprintln!(
            "season {season}: grain {} crop {:?} fieldsGrain {} +0x206 {} shortfall {} terrain {:#04X} -> original variant {want}, C124 drew {was}",
            c.grain, c.crop, c.fields_grain, c.fields_grain_standing, c.sow_shortfall,
            game.kingdom.campaign.map.terrain[probe],
        );

        let selected = game.selected;
        game.selected = elsewhere as u8;
        let (near, near_distinct) = drawn_variant(&mut game, &assets, probe, false);
        let (far, far_distinct) = drawn_variant(&mut game, &assets, probe, true);
        game.selected = selected;
        eprintln!("  near draws {near:?} (distinct {near_distinct}), far draws {far:?} (distinct {far_distinct})");

        assert!(near_distinct, "season {season}: the four wheat frames are four pictures at the near zoom");
        assert_eq!(near, Some(want), "season {season}: the near zoom draws the original's variant");
        if far_distinct {
            assert_eq!(far, Some(want), "season {season}: the far zoom draws the original's variant");
        } else {
            assert!(far.is_some(), "season {season}: the far zoom draws a wheat frame");
        }
        year.push((season, want, was));
    }
    assert_eq!(year.iter().map(|y| y.0).collect::<Vec<_>>(), [1, 2, 3, 4], "a whole growing year");
    assert!(
        year.iter().any(|&(_, want, was)| want != was),
        "and the year includes a season C124's reading draws wrongly: {year:?}"
    );
}

