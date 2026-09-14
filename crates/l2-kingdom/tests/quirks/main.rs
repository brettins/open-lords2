//! **Every switch, flipped, with the simulation watched.**
//!
//! `docs/agents.md`: *a field is only tested if something a test reads was
//! written by something the game runs.* The quirk-shaped version of that, and
//! the reason this file exists:
//!
//! > **A quirk switch that nothing reads is worse than no switch**, because it
//! > claims a behaviour is configurable when it is not.
//!
//! So there is one test per [`Quirk`], each of the same shape: run the *same*
//! rule twice over the *same* state, once faithful and once fixed, and assert
//! the two answers differ **and** that each is the answer it should be. Asserting
//! only that they differ would pass for a switch that broke the rule in some
//! third way.
//!
//! `crates/l2-testkit/tests/quirks_catalogue.rs` is the other half: it checks
//! that the switch list and `docs/bugs.md` are the same list. It can tell that a
//! variant is *named* by the simulation; only this file can tell that flipping
//! it changes an answer.
//!
//! # The two that are about the wire, not about a rule
//!
//! [`the_quirk_set_is_inside_the_lockstep_digest`] and
//! [`a_saved_game_remembers_which_bugs_it_was_played_with`] are the ones that
//! would have caught the failure this project has had five times — a field in
//! neither the save nor the digest, with a green suite over it
//! (`docs/decisions.md` C30). A quirk that did not reach the digest would let
//! two peers with different settings agree on a checksum while computing
//! different games, which is the exact defect this engine exists to replace.

mod economy;
pub use economy::*;
mod counties;
pub use counties::*;
mod victory;
pub use victory::*;
mod wire;
pub use wire::*;

use l2_kingdom::county::County;
use l2_kingdom::kingdom::Kingdom;
use l2_kingdom::realm::Realm;
use l2_kingdom::tables::{Season, Tables, Weather};
use l2_kingdom::{Quirk, Quirks};

const T: &Tables = &Tables::DEFAULT;

/// The two settings for one quirk, so every test below reads the same way:
/// `(faithful, fixed)`.
fn pair(q: Quirk) -> (Quirks, Quirks) {
    let faithful = Quirks::FAITHFUL;
    let mut fixed = Quirks::FAITHFUL;
    fixed.set_reproduced(q, false);
    assert!(faithful.reproduces(q) && !fixed.reproduces(q));
    (faithful, fixed)
}

// ---------------------------------------------------------------------------
// B1 — the harvest weather band throws away the labour cap
// ---------------------------------------------------------------------------

