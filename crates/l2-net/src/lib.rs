//! Deterministic primitives and the lockstep session core for the
//! lords2 engine.
//!
//! This crate implements `docs/netcode.md`. Read that first — it is the
//! design and this is the implementation, and where the two differ the
//! difference is marked in a doc comment on the type that differs.
//!
//! # What this crate is, in one sentence
//!
//! Everything two machines must agree on bit for bit, and nothing else.
//!
//! # The three layers
//!
//! **The deterministic primitives** are the load-bearing part, and they
//! are load-bearing for code that is not networking at all. `l2-sim`
//! depends on this crate for them; this crate never depends on
//! `l2-sim`.
//!
//! * [`Pcg32`] — the simulation's random number generator, frozen
//!   in-tree with published reference vectors (D-3).
//! * [`Fixed`] — Q16.16 arithmetic, because floats are banned and
//!   fractions are not (D-1, D-2).
//! * [`Canonical`] and [`XxHash64`] — one canonical byte stream, hashed
//!   as it is written, serving the checksum, the snapshot and the
//!   desync dump so they cannot disagree (§6, D-10).
//!
//! **The lockstep core** collects commands, orders them identically on
//! every peer, steps the simulation, and stops the game the moment two
//! peers' checksums differ.
//!
//! * [`Session`] — the tick queue and the whole of §4 and §5.
//! * [`Command`], [`order_commands`] — the ordering rule.
//! * [`History`], [`Divergence`], [`DesyncDump`] — §6.
//! * [`Replay`] — the check that pays for the design.
//!
//! **The transport seam** is one trait, framing, and an in-process
//! network to test against.
//!
//! * [`Transport`], [`frame`], [`FrameReader`], [`Loopback`].
//!
//! # Testable without a network, deliberately
//!
//! The whole crate runs on a bare checkout with no game install, no
//! sockets and no threads. That is not a convenience: §6's central
//! claim is that two simulations fed identical commands must produce
//! identical checksums, and a test for that which needs two machines is
//! a test that runs once a quarter. `tests/lockstep.rs` runs two
//! sessions in one process over a [`Loopback`] and checks it every
//! time, with latency, reordering and a deliberately perturbed peer.
//!
//! # What is not here
//!
//! Every one of these is a decision, not an oversight.
//!
//! * **No socket implementation.** Argued at length in
//!   [`transport`]. Everything a TCP transport needs is here and
//!   tested; the sockets themselves are not, because a networking layer
//!   whose tests are skipped by default is code that looks finished and
//!   has never run.
//! * **No `std::fs`.** A desync dump becomes bytes here and becomes a
//!   file in the engine. Keeping I/O out means the tests need no
//!   temporary directory, and it means nothing in this crate can
//!   accidentally violate D-11's "no file I/O inside `step()`".
//! * **No `std::time`.** D-5 forbids the simulation from seeing a
//!   clock, and the surest way to obey that is to have no clock to
//!   consult. [`Session`] therefore waits forever; timeouts belong to
//!   the caller, which has a render loop and a UI.
//! * **No threads.** The reader thread §7 recommends belongs to the
//!   transport implementation, not to the session.
//! * **No rollback, no prediction, no interest management, no
//!   encryption, no matchmaking.** §8, each for its own reason.
//! * **No dependencies.** Argued in `Cargo.toml`: every value stream
//!   here must be frozen forever, and a dependency is a value stream
//!   owned by somebody else.
//!
//! # A worked example
//!
//! Two peers, one process, no network — which is also how the tests are
//! written.
//!
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
pub mod lockstep;
pub mod packet;
pub mod replay;
pub mod rng;
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
pub use lockstep::{
    state_digest, state_hash, state_snapshot, Advance, Config, Session, SessionError, Simulation,
};
pub use packet::{
    Ack, HaltReason, Hello, Message, Mismatch, TickPacket, DEFAULT_PORT, MAX_ACKS,
    PROTOCOL_VERSION,
};
pub use replay::{Replay, ReplayMismatch, REPLAY_MAGIC, REPLAY_VERSION};
pub use rng::{Pcg32, DEFAULT_STREAM};
pub use transport::{
    frame, Endpoint, FrameReader, Loopback, PeerId, Transport, TransportError, MAX_FRAME,
};
