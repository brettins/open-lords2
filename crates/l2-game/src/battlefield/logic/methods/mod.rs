#![allow(unused_imports)]

mod view;
pub use view::*;
mod commands;
pub use commands::*;
mod field;
pub use field::*;
mod groups;
pub use groups::*;

use super::*;

use super::*;
use super::view::*;
use tests::*;
use l2_sim::runner::{BattleRunner, Conclusion, Formation};
use l2_sim::terrain::DIM;
use crate::input::Rect;

