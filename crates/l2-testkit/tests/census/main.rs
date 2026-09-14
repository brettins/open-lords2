//! **The census of install-gated tests**, so that a silent skip stops being
//! the failure mode.
//!
//! # The problem
//!
//! `cargo test --workspace` on this machine and `cargo test --workspace` with
//! `LORDS2_DIR` and `LORDS2_FIXTURES` pointing nowhere — which is what CI does —
//! print **the same** `N passed; 0 failed`, whatever `N` is that week. The two
//! runs assert wildly different amounts and are indistinguishable from their
//! output, because a gated test that finds no game prints a line to stderr and
//! returns green.
//!
//! The gap is [`GATED_TOTAL`] tests, which this file names.
//!
//! Nobody notices a test that stops existing. The reproduction against a real
//! save, the renderer against real sprites, the scenario against the England
//! fixture — the project's strongest evidence — did not run on CI at all, and
//! nothing said so.
//!
//! # The mechanism
//!
//! Every gate in the workspace goes through a macro in `l2-testkit`. This test
//! reads the source of every test file, counts the gated test functions per
//! file and per gate, and asserts the result against [`INVENTORY`] below. It
//! runs everywhere, needs no game, and cannot itself be skipped.
//!
//! So: **add a gated test tomorrow and this goes red** until the inventory is
//! updated, which is a one-line diff that makes the new gate visible in the
//! history. Remove a gate and it goes red the same way. The number is small
//! enough to argue with, which is the point — 91 of 946 test functions were
//! install-gated before anybody counted, and 42 of them resolved the install
//! through a hard-coded path copied between files.
//!
//! It also prints, on every run, how many of those gates the current
//! environment satisfies. A run that asserted a third of what it looks like it
//! asserted now says so.

mod scanner;
pub use scanner::*;
mod assertions;
pub use assertions::*;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Which gate a test sits behind, in the order a test body is searched. The
/// order is the precedence: a test that takes both the fixture and the install
/// is counted against the fixture, because that is the stronger requirement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Gate {
    /// `l2_testkit::england!()` — needs the England turn-one fixture.
    England,
    /// `l2_testkit::fixture!(..)` — needs one named save from `LORDS2_FIXTURES`.
    Fixture,
    /// `l2_testkit::saves!()` — needs at least one save from anywhere.
    Saves,
    /// `l2_testkit::executable!()` — needs `Lords2.exe`.
    Executable,
    /// `l2_testkit::install!()`, or a local helper built on
    /// `l2_testkit::install_dir()`.
    Install,
    /// A `l2_testkit::skip!` on some other condition — a file missing from an
    /// install that is otherwise present, say. Counted, because a skip is a
    /// skip.
    Other,
}

impl Gate {
    fn name(self) -> &'static str {
        match self {
            Gate::England => "england",
            Gate::Fixture => "fixture",
            Gate::Saves => "saves",
            Gate::Executable => "executable",
            Gate::Install => "install",
            Gate::Other => "other",
        }
    }

    /// Is this gate satisfied in the environment the test process is running
    /// in? `Other` cannot be decided from outside the test, so it is `None`.
    fn satisfied(self) -> Option<bool> {
        match self {
            Gate::England => {
                Some(matches!(l2_testkit::england_turn1(), l2_testkit::FixtureState::Ready(_)))
            }
            Gate::Fixture => Some(l2_testkit::fixtures_dir().is_some()),
            Gate::Saves => Some(!l2_testkit::every_available_save().is_empty()),
            Gate::Executable => Some(l2_testkit::executable().is_some()),
            Gate::Install => Some(l2_testkit::install_dir().is_some()),
            Gate::Other => None,
        }
    }
}

