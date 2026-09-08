//! The screens: the slice's four, the front end and its thirteen setup pages,
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
//! [`Transition`]: crate::screen::Transition

pub mod conquest;
pub mod county;
pub mod index;
pub mod map;
pub mod menu;
pub mod setup;
pub mod shells;
