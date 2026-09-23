
mod types;
pub use types::*;
mod dirty;
pub use dirty::*;
mod screen;
pub use screen::*;
mod machine_struct;
pub use machine_struct::*;

mod machine;
pub use machine::*;
pub mod tick_gate;

use l2_view::Canvas;

use crate::game::{Assets, Game};
use crate::input::Event;

