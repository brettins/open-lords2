#![allow(unused_imports)]

mod b11a_trade;
pub use b11a_trade::*;
mod b12_b15_quirks;
pub use b12_b15_quirks::*;
mod b16_b17_quirks;
pub use b16_b17_quirks::*;

use super::*;
use super::economy::*;
use super::victory::*;
use super::wire::*;
use l2_kingdom::county::County;
use l2_kingdom::kingdom::Kingdom;
use l2_kingdom::realm::Realm;
use l2_kingdom::tables::{Season, Tables, Weather};
use l2_kingdom::{Quirk, Quirks};

// ---------------------------------------------------------------------------
// B11a — an unowned county trades with neither stock nor gold
// ---------------------------------------------------------------------------

