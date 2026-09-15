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

fn pair(q: Quirk) -> (Quirks, Quirks) {
    let faithful = Quirks::FAITHFUL;
    let mut fixed = Quirks::FAITHFUL;
    fixed.set_reproduced(q, false);
    assert!(faithful.reproduces(q) && !fixed.reproduces(q));
    (faithful, fixed)
}


