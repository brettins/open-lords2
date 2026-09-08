//! Rendering for Lords of the Realm II: an indexed framebuffer, the battlefield
//! the `.skr` files describe, and the figures that fight on it.
//!
//! Everything here is a **reader** of state. Nothing in this crate feeds back
//! into `l2-sim`: no float, no wall-clock time and no frame counter crosses
//! that line, because the simulation has to stay bit-identical across machines
//! (`docs/netcode.md`). The battle driver in [`battle`] is the one place the
//! two meet, and it is integer-only for the same reason.
//!
//! The window lives in `main.rs`. Everything drawn can be produced, and
//! asserted on, with no window at all — which is what the tests do, and what
//! the eventual pixel diff against `Lords2.exe` will do.

pub mod battle;
pub mod canvas;
pub mod figures;
pub mod scene;
pub mod sheet;
pub mod terrain;

pub use canvas::Canvas;
pub use terrain::Battlefield;
