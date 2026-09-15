//! **[`Transition::Pass`] extends that rule and does not break it.** A screen
//! may decline an event
//! underneath — so an event can reach a screen that did not receive it, while
//! the *routing* stays the machine's. Two things follow for anyone adding a
//! screen. Your `handle` may be called for a click that landed on something
//! drawn above you, so decide what is yours by position
//! the machine filtered it. And if you return `Pass` you are asserting the
//! event is not yours at all: passing by default is how
//! two screens end up both acting on one click. It exists because
//! `Screen_FrameInput`'s per-screen arms are ladders of guards and a guard that
//! returns zero has not consumed the click — `docs/decisions.md` C59.
//!
//! This module claimed the village was a full screen until a player looked at
//! one and said it was a dialogue. `docs/decisions.md` C22.

use crate::input::Event;

/// | # | guard | what it is |
/// |---|---|---|
/// | 1 | `Minimap_ModeButtonClicked` (`0x0043292D`) | `Hotspot_Test(0x262, 0x20, &g_minimapModeButtons, 4)` |
/// | 2 | `Sidebar_ButtonClicked` (`0x00432967`) | `Hotspot_Test(0x1DE, 0x1AE, &g_sidebarButtons, 6)` |
/// | 3 | `CountyStrip_Click` (`0x00438CEB`) | the 2 × 2 quadrant into the four panels |
/// | 4 | `Labour_SplitSliderDrag` (`0x00439122`) | `x 0x1DE … 0x27F, y 0x101 … 0x128` |
/// | 5 | `CountyStrip_JobClick` (`0x00438E3B`) | the produce rows, `y 0x12E … 0x1AD` |
/// | 6 | `FUN_00439079` | a right release that clears the minimap overlay |
///
/// The right button is deliberately **not** here. Guard 6 is the only one of the
/// six that reads it, it consumes the click only while an overlay is up, and
/// the arm's *own* right-release — which closes the panel or the village — comes
/// after it. An overlay that passed every right click down would reach the
/// campaign map's information panel instead of closing
/// behaviour. `docs/arms.json`'s `0x00439079/right-clears-minimap-mode` records
/// what that costs.
///
/// `docs/screens-county.md` §6.4.4a; `docs/decisions.md` C59.
///
// arm: 0x0042FF10/inset-runs-the-sidebar-guards left-press
pub fn belongs_to_the_right_column(event: Event) -> bool {
    let x = match event {
        Event::Click { x, .. } | Event::Release { x, .. } | Event::Pointer { x, .. } => x,
        _ => return false,
    };
    x >= l2_view::campaign::PANEL_X
}

pub mod about;
pub mod armoury;
pub mod army;
pub mod battle;
pub mod battlefield;
pub mod castle;
pub mod confirm;
pub mod conquest;
pub mod county;
pub mod court;
pub mod diplomacy;
pub mod divide;
pub mod index;
pub mod info;
pub mod job;
pub mod map;
pub mod menu;
pub mod menubar;
pub mod merchant;
pub mod message;
pub mod movie;
pub mod nobles;
pub mod options;
pub mod ratings;
pub mod saveload;
pub mod supplies;
pub mod setup;
pub mod shells;
pub mod siege;
pub mod tip;
pub mod village;
