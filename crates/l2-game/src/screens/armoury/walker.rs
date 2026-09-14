#![allow(unused_imports)]
use super::*;
use super::screen::*;
use super::tests::*;
use l2_kingdom::levy::{self, LevyRefusal};
use l2_kingdom::tables::WEAPON_TYPE_COUNT;
use l2_kingdom::unit::TroopType;
use l2_view::{text, Canvas};
use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{self, font, Pen};
use crate::widget;

/// **`g_armouryWalkerSheets` (`0x004DE450`)** — thirty sheets, `0x10` bytes
/// apart, `0x70` (seven slots) per shield colour, indexed by
/// `[shieldIndex][basketSlot]`.
///
/// Slot 0 and slot 1 both name the crossbowman.
/// names red twice: the tables are indexed by a 1-based slot and a 1-based
/// shield and each pads its zeroth entry with its first. The order after that
/// is the basket's — crossbow, mace, sword, pike, archer, knight — which is
/// **not** [`WALL`]'s order and not [`RACKS`]'s x order.
#[rustfmt::skip]
pub const WALKER_SHEETS: [[&str; 7]; 6] = [
    ["Trp_xb_r.pl8", "Trp_xb_r.pl8", "Trp_ma_r.pl8", "Trp_sw_r.pl8", "Trp_pi_r.pl8", "Trp_ar_r.pl8", "Trp_kn_r.pl8"],
    ["Trp_xb_r.pl8", "Trp_xb_r.pl8", "Trp_ma_r.pl8", "Trp_sw_r.pl8", "Trp_pi_r.pl8", "Trp_ar_r.pl8", "Trp_kn_r.pl8"],
    ["Trp_xb_y.pl8", "Trp_xb_y.pl8", "Trp_ma_y.pl8", "Trp_sw_y.pl8", "Trp_pi_y.pl8", "Trp_ar_y.pl8", "Trp_kn_y.pl8"],
    ["Trp_xb_k.pl8", "Trp_xb_k.pl8", "Trp_ma_k.pl8", "Trp_sw_k.pl8", "Trp_pi_k.pl8", "Trp_ar_k.pl8", "Trp_kn_k.pl8"],
    ["Trp_xb_p.pl8", "Trp_xb_p.pl8", "Trp_ma_p.pl8", "Trp_sw_p.pl8", "Trp_pi_p.pl8", "Trp_ar_p.pl8", "Trp_kn_p.pl8"],
    ["Trp_xb_b.pl8", "Trp_xb_b.pl8", "Trp_ma_b.pl8", "Trp_sw_b.pl8", "Trp_pi_b.pl8", "Trp_ar_b.pl8", "Trp_kn_b.pl8"],
];

/// `File_ReadChunk(…)`'s fallback when the colour-and-slot read fails —
/// `s_trp_xb_b_pl8_004DE8E8`, the blue crossbowman.
pub const WALKER_FALLBACK: &str = "Trp_xb_b.pl8";

/// The sheet one walk is drawn from, clamped the way the original's index
/// arithmetic is bounded.
pub fn walker_sheet(shield_index: u8, slot: u8) -> &'static str {
    let colour = (shield_index as usize).min(WALKER_SHEETS.len() - 1);
    let s = slot as usize;
    if s >= WALKER_SHEETS[colour].len() {
        return WALKER_FALLBACK;
    }
    WALKER_SHEETS[colour][s]
}

/// **The soldier who walks over and takes the weapon.**
///
/// A player, on `BUILD 3F9C11E`: *"The animations when you pick a weapon to
/// assign during an army doesn't happen — usually a dude comes and grabs a
/// weapon."* He is right about the picture and, it turns out, about the
/// trigger: this is fired by leaving a rack, not by entering one.
///
/// # When he appears, which is not where you would look for it
///
/// `Armoury_ClickRack` (`0x004358B0`) is three statements and the **first**
/// one starts the walk:
///
/// ```c
/// FUN_004AABD8(g_selectedCounty, g_armourySelectedType);   /* the OLD rack */
/// g_armourySelectedType = g_uiHotspotId;                   /* now the new  */
/// g_levyBasket[id].latch = g_levyBasket[id].chosen;
/// ```
///
/// so the type handed to the starter is **the rack the player is leaving**, and
/// `FUN_004AABD8`'s own guard is `latch[t] < chosen[t]` — the latch being what
/// `Armoury_ClickRack` wrote when that rack was *opened*. Put together: a
/// soldier walks only when the player assigned at least one man to the weapon
/// he was looking at, and he walks when the player moves on. He is of the type
/// just equipped, he stops under that weapon, he takes it down and he carries
/// it off the right-hand side.
///
/// The latch is `g_levyBasket + 0x0C`. **Nothing else in the binary reads or
/// writes it** — `Armoury_ClickRack` and `FUN_004AABD8` are its only two
/// references — so it is presentation state that happens to be stored in the
/// basket, and it is here for that
/// reason. `[V]`, by grep over the whole decompilation.
///
/// # None of it may reach the simulation
///
/// Every field here is display state. It lives on [`crate::game::LevyOrder`],
/// which is session state the save does not carry and the lockstep digest
/// cannot see (`docs/netcode.md`); a hundred ticks of this leave
/// [`l2_kingdom::Kingdom`] byte-identical, which
/// `crates/l2-game/tests/armoury.rs` asserts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Walker {
    /// `DAT_005679D0` — non-zero while a soldier is on the floor.
    pub active: bool,
    /// `DAT_0056D630` — his x. `Armoury_DrawWalker` blits at it with no
    /// centring at all, so the sprite spans `x … x + 89`.
    pub x: i32,
    /// `DAT_0052F008` — the frame to draw, and the only thing the painter reads.
    pub frame: usize,
    /// `DAT_0057CB10` — the eight-phase walk cycle, stepped on the 80 ms pulse.
    cycle: u8,
    /// `DAT_005681F8` — the pickup counter. It is **1** while he is still
    /// walking in, counts up while he is taking the weapon down, and is put
    /// back to 0 at the end of that, which is what lets him walk again.
    pickup: u8,
    /// `DAT_00568228` — [`WALKER_STOP_X`] for his slot.
    stop_x: i32,
    /// The basket slot he belongs to; [`walker_sheet`] turns it into a sheet.
    pub slot: u8,
    /// `g_levyBasket[t] + 0x0C` — what `chosen` was when rack `t` was opened.
    latch: [i32; 8],
}

