#![allow(unused_imports)]

mod methods;
pub use methods::*;

use super::*;
use super::view::*;
use tests::*;
use l2_sim::runner::{BattleRunner, Conclusion, Formation};
use l2_sim::terrain::DIM;
use crate::input::Rect;

