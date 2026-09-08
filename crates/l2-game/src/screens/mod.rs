//! The screens. Five of them, which is the slice.
//!
//! None of these modules refers to another, with one exception that is worth
//! naming: the county panel calls `map::season_name` to print a season. That is
//! a formatting helper, not a transition — no screen here constructs, owns or
//! pushes another, and the only way from one to another is a [`Transition`]
//! value handed back to the machine.
//!
//! That rule is what makes the village's three-state drag safe to model as one
//! screen: in the original it *is* three screen ids (`0x02`, `0x05`, `0x06`),
//! but all three share a painter and a county, so they are one screen and a
//! phase here. The job popup is genuinely separate — the original reaches it
//! from the village *and* from the campaign sidebar and returns to whichever —
//! so it is its own [`ScreenId`] and the machine owns the stack.
//!
//! [`Transition`]: crate::screen::Transition
//! [`ScreenId`]: crate::screen::ScreenId

pub mod county;
pub mod job;
pub mod map;
pub mod menu;
pub mod village;
