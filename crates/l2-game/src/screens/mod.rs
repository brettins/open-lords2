//! The screens: the slice's five, the front end and its thirteen setup pages,
//! the conquest interstitial, and the shells for everything else.
//!
//! [`shells`] is a table rather than a module per screen: each entry names the
//! `g_screenId`, the painter, the `.pl8` it loads and the `L2.eng` group it
//! draws, and one painter walks the table. A screen graduates out of it when
//! there is state behind it to draw.
//!
//! [`index`] is **ours** — the demo's list of every screen, so that the ones
//! the game logic cannot yet open can still be reached.
//!
//! None of these modules refers to another, with two exceptions that are worth
//! naming: the county panel calls `map::season_name` to print a season, and the
//! raise-army screen calls `armoury::page` to paint the room it stands in.
//! Both are painting helpers, not transitions — no screen here constructs, owns
//! or pushes another, and the only way from one to another is a [`Transition`]
//! value handed back to the machine.
//!
//! The second one is the binary's own arrangement rather than a convenience:
//! `Screen_Draw`'s `0x17` arm opens `if (firstFrame == 1) Screen_Armoury(1);`
//! before it calls `Screen_RaiseArmy`, so the levy window really is a box drawn
//! over the armoury. Sharing the painter here is sharing the painter there.
//!
//! **[`Transition::Pass`] extends that rule and does not break it.** A screen
//! may decline an event, and the machine then offers it to the screen
//! underneath — so an event can reach a screen that did not receive it, while
//! the *routing* stays the machine's. Two things follow for anyone adding a
//! screen. Your `handle` may be called for a click that landed on something
//! drawn above you, so decide what is yours by position rather than by assuming
//! the machine filtered it. And if you return `Pass` you are asserting the
//! event is not yours at all: passing by default rather than by decision is how
//! two screens end up both acting on one click. It exists because
//! `Screen_FrameInput`'s per-screen arms are ladders of guards and a guard that
//! returns zero has not consumed the click — `docs/decisions.md` C59.
//!
//! That rule is what makes the village's three-state drag safe to model as one
//! screen: in the original it *is* three screen ids (`0x02`, `0x05`, `0x06`),
//! but all three share a painter and a county, so they are one screen and a
//! phase here. The job popup is genuinely separate — the original reaches it
//! from the village *and* from the campaign sidebar and returns to whichever —
//! so it is its own [`ScreenId`] and the machine owns the stack.
//!
//! # Most of them are insets, and the machine composites them
//!
//! **The original has no screen clear anywhere.** `Screen_Draw` picks a painter
//! and the painter fills a rectangle; whatever is outside it is still there
//! from the last frame. The village is a picture blitted at (64, 64) over a
//! campaign map it repaints itself; the job popup and the four county panels
//! are `Ui_DrawBox` windows over whatever opened them.
//!
//! A screen says so with [`Screen::overlay`], and [`Machine::draw`] paints from
//! the lowest non-overlay screen upward. **That is still not one screen drawing
//! another** — the machine owns the stack and does the compositing, which is
//! exactly the arrangement the paragraph above describes.
//!
//! This module claimed the village was a full screen until a player looked at
//! one and said it was a dialogue. `docs/decisions.md` C22.
//!
//! [`Transition`]: crate::screen::Transition
//! [`ScreenId`]: crate::screen::ScreenId
//! [`Screen::overlay`]: crate::screen::Screen::overlay
//! [`Machine::draw`]: crate::screen::Machine::draw

use crate::input::Event;

/// **Does this event belong to the campaign map's right-hand column?**
///
/// `Screen_FrameInput`'s arms for the village (`0x02`) and for all four county
/// panels (`0x14`, `0x15`, `0x16`, `0x19`) open with the *same six guards*, in
/// the same order, before a single verb of their own:
///
/// | # | guard | what it is |
/// |---|---|---|
/// | 1 | `Minimap_ModeButtonClicked` (`0x0043292D`) | `Hotspot_Test(0x262, 0x20, &g_minimapModeButtons, 4)` |
/// | 2 | `Sidebar_ButtonClicked` (`0x00432967`) | `Hotspot_Test(0x1DE, 0x1AE, &g_sidebarButtons, 6)` |
/// | 3 | `CountyStrip_Click` (`0x00438CEB`) | the 2 × 2 quadrant into the four panels |
/// | 4 | `Labour_SplitSliderDrag` (`0x00439122`) | `x 0x1DE … 0x27F, y 0x101 … 0x128` |
/// | 5 | `CountyStrip_JobClick` (`0x00438E3B`) | the produce rows, `y 0x12E … 0x1AD` |
/// | 6 | `FUN_00439079` | a right release that clears the minimap overlay |
///
/// **Every one of the six hit-tests `x >= 0x1DE`**, which is 478, and nothing
/// else in either arm does. So the rule is exactly *"the column at 478 keeps
/// working"* — `Map_Click` is not in either ladder, and a click on the strip of
/// campaign map either side of an inset does nothing at all.
///
/// The right button is deliberately **not** here. Guard 6 is the only one of the
/// six that reads it, it consumes the click only while an overlay is up, and
/// the arm's *own* right-release — which closes the panel or the village — comes
/// after it. An overlay that passed every right click down would reach the
/// campaign map's information panel instead of closing, which is the opposite
/// behaviour. `docs/arms.json`'s `0x00439079/right-clears-minimap-mode` records
/// what that costs.
///
/// `docs/screens-county.md` §6.4.4a; `docs/decisions.md` C59.
///
/// One record for six guards on five screens, because
/// `docs/arms.json` counts **per implementation** and this is the
/// implementation: the six functions themselves are the campaign map's and are
/// recorded against their own addresses there.
// arm: 0x0042FF10/inset-runs-the-sidebar-guards
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
pub mod options;
pub mod ratings;
pub mod saveload;
pub mod supplies;
pub mod setup;
pub mod shells;
pub mod siege;
pub mod village;
