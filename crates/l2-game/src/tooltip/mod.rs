//! **The tool tips** — `FUN_00476E95` (`0x00476E95`), the frame function that
//! puts a one-line box beside a resting pointer, and the six functions it calls.
//!
//! `L2.eng` group **220** is its vocabulary and it has no other consumer: *"Null
//! tool tip"* at index 0 and thirty-four tips after it. The Help Options panel's
//! *"Tool tips"* row (`Opt_ToggleToolTips`, `0x004347C7`) flips `g_optToolTips`,
//! and until this module that flag was read by nothing here.
//!
//! # The functions, `[V]`, read out of the decompilation
//!
//! ```c
//! void FUN_00476E95(void) {                            /* once a frame, Battle_Frame */
//!   if (g_optToolTips) {
//!     if (tipId == 0) FUN_00477131();                  /* wait for a rest, then show */
//!     else            FUN_004770B8();                  /* any mouse change hides it */
//!     if (tipId) {
//!       DAT_005AEA40 = 1; DAT_005CD4F8 = 0; g_penAdvance = 0;       /* flat text */
//!       FUN_0040328E(0xDC, tipId, x + 4, y + 4, 0xB4, 200, 0, 0, body, 0x3F);
//!       w = 0xC - (0xB0 - widest) / 16;                /* in 16-pixel units */
//!       h = DAT_005CD4F8 < 0x11 ? 0x16 : 0x28;         /* one line, or more */
//!       FUN_004B414A(x, y, 0x20);                      /* fill w*16 x h */
//!       FUN_0040328E(0xDC, tipId, x + 4, y + 4, 0xB0, 200, 0, 0, body, 0x3F);
//!       DAT_005AEA40 = 0;
//!       FUN_00403CF4(x, y, w << 4, h, 0x3F);           /* the outline, clipped */
//!     }
//!   }
//! }
//! void FUN_00477131(void) {
//!   now = timeGetTime();
//!   if (!g_mouseInputChanged && 999 < (int)(now - stamp)) {
//!     stamp = now;
//!     tipId = FUN_004772B6();                          /* may be 0 */
//!     x = g_mouseX < 0x141 ? g_mouseX + 0x1E : g_mouseX - 0xDC;
//!     y = g_mouseY < 0xF1  ? g_mouseY + 0x1E : g_mouseY - 0x1E;
//!     FUN_00477249();                                  /* clamp to 0 … 0x1B8 */
//!     …save the 192 x 40 backdrop…
//!   } else if (g_mouseInputChanged) stamp = now;
//! }
//! void FUN_004770B8(void) { if (g_mouseInputChanged) { tipId = 0; …restore… } }
//! void FUN_0047703A(void) { if (tipId) { tipId = 0; …restore… } }  /* Screen_Draw */
//! int  FUN_004772B6(void) {
//!   switch (DAT_004D6FB8[g_screenId]) { case 1: return FUN_00477320();
//!                                      case 2: return FUN_004777AA(); }
//!   return 0;
//! }
//! ```
//!
//! * **"The mouse changed" is `g_mouseInputChanged`**, which the frame poll
//!   (`FUN_004B191E`) sets when the position moved *or* either button changed.
//!
//! * **The lookup is not a widget table.** It is a per-screen byte,
//!   `DAT_004D6FB8[g_screenId]` ([`SCREENS`]), choosing one of two pointer
//!   ladders: [`campaign_tip`] for the thirty-five screens that sit on the
//!   campaign map's sidebar, [`battle_tip`] for the battlefield, `0x29`. The
//!   campaign ladder reads live state — the minimap mode, whether the selected
//! county is the player's, and the two produce-row lists — at the moment the
//! tip is resolved, and the id it answers is the group-220 index.
//!
//! * **A repaint takes it away too.** `Screen_Draw` opens with `FUN_0047703A`,
//!   and so do the siege-preparation, battle-prompt, battle-result and outcome
//!   painters and `Smk_PlayThenClose`. It comes back by the ordinary rule — at
//!   once if its stamp is already a second old.
//!
//! [`Tooltips`] is the state and [`Tooltips::frame`] is `FUN_00476E95`'s input
//! half; [`crate::screen::Machine`] owns it, feeds it the pointer and the screen
//! byte, and draws [`draw`] last — `Battle_Frame` calls `FUN_00476E95` after the
//! turn timer, the tip ladder and the message pump.
//!
//! * `g_selectedCounty`'s produce-row lists (`DAT_0053F690`, `DAT_00553FD0`)
//!   are filled by `FUN_0040FEC1` when the county strip is painted. That they
//!   are always the selected county's when a tip resolves is `[I]`; ours
//!   computes them from the selected county at resolve time.
//!
//! * `FUN_0047703A`'s callers are painters. Ours drops the tip when the set of
//!   screens on the stack changes, which is when a painter would run; a
//!   repaint that changes no screen (a toggle's `g_redrawRequest = 2`) is not
//!   modelled, and every such repaint found is preceded by a click, which has
//!   already hidden the tip. `[I]`.

