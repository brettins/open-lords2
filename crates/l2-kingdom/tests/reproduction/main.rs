//! **The reproduction from the England turn-one fixture** — and this time it
//! opens it.
//!
//! # What this file used to be
//!
//! `docs/decisions.md` C12, exactly: a test headed *"the reproduction from the
//! shipped save"* that never read a save. It declared `const OWNED: usize = 4`,
//! handed counties 1–4 to the human realm, gave every county the same numbers
//! out of `docs/kingdom.md`, and then checked that those numbers came back —
//! a scenario invented from the document, measured against the rules built from
//! the same document. It could not fail, and its scenario was wrong.
//!
//! The file holds **five owned counties, one for each of realms 1 to 5**, at
//! indices 1, 4, 8, 11 and 13; nine unowned; fourteen counties in seventeen
//! slots. `crates/l2-formats/tests/save_england_turn1/main.rs` asserts all of that
//! against the bytes.
//!
//! # "Shipped" was the wrong word, and it cost the suite a day
//!
//! A clean GOG install ships **no saves at all**. The file this test reads was
//! produced by somebody in an earlier session of this project starting a
//! campaign; the game writes `lastturn.sav` when turn one begins and rewrites it
//! every turn thereafter. Calling it "the shipped save" made a volatile file
//! look permanent, and resolving it by a hard-coded path inside the game
//! directory meant ten minutes of play replaced it. That is now a **named
//! fixture** — `%LORDS2_FIXTURES%\england-turn1.sav`, checked against
//! `l2_testkit::england_turn1_fingerprint` before a single assertion runs.
//!
//! Two things this position does *not* fix, and which are rolled per game:
//! **which realm gets which of the five starting counties**, and therefore
//! which county starts short of food. Both were written down here as constants.
//! See [`hungry_county`].
//!
//! # What it is now
//!
//! An oracle. `l2-scenario` imports the fixture twice — once as the state the
//! file holds, once rewound to the position it was taken from — and every
//! assertion below compares this crate's output against the *file's* numbers.
//! Nothing here is quoted from a document. If the pipeline's order, the health
//! ladder's comparison sense, the delta table's indexing, the 1-based season
//! array, the birth ladder's pairing or the unowned happiness bonus were wrong,
//! a number read out of the fixture would say so.
//!
//! **No rule was changed to make this pass.** Fourteen counties reproduce
//! twenty-four stored fields each — see
//! [`every_county_reproduces_every_stored_field`] — and the herd's own
//! forecast, four more numbers a county, reproduces with no inversion at all
//! ([`the_herds_own_forecast_reproduces_for_every_county`]).
//!
//! # The two things the save does not record
//!
//! It stores `popLast` and `happinessLast`, so the previous season's population
//! and happiness are exact. It stores **no previous herd and no previous
//! grain**: the ration pass ate some of both and only the remainder survives.
//! So the starting position `l2-scenario` builds carries the stored stores
//! forward, and one county of fourteen — **realm 5's**, whichever that is —
//! then starves where the real one did not. That is not a rule failing;
//! missing input, and [`the_food_the_season_ate_is_recoverable_and_unique`]
//! recovers it: there is **exactly one** pre-season store per county that
//! reproduces the file, and putting it back makes the map land.
//!
//! It also stores **no pre-season labour**. `FUN_0044F6E7` reallocates every
//! county's workers after the population moves, so what is in the file is where
//! the peasants went *afterwards* — and the herd's births and deaths were
//! computed from where they were *before*. That is why [`comparison`] no longer
//! carries `herd` and `herd_eaten`, and its own documentation is where the
//! evidence for that is written down. Nothing was relaxed to make a rule pass:
//! the herd rule is checked directly instead, against the numbers the file
//! holds for it.
//!
//! Recovering it also answers a question `docs/kingdom.md` §4.3 left open.
//! That county holds no grain and its `shownRation` is `+1`, so it was fed at
//! Normal by the pass that ran *before* happiness — and with an all-grain split
//! that costs eight sacks. A `Ration_Apply` that did not debit the store would
//! have left those eight sacks in it. **It debits**, which is the reading
//! `l2_kingdom::ration::apply` already implements.
//!
//! > ### ⚠ Both paragraphs above are wrong, and the save said so all along.
//! >
//! > **The save does record what the season started with.** `Grain_SeasonTick`
//! > (`0x0044C8AE`) and `Herd_SeasonTick` (`0x0044D60D`) each open by copying the
//! > store into county `+0x228` / `+0x254` and only then take the season's food
//! > out of it; `Game_SetupRealmsAndCounties` (`0x0049BD99`) writes the same two
//! > fields with the new-game stores. In this file `+0x254` is 95 in every
//! > county — so realm 5's county went into the ration pass with 95 head, fed
//! > all 417 people on cheese, and ate **no grain at all**. There were never
//! > eight sacks.
//! >
//! > **And `Ration_Apply` (`0x0044DF5F`) does not debit** — `docs/decisions.md`
//! > C149 read it and found no store `-=` anywhere; the debit is the two ticks'
//! > second statements. The inversion here assumed a debiting pass that saw the
//! > post-season herd, and under that model its answer really is unique — which
//! > is why uniqueness did not make it a measurement.
//! >
//! > Start every county from `+0x228` and `+0x254` and the whole map reproduces,
//! > fourteen counties and twenty-five fields with no inversion anywhere:
//! > [`every_county_reproduces_from_the_stores_the_season_found`]. What predicts
//! > "the hungry county" is therefore not a lord who begins short of food but the
//! > one county whose herd the season took below 84 head, where cheese stops
//! > covering 417 people. Why realm 5's herd fell further than the others' — its
//! > lord is the only arable one in this save, and the arable style keeps one
//! > pasture — is `[I]`, not established here.
//!
//! # Running it
//!
//! ```text
//! LORDS2_FIXTURES="E:\dev\lords2-fixtures" cargo test -p l2-kingdom --test reproduction
//! ```
//!
//! Every test skips *visibly* when the fixture is not configured, and **fails**
//! when something is configured that is not it. Those are different states and
//! conflating them is how the previous breakage went unnoticed.

