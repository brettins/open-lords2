//! Rendering for Lords of the Realm II: an indexed framebuffer, the men and the
//! battlefield drawn from `l2-sim`'s state.
//!
//! Everything here is a **reader** of state. Nothing in this crate feeds back
//! into `l2-sim`: no float, no wall-clock time and no frame counter crosses
//! that line, because the simulation has to stay bit-identical across machines
//! (`docs/netcode.md`).
//!
//! # Where the seam is
//!
//! The simulation is `l2-sim`, and that now includes everything with a
//! coordinate in it: `l2_sim::terrain::Battlefield` (the cell array the mover
//! and the pathfinder read), `l2_sim::runner::BattleRunner` (figure positions,
//! cell occupancy, deployment, the unit array and the tick loop) and
//! `l2_sim::facing` (which way a step points a man). They used to live here,
//! behind `winit` and `pixels` — 124 transitive crates — while every other
//! crate in the workspace is dependency-free *for correctness*, because a
//! lockstep value stream may not be owned by somebody else.
//!
//! What stays here is everything that turns that state into bytes: the
//! [`Canvas`] framebuffer, the sprite-frame arithmetic in [`figures`], the
//! tile viewport in [`scene`], PL8 decoding in [`sheet`], and the window in
//! `main.rs`. The dependency points one way — `l2-view` reads `l2-sim`, and
//! `l2-sim` does not know a renderer exists.
//!
//! Everything drawn can be produced, and asserted on, with no window at all —
//! which is what the tests do, and what the eventual pixel diff against
//! `Lords2.exe` will do.

pub mod canvas;
pub mod figures;
pub mod scene;
pub mod sheet;

pub use canvas::Canvas;
