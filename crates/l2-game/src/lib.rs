
/// It lives in the library because a screen that reproduces an animation whose
/// rate is stated in *milliseconds* has to convert — the campaign map's
/// industry wheels turn on `Tick_Pulses`' 80/160/320/640 ms rungs
/// (`0x004BBC80`), and 640 ms is forty of these. Nothing below this crate may
/// read a clock (`docs/netcode.md` D-12) and this is not one: it is the length
/// of a tick, not the time.
pub const TICK_MS: u32 = 16;

pub mod arrival;
pub mod audio;
pub mod batfield;
pub mod battlefield;
pub mod build_id;
pub mod castle;
pub mod clock;
pub mod cursor;
pub mod engagement;
pub mod game;
pub mod input;
pub mod message;
pub mod movie;
pub mod press;
pub mod save;
pub mod saves;
pub mod scenario;
pub mod screen;
pub mod screens;
pub mod setup;
pub mod shell;
pub mod text;
pub mod tip;
pub mod tooltip;
pub mod turn;
pub mod turn_clock;
pub mod victory;
pub mod wallclock;
pub mod widget;

pub use game::{Assets, Game};
pub use input::{Event, Key};
pub use screen::{Ctx, Machine, Screen, ScreenId, Transition};
