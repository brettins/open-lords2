//! The game under lockstep. `docs/netcode.md` §5.

mod sim;

pub use sim::{encode_state, frame, NetGame};
