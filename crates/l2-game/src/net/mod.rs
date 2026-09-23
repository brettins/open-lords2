//! The game under lockstep. `docs/netcode.md` §5.

mod driver;
mod link;
mod live;
mod status;
mod setup;
mod sim;
mod start;

pub use driver::{Driver, DEFAULT_PORT};
pub use link::{lobby, service, Identity, Link, Phase, Request};
pub use live::{Live, Stepper, CATCH_UP, STALL_GRACE};
pub use status::draw as draw_status;
pub use setup::Setup;

pub use sim::{encode_state, frame, NetGame};
pub use start::{hello, new_game, seats};
