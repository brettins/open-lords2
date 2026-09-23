//! **How a skirmish's end reaches screen `0x2E`** — the `DAT_0057A0F0 != 0`
//! limb of every place a battle finishes.
//!
//! ```text
//! Battle_CheckOutcome (0x00477DFC), 00470000.c:3452-3469:
//!   if (DAT_0057A0F0 == 0) { Battle_ReturnToCampaign(1); g_screenId = 0x13; … }
//!   else { Battle_WriteBackCasualties(); g_screenId = '.';   /* 0x2E */
//!          FUN_0042C64F(); g_battlePhase = 0; g_appPhase = 2; }
//!   g_redrawRequest = 2;
//! ```
//!
//! and the same five statements again in the `g_screenId == 0x2B` timeout arm
//! (`00470000.c:3323`), in `Battle_GiveUp` (`00430000.c:6938`), in
//! `FUN_00433DEB` (`00430000.c:1227`), in `NetCmd_Write_19` / `NetCmd_Read_19`
//! (`00440000.c:1749`, `:1779`) and in `NetAct_ShowBattleScreen`
//! (`00440000.c:5695`) — seven writers, every one of them guarded on the
//! skirmish flag, the `== 0` sibling going to campaign screen `0x13`.
//!
//! `FUN_0042C64F` is [`super::score`]; this module is the two statements
//! around it.
use super::*;

impl BattleMaster {
    /// **`FUN_0042C5AD` (`0x0042C5AD`)**, run by *Go* inside `FUN_0043D5B7`:
    /// `Mem_CopyBytes(0x52F254, 0x52F59C, 0x1A4)` and
    /// `Mem_CopyBytes(0x52F3F8, 0x52F740, 0x1A4)` — the two musters copied
    /// into the records the ratings sheet reads as *Before*.
    pub fn begin(&mut self, realms: (u8, u8), local_side: u8, mine: [i16; 11], theirs: [i16; 11]) {
        let snap = |c: [i16; 11]| Snapshot {
            troops: std::array::from_fn(|i| c[i] as i32),
            men: c[..COLUMNS].iter().map(|&n| n as i32).sum(),
        };
        self.local_side = local_side;
        self.last = Ratings {
            mine: (snap(mine), snap(mine)),
            theirs: (snap(theirs), snap(theirs)),
            ending: Ending::Fought,
            realms,
        };
    }

    /// `Battle_WriteBackCasualties` into the `+0x4` records, then
    /// `FUN_0042C64F`: the survivors become *after*, and the two scores are
    /// computed once, here, and only read by the sheet.
    pub fn finish(&mut self, mine: [u32; 11], theirs: [u32; 11], ending: Ending) {
        let snap = |c: [u32; 11]| Snapshot {
            troops: std::array::from_fn(|i| c[i] as i32),
            men: c[..COLUMNS].iter().map(|&n| n as i32).sum(),
        };
        self.last.mine.1 = snap(mine);
        self.last.theirs.1 = snap(theirs);
        self.last.ending = ending;
    }

    /// Which `l2_sim` side `DAT_00553EFC` stands for — the local player's.
    pub fn local_side(&self) -> u8 {
        self.local_side
    }
}

/// `Conclusion` → the three endings `FUN_0042C64F` branches on:
/// `g_battleWithdrawal` with `DAT_005656F8` naming who withdrew, then
/// `g_castleEntered` with `DAT_0053EF5C`. The flag names the side that
/// **suffered** it, which is why `Ending::Withdrew { local: true }` scores the
/// local player 0. The castle arm is keyed on the attacker, not the winner:
/// `if ((g_castleEntered == 0) || (DAT_0053ef5c != g_localPlayer))`,
/// `00420000.c:4934`. [V]
pub fn ending_of(c: Option<l2_sim::runner::Conclusion>, local_side: u8) -> Ending {
    let Some(c) = c else { return Ending::Fought };
    let local_lost = c.winner != local_side;
    match c.cause {
        l2_sim::runner::End::Withdrawal => Ending::Withdrew { local: local_lost },
        // `DAT_0053EF5C == g_localPlayer`: the local muster is `SIDE_A` when he attacks (skirmish_start.rs).
        l2_sim::runner::End::BrokeIn => Ending::CastleFell { local: local_side == l2_sim::SIDE_A },
        _ => Ending::Fought,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use l2_sim::runner::{Conclusion, End};

    #[test]
    fn a_castle_ending_is_charged_to_the_attacker_whoever_won() {
        let c = |winner| Some(Conclusion { winner, cause: End::BrokeIn });
        for w in [l2_sim::SIDE_A, l2_sim::SIDE_B] {
            let at = ending_of(c(w), l2_sim::SIDE_A);
            assert_eq!(at, Ending::CastleFell { local: true }, "00420000.c:4934");
            let def = ending_of(c(w), l2_sim::SIDE_B);
            assert_eq!(def, Ending::CastleFell { local: false });
        }
    }
}