mod lookup;
pub use lookup::*;
mod render;
pub use render::*;

use l2_view::Canvas;

use crate::screen::{Ctx, ScreenId};
use crate::shell::{font, Pen};
use crate::Game;

/// `L2.eng` group 220, the tips' only consumer is this layer.
pub const GROUP: usize = 220;
pub const COUNT: usize = 35;

/// `FUN_00477131`'s `999 < (int)(now - stamp)`, in milliseconds.
pub const REST_MS: u32 = 999;

/// **`DAT_004D6FB8`**, one byte per `g_screenId` `0x00 … 0x43`: `1` the
/// campaign ladder, `2` the battlefield ladder, `0` no tips. `[V]`, read out of
/// `Lords2.exe` and held against the player's own copy by
/// `tests/tooltips.rs`.
pub const SCREENS: [u8; 0x44] = [
    1, 1, 1, 1, 1, 1, 1, 1, 0, 1, 0, 0, 0, 0, 1, 1, // 0x00
    1, 1, 1, 1, 1, 1, 1, 0, 1, 1, 0, 0, 0, 1, 1, 0, // 0x10
    0, 1, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, // 0x20
    0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, // 0x30
    0, 0, 1, 1, // 0x40
];

pub mod ladder {
    /// `FUN_00477320`, [`super::campaign_tip`].
    pub const CAMPAIGN: u8 = 1;
    /// `FUN_004777AA`, [`super::battle_tip`].
    pub const BATTLE: u8 = 2;
}

/// `DAT_004D6FB8[g_screenId]`, zero for a byte past the table.
pub fn ladder_of(screen: Option<u8>) -> u8 {
    screen.and_then(|b| SCREENS.get(b as usize).copied()).unwrap_or(0)
}

pub fn screen_byte(id: ScreenId, game: &Game, mode: Option<u8>) -> Option<u8> {
    use crate::screens::county::Panel;
    use ScreenId as S;
    if let Some(b) = mode {
        return Some(b);
    }
    match id {
        S::Campaign => Some(0x00),
        S::Village(_) => Some(0x02),
        S::Info(_) => Some(0x04),
        S::Merchant(_) => Some(0x08),
        S::Court => Some(0x09),
        S::Armoury(_) => Some(0x0A),
        S::Diplomacy => Some(0x0B),
        S::Trade(..) => Some(0x0C),
        S::Rack(..) => Some(0x0D),
        S::Job(..) => Some(0x0F),
        S::Divide(_) => Some(0x11),
        S::BattlePrompt => Some(0x12),
        S::BattleResult => Some(0x13),
        S::County(_, Panel::Population) => Some(0x14),
        S::County(_, Panel::Tax) => Some(0x15),
        S::County(_, Panel::Happiness) => Some(0x16),
        S::RaiseArmy(_) => Some(0x17),
        S::Supplies(_) => Some(0x18),
        S::County(_, Panel::Ration) => Some(0x19),
        S::DiploCompose(..) => Some(0x1A),
        S::Castle(_) => Some(0x1B),
        S::Conquest => Some(0x1C),
        S::Nobles => Some(0x20),
        S::Siege(_) => Some(0x1D),
        S::Menu | S::Setup(_) => Some(0x1F),
        S::About => Some(0x25),
        S::Movie(_) => Some(0x22),
        S::Tip => Some(0x27),
        S::Battlefield => Some(game.battle.as_ref().map_or(0x29, |b| b.screen_id())),
        S::Ratings => Some(0x2E),
        S::MenuBar(_) => Some(0x32),
        S::SaveLoad(mode) => Some(mode.screen_id()),
        S::Options(page) => page.screen_id(),
        S::Confirm(_) => Some(0x1E),
        S::Message | S::Index => None,
    }
}