impl Default for Walker {
    fn default() -> Walker {
        Walker {
            active: false,
            x: WALKER_START_X,
            frame: 0,
            cycle: 0,
            pickup: 0,
            stop_x: 0,
            slot: 0,
            latch: [0; 8],
        }
    }
}

impl Walker {
    /// `Armoury_ClickRack`'s third statement: `latch[t] = chosen[t]`.
    pub fn latch(&mut self, slot: u8, chosen: i32) {
        if let Some(v) = self.latch.get_mut(slot as usize) {
            *v = chosen;
        }
    }

    /// **`FUN_004AABD8` (`0x004AABD8`)** — start a walk, or decline to.
    ///
    /// ```c
    /// if (0 < type && latch[type] < chosen[type] && walkActive < 1) { … }
    /// ```
    ///
    /// Three guards and all three matter: slot 0 is the unequipped peasants and
    /// has no weapon to fetch, a rack the player looked at without assigning
    /// anybody sends nobody, and a walk already in progress is not restarted.
    /// Returns whether a soldier set off, which is what the test ablates.
    pub fn start(&mut self, slot: u8, chosen: i32) -> bool {
        if slot == 0 || self.active {
            return false;
        }
        if self.latch.get(slot as usize).is_none_or(|&l| l >= chosen) {
            return false;
        }
        self.active = true;
        self.x = WALKER_START_X;
        self.frame = 0;
        self.cycle = 0;
        self.pickup = 1;
        self.slot = slot;
        self.stop_x = WALKER_STOP_X.get(slot as usize).copied().unwrap_or(0);
        true
    }

    /// **`Armoury_DrawWalker`'s state half**, one 20 ms pulse of it.
    ///
    /// ```c
    /// if (walkActive < 1) return;
    /// if (0x280 <= x) { walkActive = 0; return; }
    /// if (pulse20) {
    ///     if (pulse80) {
    ///         if (stopX <= x && pickup != 0) pickup++;
    ///         cycle = (cycle + 1) & 7;
    ///     }
    ///     if (x < stopX || pickup == 0) { x += 4; frame = cycle + (x < stopX ? 0 : 0xD); }
    ///     else { frame = pickup / 3 + 8; if (0xC < frame) { cycle = 0; pickup = 0; } }
    /// }
    /// draw(frame, x, 0xD8);
    /// ```
    ///
/// The last two lines are the seam and they are written out.
    /// tidied: the frame that *overflows* the pickup run is `0x0D`, which is
    /// also the first carrying-walk frame, so the reset happens under a picture
    /// that is already correct and the join is invisible.
    fn pulse(&mut self, pulse80: bool) {
        if !self.active {
            return;
        }
        if self.x >= WALKER_END_X {
            self.active = false;
            return;
        }
        if pulse80 {
            if self.stop_x <= self.x && self.pickup != 0 {
                self.pickup = self.pickup.saturating_add(1);
            }
            self.cycle = (self.cycle + 1) % WALK_PHASES;
        }
        if self.x < self.stop_x || self.pickup == 0 {
            let carrying = self.x >= self.stop_x;
            self.x += WALKER_STEP;
            self.frame = self.cycle as usize + if carrying { CARRY_FIRST } else { 0 };
        } else {
            self.frame = (self.pickup / PICKUP_HOLD) as usize + PICKUP_FIRST;
            if self.frame > PICKUP_LAST {
                self.cycle = 0;
                self.pickup = 0;
            }
        }
    }
}

