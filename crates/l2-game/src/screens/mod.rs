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
//! None of these modules refers to another, with one exception that is worth
//! naming: the county panel calls `map::season_name` to print a season. That is
//! a formatting helper, not a transition — no screen here constructs, owns or
//! pushes another, and the only way from one to another is a [`Transition`]
//! value handed back to the machine.
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

pub mod army;
pub mod battle;
pub mod conquest;
pub mod county;
pub mod divide;
pub mod index;
pub mod job;
pub mod map;
pub mod menu;
pub mod merchant;
pub mod options;
pub mod saveload;
pub mod setup;
pub mod shells;
pub mod siege;
pub mod village;
