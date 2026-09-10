//! The application spine: the world, the screens, the input and the turn.
//!
//! Six crates worked and none of them were joined into anything a person could
//! play. This is the joint. `docs/plan.md` settles its shape before anyone
//! builds it, and the settled calls are:
//!
//! * **The window, the event loop, input and the screen state machine live
//!   here**, and this is the workspace's only binary. `l2-view` is a library of
//!   drawing.
//! * **The dependency direction is one way.** Nothing below this crate learns
//!   that screens or input exist; `l2-sim` and `l2-kingdom` keep their
//!   manifests exactly as they were.
//! * **A screen returns transitions as values** and cannot reach the stack.
//! * **Simulation time is not frame time.** The renderer draws when it can; the
//!   simulation steps on a fixed tick and never reads a clock.
//! * **One [`Game`] owns the world**, and screens borrow it.
//!
//! # The slice
//!
//! Menu, campaign map, county panel, end turn — starting from the shipped
//! scenario in `lastturn.sav` rather than an invented position. Everything else
//! is refused: the castle designer, diplomacy, video, the multiplayer lobby,
//! and any screen not on that list.
//!
//! Two things have since been let in and are worth naming, because the list
//! above is what stops this crate becoming the whole game. Sieges arrived with
//! their own screens. **Sound arrived as [`audio`]** — the music bed and the
//! message fanfare — and it is allowed here on one condition: it may only ever
//! *read* the world. It is not in [`Ctx`], no screen can reach it, and the
//! event loop derives what should be audible from what already happened. A
//! sound that could change a tick would end the lockstep argument
//! (`docs/netcode.md`).
//!
//! # Everything here is testable without a window
//!
//! `main.rs` is the only file that names `winit` or `pixels`. A screen is
//! handed [`input::Event`]s and a [`Canvas`](l2_view::Canvas), so the tests
//! click on the map, end turns and assert on a `Vec<u8>` of palette indices
//! with nothing on screen — which is also the shape the eventual pixel diff
//! against `Lords2.exe` will take.

pub mod audio;
pub mod battlefield;
pub mod build_id;
pub mod engagement;
pub mod game;
pub mod input;
pub mod message;
pub mod save;
pub mod saves;
pub mod scenario;
pub mod screen;
pub mod screens;
pub mod setup;
pub mod shell;
pub mod text;
pub mod turn;
pub mod victory;
pub mod widget;

pub use game::{Assets, Game};
pub use input::{Event, Key};
pub use screen::{Ctx, Machine, Screen, ScreenId, Transition};
