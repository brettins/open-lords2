#![allow(unused_imports)]

mod unit_impl;
pub use unit_impl::*;
mod units_impl;
pub use units_impl::*;
mod wages;
pub use wages::*;
mod starvation;
pub use starvation::*;
mod combine_part;
pub use combine_part::*;
mod destroy_part;
pub use destroy_part::*;

use super::*;

use crate::county::{County, MAX_COUNTIES};
use crate::realm::{Realm, MAX_REALMS};
use crate::tables::Tables;

/// `Army_StrengthScore`'s bonus for being an army at all — the `+ 20` on any
/// score that reached 1.
pub const STRENGTH_SCORE_BONUS: i32 = 20;

