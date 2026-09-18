//! `docs/decisions.md` C12, exactly: a test headed *"the reproduction from the
//! shipped save"* that never read a save. It declared `const OWNED: usize = 4`,
//! handed counties 1–4 to the human realm, gave every county the same numbers
//! out of `docs/kingdom.md`, and then checked that those numbers came back —
//! a scenario invented from the document, measured against the rules built from
//! the same document. It could not fail, and its scenario was wrong.
//!
//! It also stores **no pre-season labour**. `FUN_0044F6E7` reallocates every
//! county's workers after the population moves, so what is in the file is where
//! the peasants went *afterwards* — and the herd's births and deaths were
//! computed from where they were *before*. That is why [`comparison`] no longer
//! carries `herd` and `herd_eaten`, and its own documentation is where the
//! evidence for that is written down. Nothing was relaxed to make a rule pass:
//!
//! >
//! > **The save does record what the season started with.** `Grain_SeasonTick`
//! > (`0x0044C8AE`) and `Herd_SeasonTick` (`0x0044D60D`) each open by copying the
//! > store into county `+0x228` / `+0x254` and only then take the season's food
//! > out of it; `Game_SetupRealmsAndCounties` (`0x0049BD99`) writes the same two
//! > fields with the new-game stores. In this file `+0x254` is 95 in every
//! > county — so realm 5's county went into the ration pass with 95 head, fed
//! > all 417 people on cheese, and ate **no grain at all**. There were never
//! > eight sacks.
//!
//! >
//! > **And `Ration_Apply` (`0x0044DF5F`) does not debit** — `docs/decisions.md`
//! > C149 read it and found no store `-=` anywhere; the debit is the two ticks'
//! > second statements. The inversion here assumed a debiting pass that saw the
//! > post-season herd, and under that model its answer really is unique — which
//! > is why uniqueness did not make it a measurement.
//!
//! >
//! > Start every county from `+0x228` and `+0x254` and the whole map reproduces,
//! > fourteen counties and twenty-five fields with no inversion anywhere:
//!
//! > [`every_county_reproduces_from_the_stores_the_season_found`]. What predicts
//! > "the hungry county" is therefore not a lord who begins short of food but the
//! > one county whose herd the season took below 84 head, where cheese stops
//! > covering 417 people. Why realm 5's herd fell further than the others' — its
//! > lord is the only arable one in this save, and the arable style keeps one
//! > pasture — is `[I]`, not established here.


use l2_formats::save::Save;
use l2_kingdom::county::County;
use l2_kingdom::phase::SEASON_PIPELINE;
use l2_kingdom::tables::{health_band, Season, Tables, Weather};
use l2_scenario::{Scenario, STARTING_HEALTH_METER};

const SEED: u64 = 0x10D_52;

pub(crate) fn county_i32(save: &Save, id: usize, offset: u32) -> i32 {
    save.i32_at(0x0053_F9B0 + (id as u32) * 0x300 + offset).expect("a saved county address")
}

macro_rules! england {
    () => {{
        let save = l2_testkit::england!();
        Scenario::from_save(&save).expect("the England turn-one fixture must import")
    }};
}

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


const MAP_DEAD_END: usize = 1;