/// **The inventory. Update it deliberately.**
///
/// `(file, gate, number of gated `#[test]` functions)`, sorted. A line here is
/// a statement that this many tests in this file do not run without that
/// input.
const INVENTORY: &[(&str, &str, usize)] = &[
    ("crates/l2-formats/tests/battle_fixtures.rs", "fixture", 3),
    ("crates/l2-formats/tests/corpus.rs", "install", 5),
    ("crates/l2-formats/tests/maps.rs", "install", 6),
    ("crates/l2-formats/tests/save/realms_and_counties/county_records.rs", "saves", 3),
    ("crates/l2-formats/tests/save/realms_and_counties/realms_and_map.rs", "saves", 6),
    ("crates/l2-formats/tests/save/structure.rs", "executable", 1),
    ("crates/l2-formats/tests/save/structure.rs", "saves", 4),
    ("crates/l2-formats/tests/save/units_and_merchants.rs", "saves", 6),
    ("crates/l2-formats/tests/save_england_turn1/counties.rs", "england", 9),
    ("crates/l2-formats/tests/save_england_turn1/globals.rs", "england", 1),
    ("crates/l2-formats/tests/save_england_turn1/main.rs", "england", 1),
    ("crates/l2-formats/tests/save_england_turn1/merchants.rs", "england", 2),
    ("crates/l2-game/src/audio/mod.rs", "install", 1),
    ("crates/l2-game/src/screens/menubar/tests_part.rs", "install", 2),
    ("crates/l2-game/tests/ai_war/simulation.rs", "england", 2),
    ("crates/l2-game/tests/armoury/animation/rendering.rs", "england", 3),
    ("crates/l2-game/tests/armoury/animation/sheet_validation.rs", "england", 3),
    ("crates/l2-game/tests/armoury/animation/walker_logic.rs", "england", 2),
    ("crates/l2-game/tests/armoury/hit_map.rs", "england", 2),
    ("crates/l2-game/tests/armoury/rack.rs", "england", 2),
    ("crates/l2-game/tests/armoury/screenshots.rs", "england", 1),
    ("crates/l2-game/tests/arms.rs", "executable", 1),
    ("crates/l2-game/tests/arrival/arrival_tests.rs", "install", 1),
    ("crates/l2-game/tests/audio_battle/cries/audio_coverage.rs", "executable", 1),
    ("crates/l2-game/tests/audio_battle/cries/audio_coverage.rs", "install", 2),
    ("crates/l2-game/tests/audio_battle/determinism.rs", "install", 2),
    ("crates/l2-game/tests/audio_battle/events.rs", "install", 1),
    ("crates/l2-game/tests/audio_install/format_tests.rs", "executable", 1),
    ("crates/l2-game/tests/audio_install/format_tests.rs", "install", 2),
    ("crates/l2-game/tests/audio_install/track_tests.rs", "england", 1),
    ("crates/l2-game/tests/audio_install/track_tests.rs", "install", 2),
    ("crates/l2-game/tests/audio_install/voice_tests.rs", "install", 4),
    ("crates/l2-game/tests/audio_screens/audio_behavior.rs", "install", 1),
    ("crates/l2-game/tests/audio_screens/screens/panels_and_sites.rs", "install", 3),
    ("crates/l2-game/tests/audio_screens/screens/standings_and_popups.rs", "install", 2),
    ("crates/l2-game/tests/audio_screens/screens/title_and_screens.rs", "install", 3),
    ("crates/l2-game/tests/audio_wiring/audio_controls_and_feedback/map_and_castle_feedback.rs", "install", 3),
    ("crates/l2-game/tests/audio_wiring/audio_controls_and_feedback/mercenary_offer_voice.rs", "install", 2),
    ("crates/l2-game/tests/audio_wiring/audio_controls_and_feedback/sound_settings_and_ui.rs", "install", 2),
    ("crates/l2-game/tests/audio_wiring/audio_controls_and_feedback/speech_and_panels.rs", "install", 5),
    ("crates/l2-game/tests/audio_wiring/ui_and_speech/title_and_speech.rs", "install", 3),
    ("crates/l2-game/tests/audio_wiring/ui_and_speech/voice_and_audio_metrics.rs", "install", 4),
    ("crates/l2-game/tests/battle_picture/band.rs", "install", 2),
    ("crates/l2-game/tests/battle_picture/column.rs", "install", 5),
    ("crates/l2-game/tests/battle_picture/entities.rs", "install", 3),
    ("crates/l2-game/tests/battle_picture/motion/motion_tests.rs", "install", 1),
    ("crates/l2-game/tests/battle_picture/panel.rs", "install", 3),
    ("crates/l2-game/tests/battle_picture/plates.rs", "install", 3),
    ("crates/l2-game/tests/battle_picture/render.rs", "install", 3),
    ("crates/l2-game/tests/battle_picture/ui/battle_events.rs", "install", 3),
    ("crates/l2-game/tests/battle_picture/ui/menu_bar.rs", "install", 3),
    ("crates/l2-game/tests/cattle_shortage.rs", "england", 2),
    ("crates/l2-game/tests/chrome_text/county_and_units/foraging_and_shoot.rs", "england", 2),
    ("crates/l2-game/tests/chrome_text/county_and_units/garrison_and_mercenaries.rs", "england", 2),
    ("crates/l2-game/tests/chrome_text/county_and_units/unit_panels.rs", "england", 2),
    ("crates/l2-game/tests/chrome_text/county_and_units/zoom_and_battle.rs", "england", 2),
    ("crates/l2-game/tests/chrome_text/panels/drop_down_plate.rs", "england", 1),
    ("crates/l2-game/tests/chrome_text/panels/tile_panel.rs", "england", 1),
    ("crates/l2-game/tests/chrome_text/panels/title_and_build.rs", "england", 2),
    ("crates/l2-game/tests/chrome_text/top_bar.rs", "england", 5),
    ("crates/l2-game/tests/cursor.rs", "england", 3),
    ("crates/l2-game/tests/differential.rs", "fixture", 3),
    ("crates/l2-game/tests/ground/army_and_confirm.rs", "install", 2),
    ("crates/l2-game/tests/ground/diplomacy_part.rs", "install", 5),
    ("crates/l2-game/tests/industry/visual_effects.rs", "england", 3),
    ("crates/l2-game/tests/industry/wheel_rates.rs", "england", 2),
    ("crates/l2-game/tests/job_bodies/events_and_letters.rs", "england", 2),
    ("crates/l2-game/tests/job_bodies/events_and_letters.rs", "fixture", 1),
    ("crates/l2-game/tests/job_bodies/job_popups/dirty.rs", "england", 3),
    ("crates/l2-game/tests/job_bodies/job_popups/grain_and_cattle.rs", "england", 3),
    ("crates/l2-game/tests/job_bodies/job_popups/grain_and_cattle.rs", "fixture", 1),
    ("crates/l2-game/tests/job_bodies/job_popups/industry_and_building.rs", "england", 2),
    ("crates/l2-game/tests/job_bodies/job_popups/industry_and_building.rs", "executable", 1),
    ("crates/l2-game/tests/job_bodies/job_popups/industry_and_building.rs", "fixture", 3),
    ("crates/l2-game/tests/job_bodies/tile_panel.rs", "england", 3),
    ("crates/l2-game/tests/labour_move/drag_and_drop_tests.rs", "england", 4),
    ("crates/l2-game/tests/labour_move/game_flow_tests.rs", "england", 1),
    ("crates/l2-game/tests/labour_move/game_flow_tests.rs", "fixture", 1),
    ("crates/l2-game/tests/long_game/england.rs", "england", 2),
    ("crates/l2-game/tests/long_game/england.rs", "other", 1),
    ("crates/l2-game/tests/long_game/scenarios.rs", "england", 1),
    ("crates/l2-game/tests/long_game/scenarios.rs", "install", 1),
    ("crates/l2-game/tests/long_game/scenarios.rs", "other", 1),
    ("crates/l2-game/tests/merchant/merchant_ui.rs", "england", 3),
    ("crates/l2-game/tests/merchant/trade_actions.rs", "england", 4),
    ("crates/l2-game/tests/messages/help_window.rs", "install", 1),
    ("crates/l2-game/tests/messages/painting_and_layout.rs", "install", 5),
    ("crates/l2-game/tests/military/battle_part/hires_and_sieges.rs", "fixture", 1),
    ("crates/l2-game/tests/minimap/overlays.rs", "england", 3),
    ("crates/l2-game/tests/minimap/ui.rs", "england", 3),
    ("crates/l2-game/tests/movies/audio_part.rs", "install", 4),
    ("crates/l2-game/tests/movies/playback/input_and_drawing.rs", "install", 3),
    ("crates/l2-game/tests/movies/playback/timing_and_sound.rs", "install", 4),
    ("crates/l2-game/tests/movies/triggers/validation.rs", "install", 1),
    ("crates/l2-game/tests/newgame/army_size.rs", "other", 1),
    ("crates/l2-game/tests/newgame/campaign.rs", "install", 1),
    ("crates/l2-game/tests/newgame/campaign.rs", "other", 3),
    ("crates/l2-game/tests/newgame/heraldry.rs", "other", 4),
    ("crates/l2-game/tests/newgame/start_game.rs", "install", 2),
    ("crates/l2-game/tests/newgame/start_game.rs", "other", 3),
    ("crates/l2-game/tests/overlay_palette/overlay_palette.rs", "install", 2),
    ("crates/l2-game/tests/pacing/pacing/pacing_tests.rs", "england", 5),
    ("crates/l2-game/tests/press.rs", "executable", 1),
    ("crates/l2-game/tests/right_column/geometry.rs", "executable", 1),
    ("crates/l2-game/tests/save/roundtrip.rs", "england", 1),
    ("crates/l2-game/tests/save/ui/save_tests.rs", "install", 1),
    ("crates/l2-game/tests/scenario/england_fixture.rs", "england", 7),
    ("crates/l2-game/tests/scenario/save_loading.rs", "england", 1),
    ("crates/l2-game/tests/scenario/save_loading.rs", "fixture", 1),
    ("crates/l2-game/tests/scenario/turn_execution.rs", "england", 5),
    ("crates/l2-game/tests/screens_battle.rs", "england", 2),
    ("crates/l2-game/tests/screens_county/drawing_and_emboss/county_name_tests.rs", "england", 3),
    ("crates/l2-game/tests/screens_county/drawing_and_emboss/panel_tests.rs", "england", 3),
    ("crates/l2-game/tests/screens_county/layout_and_labels/labels.rs", "england", 3),
    ("crates/l2-game/tests/screens_county/layout_and_labels/layout.rs", "england", 4),
    ("crates/l2-game/tests/screens_county/panels/population.rs", "england", 2),
    ("crates/l2-game/tests/screens_county/panels/rations.rs", "england", 2),
    ("crates/l2-game/tests/screens_county/panels/tax.rs", "england", 4),
    ("crates/l2-game/tests/screens_county/panels/turns_and_sidebar.rs", "england", 4),
    ("crates/l2-game/tests/screens_county/produce_and_pastures/produce_and_pastures_tests.rs", "england", 5),
    ("crates/l2-game/tests/screens_county/strip_and_sidebar/strip_closing_and_numbers.rs", "england", 2),
    ("crates/l2-game/tests/screens_county/strip_and_sidebar/strip_quadrants_and_sidebar_buttons.rs", "england", 2),
    ("crates/l2-game/tests/screens_county/strip_and_sidebar/strip_slider_and_pointer.rs", "england", 3),
    ("crates/l2-game/tests/screens_diplomacy.rs", "england", 1),
    ("crates/l2-game/tests/screens_info/field_panel/interaction.rs", "england", 3),
    ("crates/l2-game/tests/screens_info/field_panel/labels_and_views.rs", "england", 4),
    ("crates/l2-game/tests/screens_info/field_panel/labels_and_views.rs", "executable", 1),
    ("crates/l2-game/tests/screens_info/tile_panel_part.rs", "england", 5),
    ("crates/l2-game/tests/screens_map/fog_and_march_tests/fog_tests.rs", "england", 3),
    ("crates/l2-game/tests/screens_map/fog_and_march_tests/march_tests.rs", "england", 4),
    ("crates/l2-game/tests/screens_map/interaction_tests/chrome_tests.rs", "england", 1),
    ("crates/l2-game/tests/screens_map/interaction_tests/selection_and_click_tests.rs", "england", 4),
    ("crates/l2-game/tests/screens_map/interaction_tests/town_and_mine_tests.rs", "england", 3),
    ("crates/l2-game/tests/screens_map/interaction_tests/view_and_navigation_tests.rs", "england", 2),
    ("crates/l2-game/tests/screens_map/structures_tests/castles.rs", "england", 3),
    ("crates/l2-game/tests/screens_map/structures_tests/minimap_and_seasons.rs", "england", 4),
    ("crates/l2-game/tests/screens_map/view_tests/view_tests.rs", "england", 6),
    ("crates/l2-game/tests/screens_menubar/menu_bar.rs", "england", 5),
    ("crates/l2-game/tests/screens_menubar/turn_timer.rs", "england", 2),
    ("crates/l2-game/tests/screens_menubar/turn_timer.rs", "executable", 1),
    ("crates/l2-game/tests/screens_shoot.rs", "england", 1),
    ("crates/l2-game/tests/screens_village/animation.rs", "england", 1),
    ("crates/l2-game/tests/screens_village/animation.rs", "install", 2),
    ("crates/l2-game/tests/screens_village/interaction/job_popups.rs", "england", 4),
    ("crates/l2-game/tests/screens_village/interaction/navigation.rs", "england", 1),
    ("crates/l2-game/tests/screens_village/interaction/peasant_drag.rs", "england", 2),
    ("crates/l2-game/tests/screens_village/render/render_tests.rs", "england", 5),
    ("crates/l2-game/tests/seam/battle.rs", "fixture", 2),
    ("crates/l2-game/tests/seam/conquest_part.rs", "fixture", 1),
    ("crates/l2-game/tests/seam/realm.rs", "fixture", 1),
    ("crates/l2-game/tests/setup/setup_tests.rs", "england", 7),
    ("crates/l2-game/tests/setup/setup_tests.rs", "install", 1),
    ("crates/l2-game/tests/setup/title_tests/build_stamp_tests.rs", "england", 2),
    ("crates/l2-game/tests/setup/title_tests/clock_tests.rs", "england", 6),
    ("crates/l2-game/tests/shell/font_part/baseline.rs", "install", 1),
    ("crates/l2-game/tests/shell/font_part/descenders.rs", "install", 1),
    ("crates/l2-game/tests/shell/font_part/font_numeral.rs", "install", 1),
    ("crates/l2-game/tests/shell/font_part/glyph_map.rs", "install", 1),
    ("crates/l2-game/tests/shell/font_part/measure.rs", "install", 1),
    ("crates/l2-game/tests/shell/font_part/preload.rs", "executable", 1),
    ("crates/l2-game/tests/shell/glyph.rs", "install", 2),
    ("crates/l2-game/tests/siege_picture/rendering_tests/banner_and_wall_rendering.rs", "install", 2),
    ("crates/l2-game/tests/siege_picture/rendering_tests/cell_selection.rs", "install", 2),
    ("crates/l2-game/tests/siege_picture/rendering_tests/picture_comparison.rs", "install", 1),
    ("crates/l2-game/tests/siege_picture/tables_and_sheets.rs", "executable", 1),
    ("crates/l2-game/tests/standings/geometry.rs", "executable", 1),
    ("crates/l2-game/tests/standings/geometry.rs", "install", 2),
    ("crates/l2-game/tests/text/rendering.rs", "install", 2),
    ("crates/l2-game/tests/text/setup.rs", "install", 1),
    ("crates/l2-game/tests/tips/tip_tests/audio_and_narration.rs", "install", 4),
    ("crates/l2-game/tests/tooltips/rendering.rs", "executable", 1),
    ("crates/l2-game/tests/tooltips/rendering.rs", "install", 1),
    ("crates/l2-game/tests/wheat/wheat_test.rs", "england", 1),
    ("crates/l2-kingdom/tests/cattle.rs", "saves", 3),
    ("crates/l2-kingdom/tests/defence.rs", "fixture", 2),
    ("crates/l2-kingdom/tests/fields/counts.rs", "england", 3),
    ("crates/l2-kingdom/tests/fields/economy/forecasts.rs", "england", 3),
    ("crates/l2-kingdom/tests/fields/herd_vis.rs", "england", 3),
    ("crates/l2-kingdom/tests/fields/painting.rs", "england", 2),
    ("crates/l2-kingdom/tests/industry_forecast/forecast/advanced_farming.rs", "england", 1),
    ("crates/l2-kingdom/tests/industry_forecast/forecast/england_season.rs", "england", 1),
    ("crates/l2-kingdom/tests/industry_forecast/forecast/refresh.rs", "england", 1),
    ("crates/l2-kingdom/tests/industry_forecast/forecast/row_ramp.rs", "england", 1),
    ("crates/l2-kingdom/tests/oracle.rs", "executable", 6),
    ("crates/l2-kingdom/tests/reproduction/food_and_ration.rs", "england", 3),
    ("crates/l2-kingdom/tests/reproduction/population_and_labour.rs", "england", 5),
    ("crates/l2-kingdom/tests/reproduction/reproduction/scenarios.rs", "england", 5),
    ("crates/l2-kingdom/tests/reproduction/reproduction/simulation.rs", "england", 6),
    ("crates/l2-kingdom/tests/reproduction/simulation.rs", "england", 6),
    ("crates/l2-kingdom/tests/siege/siege_tests.rs", "fixture", 5),
    ("crates/l2-kingdom/tests/weapon_choice.rs", "fixture", 1),
    ("crates/l2-mods/tests/corpus/difficulty.rs", "install", 1),
    ("crates/l2-mods/tests/corpus/indexing.rs", "install", 2),
    ("crates/l2-mods/tests/corpus/seeding.rs", "install", 3),
    ("crates/l2-scenario/tests/explored.rs", "england", 2),
    ("crates/l2-scenario/tests/explored.rs", "fixture", 1),
    ("crates/l2-scenario/tests/explored.rs", "install", 1),
    ("crates/l2-scenario/tests/import/county.rs", "england", 5),
    ("crates/l2-scenario/tests/import/county.rs", "saves", 6),
    ("crates/l2-scenario/tests/import/economy/forecasts_and_events.rs", "saves", 3),
    ("crates/l2-scenario/tests/import/economy/industry_and_labor.rs", "england", 4),
    ("crates/l2-scenario/tests/import/economy/industry_and_labor.rs", "saves", 1),
    ("crates/l2-scenario/tests/import/economy/tax_and_happiness.rs", "saves", 6),
    ("crates/l2-scenario/tests/import/units/generic_units.rs", "saves", 5),
    ("crates/l2-scenario/tests/import/units/mercenaries.rs", "fixture", 1),
    ("crates/l2-scenario/tests/import/units/mercenaries.rs", "saves", 7),
    ("crates/l2-scenario/tests/import/units/trade_units.rs", "england", 1),
    ("crates/l2-scenario/tests/newgame/build.rs", "install", 4),
    ("crates/l2-scenario/tests/newgame/comparison.rs", "england", 2),
    ("crates/l2-scenario/tests/stored_fields/oracle_tests.rs", "other", 1),
    ("crates/l2-scenario/tests/stored_fields/value_tests.rs", "saves", 1),
    ("crates/l2-sim/tests/castle_layout/layout_tests.rs", "install", 8),
    ("crates/l2-sim/tests/castle_layout/siege_tests.rs", "install", 4),
    ("crates/l2-sim/tests/oracle.rs", "executable", 4),
    ("crates/l2-smk/tests/corpus/corpus_tests.rs", "install", 5),
    ("crates/l2-view/tests/install/corpus.rs", "install", 4),
    ("crates/l2-view/tests/install/oracle/asset_tests.rs", "executable", 2),
    ("crates/l2-view/tests/install/oracle/asset_tests.rs", "fixture", 1),
    ("crates/l2-view/tests/install/oracle/asset_tests.rs", "install", 5),
    ("crates/l2-view/tests/install/oracle/campaign_tests.rs", "executable", 1),
    ("crates/l2-view/tests/install/oracle/campaign_tests.rs", "install", 8),
    ("crates/l2-view/tests/install/render/battle_part.rs", "install", 3),
    ("crates/l2-view/tests/install/render/sprites.rs", "install", 3),
    ("crates/l2-view/tests/install/render/terrain_part.rs", "install", 2),
    ("crates/l2-view/tests/install/render/ui.rs", "install", 4),
];

