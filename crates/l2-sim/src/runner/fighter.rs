use super::*;

impl Fighter {
    pub(super) fn pos(&self) -> Pos {
        Pos::new(self.x, self.y)
    }
    pub(super) fn at_target(&self) -> bool {
        (self.x, self.y) == self.target
    }

    /// Which of the two counts this figure's corpse runs on.
    ///
    /// `BattleUnit_Create`'s `6 < troopType && troopType < 10` is the engine
    /// test — [`crate::fire::is_engine`] — and **a pot of oil is not one of
    /// them**: `FUN_0047A814` puts a spent pot into state **2**, not 15.
    pub fn corpse_frames(&self) -> u16 {
        if crate::fire::is_engine(self.troop) {
            ENGINE_CORPSE_FRAMES
        } else {
            CORPSE_FRAMES
        }
    }
}

