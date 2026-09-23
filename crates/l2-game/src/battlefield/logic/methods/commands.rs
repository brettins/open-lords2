#![allow(unused_imports)]
use super::*;
use super::view::*;
use super::field::*;
use super::groups::*;
use super::super::*;
use super::super::*;
use super::super::view::*;
use tests::*;
use l2_sim::runner::{BattleRunner, Conclusion, Formation};
use l2_sim::terrain::DIM;
use crate::input::Rect;

impl LiveBattle {
    /// `FUN_0043B9A1` (`0x0043B9A1`) — **the pause button**.
    ///
    /// `DAT_0053F238 = ~DAT_0053F238` toggles between 0 and −1, and the branch
    /// underneath it — `if (DAT_0053F238 == 1) { … Sound_PlayFile("s032_01.wav") }`
    /// — can therefore never be taken. The pause sound in the shipped game is
    /// dead code. `docs/bugs.md` D38.
    ///
    /// // arm: 0x0043B9A1/pause left-press
    pub fn press_pause(&mut self) -> bool {
        if self.choice_owner == 0 {
            return false;
        }
        self.act(BattleOp::Pause);
        self.redraw = true;
        true
    }

    /// `FUN_0043BA29` (`0x0043BA29`) — **retreat**, or **surrender** when this
    /// is a siege and the local player owns army B.
    ///
    /// It opens `Ui_OpenConfirm(12)` or `Ui_OpenConfirm(11)`; both callbacks
    /// reach `FUN_0043BE65`, which is `Battle_AutoResolve` plus the return to
    /// the campaign. The prompt index is what tells them apart to the player and
    /// nothing downstream reads which was answered except `DAT_005653F4`.
    ///
    /// // arm: 0x0043BA29/retreat left-press
    pub fn press_retreat(&mut self) -> Option<usize> {
        if self.choice_owner != 1 {
            return None;
        }
        Some(if self.is_siege() { 11 } else { 12 })
    }

    /// `FUN_0043BBE7` (`0x0043BBE7`) — **lower the drawbridge**, the garrison's
    /// own button.
    ///
    /// ```c
    /// if (g_battleChoiceOwner == 0) return;
    /// if (!g_battleIsSiege)                          Msg_Enqueue(0x6E);   /* Sieges only! */
    /// else if (localPlayer != units[armyB].owner)    Msg_Enqueue(0x6F);   /* No drawbridge! */
    /// else if (g_castleLevel < 3)                    Msg_Enqueue(0x6F);
    /// else if (DAT_0052AF9C != 0)                    Msg_Enqueue(0x9D);   /* Drawbridge is down. */
    /// else if (!g_multiplayer) FUN_00496B9F(); else Net_SendCommand(0x45, 0);
    /// ```
    ///
    /// **The three messages are what name the verb**: `L2.eng` group 110
    /// *"Sieges only!"*, group 111 *"No drawbridge!"* and group 157
    /// *"Drawbridge is down."* — so `FUN_00496B9F` lowers a drawbridge and
    /// nothing else. The hand-off this was built from called it siege-engine
    /// placement; it is not, and `docs/decisions.md` `C79`
    /// records how the mistake was caught.
    ///
    /// Returns the `L2.eng` message the original enqueues when it refuses, or
    /// `Ok` when the bridge went down. `sallied` is set **only when the routine
    ///
    /// // arm: 0x0043BBE7/sally left-press
    pub fn press_sally(&mut self, garrison_is_local: bool) -> Result<(), u16> {
        if self.choice_owner == 0 {
            return Err(0);
        }
        if !self.is_siege() {
            return Err(0x6E);
        }
        if !garrison_is_local || self.castle_level.unwrap_or(0) < 3 {
            return Err(0x6F);
        }
        if self.sallied {
            return Err(0x9D);
        }
        if !self.act(BattleOp::Sally) {
            return Err(0x6F);
        }
        self.redraw = true;
        Ok(())
    }

    /// `FUN_0043BD02` (`0x0043BD02`) → `FUN_0047A76D` (`0x0047A76D`) — **the
    /// charge**, and it is available exactly once in a battle to each realm.
    ///
    /// // arm: 0x0043BD02/charge left-press
    pub fn has_charged(&self, realm: u8) -> bool {
        self.charged & (1 << realm) != 0
    }

    pub fn press_charge(&mut self) -> bool {
        if self.choice_owner == 0 || self.has_charged(self.owner) {
            return false;
        }
        self.act(BattleOp::Charge);
        self.redraw = true;
        true
    }

    /// `FUN_0043BD67` (`0x0043BD67`) — `Ui_OpenConfirm(9)`, *"Autocalc
    /// battle?"*.
    ///
    /// // arm: 0x0043BD67/autocalc left-press
    pub fn press_autocalc(&mut self) -> Option<usize> {
        if self.choice_owner != 1 {
            return None;
        }
        Some(9)
    }

    /// What the two confirm boxes do when they are answered yes: `FUN_0043BE65`
    /// (`0x0043BE65`), and the return to the
    /// campaign. Declining a battle and giving up on one you are watching are
    /// the same code.
    pub fn confirm_autocalc(&mut self) {
        self.act(BattleOp::Autocalc);
    }


}
