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

mod counts;
pub use counts::*;
mod painting;
pub use painting::*;
mod herd_vis;
pub use herd_vis::*;
mod economy;
pub use economy::*;
mod blight;
pub use blight::*;

use l2_kingdom::field::FieldType;
use l2_kingdom::{Kingdom, MAX_FIELDS};
use l2_scenario::Scenario;
use l2_testkit::england;

