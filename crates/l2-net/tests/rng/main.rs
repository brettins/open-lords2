//! The PRNG's value stream, pinned.
//!
//! `docs/netcode.md` D-3 requires the generator's output to be frozen
//! *forever*, because a change to it is a silent desync between a
//! player who updated and one who did not, and between a replay and its
//! recording. This file is the mechanism that makes "forever" mean
//! something: **if you change `src/rng.rs` and a test here fails, the
//! test is right.**
//!
//! The vectors come in two kinds and the difference matters.
//!
//! * [`matches_the_published_pcg32_demo`] checks our transcription
//!   against *somebody else's* published numbers — the output of
//!   O'Neill's `pcg32-demo` from `pcg-c-basic` seeded `(42, 54)`, which
//!   is quoted on pcg-random.org. That is the only test here that can
//!   catch a mistake in the algorithm itself; every other vector below
//!   would happily pin a wrong implementation.
//! * The rest pin *our* conventions — the seeding dance, the order of
//!   the two draws in `next_u64`, the rejection method in `below`, the
//!   direction of the shuffle. Those are ours to choose and ours to
//!   freeze, and they are recorded here because they are not written
//!   down anywhere else in the world.

mod vectors;
pub use vectors::*;
mod ranges;
pub use ranges::*;
mod operations;
pub use operations::*;

use l2_net::{Canonical, Pcg32};

