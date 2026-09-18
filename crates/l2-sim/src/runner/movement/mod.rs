
mod movement;
pub use movement::*;
mod pathfinding;
pub use pathfinding::*;

use super::*;

impl Formation {
    /// The value `BattleUnit_Order` writes into unit `+0x09`, or `None` for
    /// [`Formation::Keep`], which writes nothing.
    pub fn orientation(self) -> Option<u8> {
        match self {
            Formation::Keep => None,
            Formation::Line => Some(0),
            Formation::Column => Some(1),
        }
    }
}