pub(crate) const GATED_TOTAL: usize = 598;

/// The needles that name a gate, strongest first. A body containing several is
/// counted against the first that matches.
const NEEDLES: &[(&str, Gate)] = &[
    ("england!(", Gate::England),
    ("england_turn1(", Gate::England),
    ("fixture!(", Gate::Fixture),
    ("fixture_save(", Gate::Fixture),
    ("fixtures_dir(", Gate::Fixture),
    ("saves!(", Gate::Saves),
    ("every_available_save(", Gate::Saves),
    ("executable!(", Gate::Executable),
    ("l2_testkit::executable(", Gate::Executable),
    ("install!(", Gate::Install),
    ("install_dir(", Gate::Install),
    ("read_install(", Gate::Install),
    ("l2_testkit::skip!(", Gate::Other),
];

/// The census itself.
#[test]
fn the_install_gated_tests_are_the_ones_we_have_written_down() {
    let found = scan();
    let expected: BTreeMap<(String, &'static str), usize> =
        INVENTORY.iter().map(|&(f, g, n)| ((f.to_string(), g), n)).collect();

    if found != expected {
        let mut lines = String::new();
        for ((file, gate), n) in &found {
            lines.push_str(&format!("    (\"{file}\", \"{gate}\", {n}),\n"));
        }
        panic!(
            "the gate census has moved.\n\n\
             A gated test does not run on CI, and nothing else in the suite says so - which is \
             why this number is written down. If the change is intended, replace INVENTORY in \
             crates/l2-testkit/tests/census/main.rs with:\n\n\
             const INVENTORY: &[(&str, &str, usize)] = &[\n{lines}];\n\n\
             expected {} entries, found {}",
            expected.len(),
            found.len()
        );
    }
    assert_eq!(
        found.values().sum::<usize>(),
        GATED_TOTAL,
        "{GATED_TOTAL} test functions in this workspace do not exist without a copy of the game"
    );
    eprintln!("gate census: {GATED_TOTAL} install-gated tests across {} files", INVENTORY.len());
}

