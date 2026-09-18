//! ```
//! use l2_net::*;
//!
//! // A simulation small enough to read, deterministic enough to mean
//! // something: a counter stirred by the frozen PRNG.
//! struct Toy { rng: Pcg32, total: i64 }
//!
//! impl Simulation for Toy {
//!     fn step(&mut self, _tick: Tick, commands: &[Command]) {
//!         for command in commands {
//!             self.total += command.payload.len() as i64;
//!         }
//!         self.total += self.rng.below(100) as i64;
//!     }
//!     fn encode_state(&self, out: &mut Canonical) {
//!         out.section("rng");
//!         out.encode(&self.rng);
//!         out.section("total");
//!         out.i64(self.total);
//!     }
//! }
//!
//! let seed = 0x1234_5678;
//! let slots = [PlayerSlot::new(0), PlayerSlot::new(1)];
//! let mut sims = [
//!     Toy { rng: Pcg32::from_seed(seed), total: 0 },
//!     Toy { rng: Pcg32::from_seed(seed), total: 0 },
//! ];
//! let mut sessions = [
//!     Session::new(Config::battle(), slots[0], &slots, seed, &sims[0]),
//!     Session::new(Config::battle(), slots[1], &slots, seed, &sims[1]),
//! ];
//!
//! for _ in 0..20 {
//!     // Each peer seals its packets and hands them to the other.
//!     let mut outgoing = Vec::new();
//!     for (i, session) in sessions.iter_mut().enumerate() {
//!         while let Some(packet) = session.next_packet() {
//!             outgoing.push((i, packet));
//!         }
//!     }
//!     for (from, packet) in outgoing {
//!         let to = 1 - from;
//!         sessions[to].receive(packet).unwrap();
//!     }
//!     for i in 0..2 {
//!         let _ = sessions[i].advance(&mut sims[i]);
//!     }
//! }
//!
//! // The property the whole design rests on.
//! assert_eq!(state_hash(&sims[0]), state_hash(&sims[1]));
//! assert!(!sessions[0].is_halted());
//! ```

pub mod canonical;
pub mod command;
pub mod desync;
pub mod fixed;
pub mod hash;
pub mod lobby;
pub mod lockstep;
pub mod packet;
pub mod quirks;
pub mod replay;
pub mod rng;
pub mod tcp;
pub mod transport;

pub use canonical::{
    decode_all, Canonical, CodecError, Decode, Digest, Encode, Reader, SectionDigest,
    CHECKSUM_SEED,
};
pub use command::{
    commands_are_distinct, order_commands, Command, PlayerSlot, Tick, MAX_PLAYERS,
};
pub use desync::{DesyncDump, Divergence, DumpComparison, History, HISTORY};
pub use fixed::{Fixed, FRAC_BITS};
pub use hash::{xxhash64, XxHash64};
pub use lobby::{
    Join, Lobby, LobbyError, LobbyEvent, Outgoing, Player, Role, Roster, Start, MAX_NAME,
};
pub use lockstep::{
    state_digest, state_hash, state_snapshot, Advance, Config, Session, SessionError, Simulation,
};
pub use packet::{
    Ack, HaltReason, Hello, Message, Mismatch, TickPacket, DEFAULT_PORT, MAX_ACKS,
    PROTOCOL_VERSION,
};
pub use quirks::{Group, Quirk, Quirks};
pub use replay::{Replay, ReplayMismatch, REPLAY_MAGIC, REPLAY_VERSION};
pub use rng::{Pcg32, DEFAULT_STREAM};
pub use tcp::{TcpTransport, MAX_OUTBOX};
pub use transport::{
    frame, Endpoint, FrameReader, Loopback, PeerId, Transport, TransportError, MAX_FRAME,
};
