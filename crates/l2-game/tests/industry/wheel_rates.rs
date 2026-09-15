#![allow(unused_imports)]
use super::*;
use super::visual_effects::*;
use std::path::PathBuf;
use l2_game::game::Assets;
use l2_game::scenario;
use l2_game::screen::{Ctx, Screen};
use l2_game::screens::map::MapScreen;
use l2_kingdom::industry::MapToggle;
use l2_kingdom::tables::{Commodity, Tables};
use l2_mods::Platform;

///   another **from its first frame**. This used to say *"the England position's
///   five forests have produced nothing, so every wheel is on the 640 ms
///   rung"* — true of `Industry::new()`, which is what the importer gave every
///   county until `docs/decisions.md` C161, and false of the file.
///
/// **no expression in this test mentions
/// [`l2_view::campaign::industry_period_ms`]**, which is the constant being
/// ablated.
///
/// **Ablation.** Flatten `industry_period_ms`'s four arms to a single `640` and
/// the second half goes red on six of its eight outputs and the first half on
/// its ratio. Flattening it to `80` fails them the other way. Widening one band
/// edge by one — `n < 0x0A` to `n <= 0x0A` — fails exactly the `(10, 1)` row,
/// which is the row that edge exists for.
#[test]
fn a_busy_site_turns_its_wheel_eight_times_as_often_as_an_idle_one() {
    let (mut game, assets) = world!();

    assert_eq!(l2_game::TICK_MS, 16, "`EVERY` is `PULSE_MS` in 16 ms ticks");
    const TICKS_PER_GATE: u32 = 2;
    for (rung, ms) in PULSE_MS.iter().enumerate() {
        assert_eq!(ms / 20 * TICKS_PER_GATE, EVERY[rung], "rung {rung}");
        assert_eq!(TICKS % EVERY[rung], 0, "rung {rung} does not divide the window");
    }

    let sites = working_sites(&game);
    assert!(
        !sites.is_empty(),
        "England turn one has no working industry site, so this test cannot see a wheel"
    );
    let (tile, county, commodity) = idle_site(&game)
        .expect("no working site on England turn one records an idle season, so the slow rung is unseen");
    let &(busy_tile, busy_county, busy_commodity) = sites
        .iter()
        .find(|&&(_, id, c)| game.kingdom.counties[id].industry[c].output >= 0x32)
        .expect("no working site on England turn one records a busy season");

    let mut screen = MapScreen::new();
    let slow = turns_of_each_wheel(&mut screen, &mut game, &assets, TICKS);
    assert_eq!(
        slow[&tile],
        TICKS / EVERY[0],
        "an on-but-unproductive site is on the 640 ms rung: {} turns in {TICKS} ticks",
        TICKS / EVERY[0]
    );
    // **The first frame of a loaded game, from the file's own record.** Before
    // the importer carried `+0x2A0`/`+0x2A4`, this wheel sat on the slow rung
    // with every other until a season ended.
    assert_eq!(
        slow[&busy_tile],
        TICKS / EVERY[3],
        "county {busy_county}'s commodity {busy_commodity} records {} this season, so the loaded \
         game should turn its wheel on the 80 ms rung from the first frame",
        game.kingdom.counties[busy_county].industry[busy_commodity].output
    );

    game.kingdom.advance_season();
    let produced = game.kingdom.counties[county].industry[commodity].output;
    assert!(
        produced >= 0x32,
        "county {county} cut {produced} — the season did not reach the top band, so the \
         ratio below is not the one being claimed"
    );

    let mut screen = MapScreen::new();
    let fast = turns_of_each_wheel(&mut screen, &mut game, &assets, TICKS);
    assert_eq!(
        fast[&tile],
        TICKS / EVERY[3],
        "a site producing {produced} is on the 80 ms rung: {} turns in {TICKS} ticks",
        TICKS / EVERY[3]
    );
    assert_eq!(
        fast[&tile],
        slow[&tile] * 8,
        "eight rungs apart is eight times the turns — {} against {}",
        fast[&tile],
        slow[&tile]
    );

    for (output, band) in BAND_EDGES {
        let (mut game, assets) = world!();
        for &(_, id, c) in &working_sites(&game) {
            game.kingdom.counties[id].industry[c].output = output;
        }
        let mut screen = MapScreen::new();
        let turns = turns_of_each_wheel(&mut screen, &mut game, &assets, TICKS);
        assert_eq!(
            turns[&tile],
            TICKS / EVERY[band],
            "output {output} belongs on the {} ms rung, so the wheel should turn {} \
             times in {TICKS} ticks and it turned {}",
            PULSE_MS[band],
            TICKS / EVERY[band],
            turns[&tile]
        );
    }
}

/// **Ablation.** Delete the `if !working { … continue; }` guard in
/// `MapScreen::step_industry` and this goes red while the rate test stays green.
#[test]
fn a_site_the_player_switched_off_stops_turning() {
    let (mut game, assets) = world!();
    let sites = working_sites(&game);
    let (tile, county, commodity) = *sites.first().expect("a working site on the fixture");

    let mut screen = MapScreen::new();
    let before = turns_of_each_wheel(&mut screen, &mut game, &assets, TICKS);
    assert!(before[&tile] > 0, "the wheel was not turning before the switch was touched");

    // `Industry_ToggleFromMap` (`0x0043D309`), which is what a click on the site
    // reaches. It writes the enable byte *and* calls `Industry_UpdateSiteTile`.
    let on = game.kingdom.toggle_industry(county, MapToggle::Industry(Commodity::ALL[commodity]));
    assert!(!on, "the toggle should have switched county {county}'s site off");

    let mut screen = MapScreen::new();
    let after = turns_of_each_wheel(&mut screen, &mut game, &assets, TICKS);
    assert_eq!(
        after[&tile], 0,
        "a switched-off site turned {} times in {TICKS} ticks",
        after[&tile]
    );
}