pub const MEASURE_WIDTH: i32 = 0xB4;
pub const DRAW_WIDTH: i32 = 0xB0;
/// `FUN_004B414A(x, y, 0x20)`.
pub const FILL: u8 = 0x20;
/// The text and `FUN_00403CF4`'s outline.
pub const INK: u8 = 0x3F;
/// `FUN_0040328E`'s line step in the body font.
pub const LINE: i32 = 0x10;

/// `0xC - (0xB0 - widest) / 16` units of sixteen pixels — C division, toward
/// zero — and 22 pixels tall for one line (`DAT_005CD4F8 < 0x11`), 40 for more.
pub fn box_size(lines: usize, widest: i32) -> (i32, i32) {
    let units = 0xC - (0xB0 - widest) / 16;
    let h = if (lines as i32) * LINE < 0x11 { 0x16 } else { 0x28 };
    (units * 16, h)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shown {
    pub id: u8,
    pub x: i32,
    pub y: i32,
}

/// **The layer's state** — `DAT_004EAC00` the tip, `DAT_004EB268`/`70` its
/// corner, `_DAT_004EA830` the stamp, and our tick count standing in for
/// `timeGetTime`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Tooltips {
    now: u64,
    stamp: Option<u64>,
    shown: Option<Shown>,
}

impl Tooltips {
    pub fn new() -> Tooltips {
        Tooltips::default()
    }

    pub fn shown(&self) -> Option<Shown> {
        self.shown
    }

    /// **`_DAT_004EA830 = 0`** — `Opt_ToggleToolTips` (`0x004347C7`) and
    /// `Map_InitMode` (`0x00498270`). The next still frame shows a tip at once.
    pub fn rearm(&mut self) {
        self.stamp = None;
    }

    /// **`FUN_0047703A` (`0x0047703A`)** — the tip goes, the stamp stays.
    ///
    // arm: 0x0047703A/tip-dropped-by-a-repaint frame
    pub fn drop_tip(&mut self) -> bool {
        self.shown.take().is_some()
    }

    /// **One frame of `FUN_00476E95`'s input half.** `changed` is
    /// `g_mouseInputChanged`, `pointer` is `(g_mouseX, g_mouseY)`, and
    /// `resolve` is `FUN_004772B6`, asked only on the frame a tip is due.
    ///
    // arm: 0x00476E95/tool-tip-rest hover
    pub fn frame(
        &mut self,
        enabled: bool,
        changed: bool,
        pointer: (i32, i32),
        resolve: impl FnOnce(i32, i32) -> u8,
    ) -> bool {
        self.now += 1;
        if !enabled {
            return false;
        }
        if self.shown.is_some() {
            // FUN_004770B8: the stamp is not touched.
            return changed && self.shown.take().is_some();
        }
        // FUN_00477131.
        let rested = self
            .stamp
            .is_none_or(|s| (self.now - s) * crate::TICK_MS as u64 > REST_MS as u64);
        if changed {
            self.stamp = Some(self.now);
            return false;
        }
        if !rested {
            return false;
        }
        self.stamp = Some(self.now);
        let (mx, my) = pointer;
        let id = resolve(mx, my);
        if id == 0 {
            return false;
        }
        let (x, y) = place(mx, my);
        self.shown = Some(Shown { id, x, y });
        true
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    /// `DAT_004D6FB8`'s shape, which the resolver depends on: the four county
    /// panels agree, and the tip's own screen answers nothing.
    #[test]
    fn the_table_answers_the_screens_it_is_about() {
        for b in [0x14, 0x15, 0x16, 0x19, 0x00, 0x10, 0x02] {
            assert_eq!(SCREENS[b], ladder::CAMPAIGN, "screen {b:#04x}");
        }
        assert_eq!(SCREENS[0x29], ladder::BATTLE);
        for b in [0x08, 0x0A, 0x0B, 0x17, 0x1B, 0x1F, 0x27, 0x2A, 0x2B] {
            assert_eq!(SCREENS[b], 0, "screen {b:#04x}");
        }
        assert_eq!(SCREENS.iter().filter(|&&v| v == ladder::CAMPAIGN).count(), 35);
    }
}

