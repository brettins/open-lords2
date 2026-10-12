//! The game under lockstep. `docs/netcode.md` §5.

mod driver;
mod link;
mod live;
pub mod relay;
pub mod replay;
pub mod resync;
mod status;
mod setup;
mod sim;
mod start;

pub use driver::{Driver, DEFAULT_PORT};
pub use relay::{configure as configure_relay, DEFAULT_RELAY};
pub use link::{lobby, service, Identity, Link, Phase, PortMap, Request};
pub use live::{Live, Stepper, CATCH_UP, STALL_GRACE, TIMEOUT};
pub use status::{draw as draw_status, wrap, Beat};
pub use setup::Setup;

pub use sim::{encode_state, frame, NetGame};
pub use start::{hello, new_game, seats};
