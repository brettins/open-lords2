//! **A realm must stay in one piece.** `Realm_SecedeIsolatedCounties`
//! (`0x0044AE3C`).
//!
//! Every season, between the unrest counter and the field recount, the game
//! partitions every owned county into **contiguous same-owner blocks** and then
//! gives each realm only its **most populous** block. Every county in any other
//! block declares independence on the spot.
//!
//! ```text
//! Realm_SecedeIsolatedCounties            0x0044AE3C
//! ├── Territory_BuildBlocks               0x0044B042   -> [`build_blocks`]
//! │   ├── Territory_BlockContains         0x0044B4D0
//! │   ├── Territory_ExtendBlock           0x0044B391
//! │   └── Territory_NewBlock              0x0044B302
//! └── Territory_SecedeMinorBlocks         0x0044AE51   -> [`minor_blocks`]
//!     └── County_MakeIndependent          0x004AC3C6   -> `Kingdom::make_county_independent`
//! ```
//!
//! The game names the rule itself, twice:
//!
//! > **`L2.eng` 127** *"Deeming itself too far from the heart of your empire,
//! > this county has declared independence and thrown out your officials."*
//! > **`L2.eng` 128, "Your lands divide."** *"Many of your people, concerned
//! > that they are not part of your main empire, have cast off your yoke of
//! > tyranny and decided to manage their lands themselves."*
//!
//! # What "contiguous" means here
//!
//! **The county neighbour list, not map adjacency.** `Territory_ExtendBlock`
//! joins a county to a block through `County_IsNeighbour` (`0x00467E2C`), which
//! walks the sixteen bytes at county `+0x5C` — [`County::neighbours`] — and
//! nothing else. Two counties whose tiles touch but which are not in each
//! other's list are *not* contiguous for this pass, and two counties that are in
//! each other's list are contiguous however far apart their anchors sit. That
//! list is a property of the scenario, and on the England turn-one fixture it is
//! exactly symmetric across all fourteen counties — 39 undirected edges, no
//! half-edges. **`[V]`**
//!
//! # Three details that look like transcription errors and are not
//!
//! **The partition is not a connected-components walk.** `Territory_BuildBlocks`
//! sweeps the counties in index order, offering each unplaced one to
//! `Territory_ExtendBlock`, and **never merges two blocks that a newly placed
//! county would join**. A county placed into block A that also neighbours block
//! B leaves A and B separate for that sweep —
//! function sweeps repeatedly (up to [`SWEEP_CAP`] times) instead of once,
//! why [`build_blocks`] is a fixpoint loop. Run to a
//! fixpoint the two agree; run once they do not.
//!
//! **The winner is the most populous block, not the largest one.**
//! `Territory_BuildBlocks`' last loop sums each block's members' populations
//! into the block's `+0x00`, and that sum is the only key
//! `Territory_SecedeMinorBlocks` ranks on. A realm holding one huge county and
//! three small ones loses the three.
//!
//! **A tie goes to the *highest* block index.** The comparison is
//! `if (best <= block.population)` scanning upward from block 0, so an equal
//! population *overwrites* the incumbent. `docs/symbols.json` said "ties go to
//! the lowest block index, because the comparison is `<=`" — the operator is
//! right and the conclusion is backwards. See [`minor_blocks`] and
//! `docs/decisions.md` C34.
//!
//! # The anchor, stated plainly
//!
//! Every realm in every fixture
//! in `E:\dev\lords2-fixtures` owns exactly one county, so "each realm's
//! counties form one connected component" holds trivially in all six saves and
//! proves nothing. What holds it up is the code above, the two `L2.eng` strings,
//! and **a player's recollection that cut-off counties do secede in play**. That
//! is corroboration, not reproduction. `docs/kingdom.md` §6.1.

mod partition;
pub use partition::*;
mod secession;
pub use secession::*;
mod tests_part;
pub use tests_part::*;

use crate::county::County;
use crate::realm::MAX_REALMS;

/// `g_territoryBlocks` (`0x00568240`) holds seventeen slots of stride `0x1C`.
///
/// Seventeen for sixteen counties, so `Territory_NewBlock`'s missing bounds
/// check on the last slot cannot be reached.
pub const MAX_BLOCKS: usize = 17;

/// Twenty member ids a block — block `+0x04 … +0x17`, one byte each.
pub const MAX_BLOCK_MEMBERS: usize = 20;

/// `Territory_BuildBlocks`' sweep cap. **A real bound, not a loop guard**: the
/// counter is compared against the count of counties already placed,
/// pathological map stops after a hundred sweeps with counties unplaced rather
/// than spinning.
pub const SWEEP_CAP: u32 = 100;

/// One contiguous block of same-owner counties — `g_territoryBlocks + n*0x1C`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Block {
    /// `+0x00` — the sum of the members' populations. **The key the secession
    /// pass ranks on**, filled by `Territory_BuildBlocks`' last loop.
    pub population: i32,
    /// `+0x04` — the owning realm, 1..=5. Zero means the slot is free,
    /// list is dense and zero-terminated: every scan stops at the first zero.
    pub owner: u8,
    /// `+0x05 … +0x18` — the member county ids, zero-padded.
    pub members: [u8; MAX_BLOCK_MEMBERS],
}

impl Block {
    pub const EMPTY: Block =
        Block { population: 0, owner: 0, members: [0; MAX_BLOCK_MEMBERS] };

    /// The member ids present, in the order they were placed.
    pub fn members(&self) -> impl Iterator<Item = u8> + '_ {
        self.members.iter().copied().filter(|&id| id != 0)
    }

    pub fn len(&self) -> usize {
        self.members().count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn push(&mut self, county: u8) -> bool {
        for slot in self.members.iter_mut() {
            if *slot == 0 {
                *slot = county;
                return true;
            }
        }
        false
    }

    fn contains(&self, county: u8) -> bool {
        self.members.contains(&county)
    }
}

/// The seventeen slots, as one value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Blocks(pub [Block; MAX_BLOCKS]);

impl Default for Blocks {
    fn default() -> Self {
        Blocks::empty()
    }
}

impl Blocks {
    pub const fn empty() -> Blocks {
        Blocks([Block::EMPTY; MAX_BLOCKS])
    }

    /// The occupied slots. `g_territoryBlockCount` (`0x00553EF4`) is this count
    /// — **written by `Territory_BuildBlocks` and, as far as the corpus shows,
    /// read nowhere**: the secession pass rescans for a matching owner instead.
    pub fn count(&self) -> usize {
        self.0.iter().filter(|b| b.owner != 0).count()
    }

    /// The blocks of one realm, with their slot indices — the order the
    /// original scans, which is what decides a tie.
    pub fn of_realm(&self, realm: u8) -> impl Iterator<Item = (usize, &Block)> + '_ {
        self.0.iter().enumerate().filter(move |(_, b)| b.owner == realm)
    }
}

