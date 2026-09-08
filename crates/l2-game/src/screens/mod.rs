//! The screens. Three of them, which is the slice.
//!
//! None of these modules refers to another, with one exception that is worth
//! naming: the county panel calls `map::season_name` to print a season. That is
//! a formatting helper, not a transition — no screen here constructs, owns or
//! pushes another, and the only way from one to another is a [`Transition`]
//! value handed back to the machine.
//!
//! [`Transition`]: crate::screen::Transition

pub mod county;
pub mod map;
pub mod menu;
