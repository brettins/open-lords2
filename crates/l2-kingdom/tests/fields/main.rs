//! **The field counts, against the tiles they were counted from.**
//!
//! ```text
//! LORDS2_FIXTURES="E:\dev\lords2-fixtures" cargo test -p l2-kingdom --test fields
//! ```
//!
//! # What this is evidence about
//!
//! `l2_kingdom::field::classify` is a ladder of six terrain boundaries taken
//! off `County_RecountFields` (`FUN_00469B8D`, `0x00469B8D`) in the
//! decompiler. `docs/decisions.md` C3 is what happens when a plausible ladder
//! is believed because it is tidy, and C24 is what happens when a
//! hand-transcribed table is never read back.
//!
//! The England turn-one save closes that loop **without any of our rules being
//! involved**, because it stores both halves of the sum. `g_countyFieldTiles`
//! names the twenty tiles, `g_tiles` holds each one's terrain byte, and county
//! `+0x1FF`, `+0x200` and `+0x201` hold the three counts the game itself made
//! of them. Applying our ladder to the first and comparing against the third is
//! a check nothing in this tree can make come out right by agreeing with
//! itself: the file was written by the original.
//!
//! # And it is the reason the brush exists
//!
//! Every one of the fourteen counties has `fieldsGrain = 0`.
//! quirk of this position — it is what the start of a game *is*: you paint your
//! fields. Until [`l2_kingdom::field`] there was no code path in this tree that
//! could set that number for the human player at all, so the entire grain half
//! of the economy was finished, tested and unreachable in play.

mod counts;
pub use counts::*;
mod painting;
pub use painting::*;
mod herd_vis;
pub use herd_vis::*;
mod economy;
pub use economy::*;

use l2_kingdom::field::FieldType;
use l2_kingdom::{Kingdom, MAX_FIELDS};
use l2_scenario::Scenario;
use l2_testkit::england;

