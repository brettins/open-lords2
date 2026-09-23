//! The game under lockstep. `docs/netcode.md` §5.

mod sim;
mod start;

pub use sim::{encode_state, frame, NetGame};
pub use start::{hello, new_game, seats};
