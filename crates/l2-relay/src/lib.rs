//! The join-code relay, protocol v1 (`docs/netcode.md` §7 *Join codes and the
//! relay*, C254). A host holds a control connection and is given a code; a
//! joiner names the code; the relay pairs the two and copies bytes.

pub mod client;
pub mod protocol;
mod pipe;
mod rand;
mod server;
mod session;

pub use protocol::{as_code, ALPHABET, CODE_LEN, LINE_MAX, TOKEN_LEN, VERSION};
pub use server::{Config, Relay};
