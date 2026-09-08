//! Rendering for Lords of the Realm II: an indexed framebuffer, and the world
//! painted onto it from other crates' state.
//!
//! **This crate is a library of drawing and nothing else.** It has no window,
//! no event loop, no input and no `main`; those live in `l2-game`, which is the
//! workspace's only binary (`docs/plan.md`). Two screens cannot each own the
//! event loop, and the moment there was more than one screen this crate had to
//! stop owning it.
//!
//! Everything here is a **reader** of state. Nothing feeds back into `l2-sim`
//! or `l2-kingdom`: no float, no wall-clock time and no frame counter crosses
//! that line, because the simulation has to stay bit-identical across machines
//! (`docs/netcode.md`).
//!
//! # What is here
//!
//! * [`Canvas`] — a 640 x 480 plane of **palette indices**, and the original's
//!   two blitters. Colour appears only in [`Canvas::to_rgba`].
//! * [`canvas::Tags`] — a parallel plane recording *what* was drawn at each
//!   pixel, which is how the campaign map is picked.
//! * [`campaign`] — the campaign map: a **scrolling viewport** into
//!   `L2_maps.dat`'s lattice at one of the original's two zooms. See
//!   `docs/screens.md`.
//! * [`chrome`] — the original's interface artwork: the `Panels.pl8` framed-box
//!   kit, the `Misc_cty.pl8` right column, and the `MAPnn.PL8` minimap.
//! * [`scene`] — the battlefield viewport and the men on it.
//! * [`figures`] — sprite-frame arithmetic; [`sheet`] — cached PL8 decoding.
//! * [`text`] and [`ink`] — our own 5 x 7 font and the palette-resolved
//!   interface colours. Both are ours, not the original's, and say so.
//!
//! # Where the seam is
//!
//! The simulation is `l2-sim`, and that includes everything with a coordinate
//! in it: `l2_sim::terrain::Battlefield`, `l2_sim::runner::BattleRunner` and
//! `l2_sim::facing`. They used to live here, behind `winit` and `pixels` — 124
//! transitive crates — while every other crate in the workspace is
//! dependency-free *for correctness*, because a lockstep value stream may not
//! be owned by somebody else. This crate now has no third-party dependency at
//! all: the window went up to `l2-game` with `winit` and `pixels`, and the mod
//! overlay went with it, because deciding *which file* to load is not drawing.
//!
//! Everything drawn can be produced, and asserted on, with no window at all —
//! which is what the tests do, and what the eventual pixel diff against
//! `Lords2.exe` will do.

pub mod campaign;
pub mod canvas;
pub mod chrome;
pub mod figures;
pub mod ink;
pub mod scene;
pub mod sheet;
pub mod text;

pub use canvas::{Canvas, Clip, Tags};
pub use ink::Ink;