mod helpers;
pub use helpers::*;
mod reproduction;
pub use reproduction::*;
mod food_and_ration;
pub use food_and_ration::*;
mod population_and_labour;
pub use population_and_labour::*;
mod simulation;
pub use simulation::*;

use l2_formats::save::Save;
use l2_kingdom::county::County;
use l2_kingdom::phase::SEASON_PIPELINE;
use l2_kingdom::tables::{health_band, Season, Tables, Weather};
use l2_scenario::{Scenario, STARTING_HEALTH_METER};

/// The seed is irrelevant to everything asserted here — with Advanced Farming
/// off the weather is forced, and no county draws an event in 1268 — but it is
/// fixed anyway, because a test that *would* notice a different seed is a test
/// worth having notice.
const SEED: u64 = 0x10D_52;

/// One county field straight out of the file, at the offset `docs/kingdom.md`
/// §1 names it at.
///
/// Used for the handful of fields `l2_formats::save::County` does not carry.
/// That struct belongs to `l2-formats` and is not this test's to change, and
/// `Save` exposes the addressed read the struct is itself built from — so the
/// bytes are reachable without either crate growing a field for a test.
fn county_i32(save: &Save, id: usize, offset: u32) -> i32 {
    save.i32_at(0x0053_F9B0 + (id as u32) * 0x300 + offset).expect("a saved county address")
}

/// The England turn-one fixture, imported. Skips when it is not configured and
/// panics when what is configured is a different game — see
/// `l2_testkit::england_turn1`.
macro_rules! england {
    () => {{
        let save = l2_testkit::england!();
        Scenario::from_save(&save).expect("the England turn-one fixture must import")
    }};
}

/// County 1 borders one county and nothing else. That **is** a property of the
/// England map and is stable across every save of it.
const MAP_DEAD_END: usize = 1;

