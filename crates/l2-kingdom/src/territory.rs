//! **A realm must stay in one piece.** `Realm_SecedeIsolatedCounties`
//! (`0x0044AE3C`), and the two functions it is.
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
//! B leaves A and B separate for that sweep — which is precisely why the
//! function sweeps repeatedly (up to [`SWEEP_CAP`] times) instead of once, and
//! why [`build_blocks`] is a fixpoint loop rather than a flood fill. Run to a
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
//! `docs/decisions.md` C32.
//!
//! # The anchor, stated plainly
//!
//! **There is no data-side oracle for this pass.** Every realm in every fixture
//! in `E:\dev\lords2-fixtures` owns exactly one county, so "each realm's
//! counties form one connected component" holds trivially in all six saves and
//! proves nothing. What holds it up is the code above, the two `L2.eng` strings,
//! and **a player's recollection that cut-off counties do secede in play**. That
//! is corroboration, not reproduction. `docs/kingdom.md` §6.1.

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
/// counter is compared against the count of counties already placed, so a
/// pathological map stops after a hundred sweeps with counties unplaced rather
/// than spinning.
pub const SWEEP_CAP: u32 = 100;

/// One contiguous block of same-owner counties — `g_territoryBlocks + n*0x1C`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Block {
    /// `+0x00` — the sum of the members' populations. **The key the secession
    /// pass ranks on**, filled by `Territory_BuildBlocks`' last loop.
    pub population: i32,
    /// `+0x04` — the owning realm, 1..=5. Zero means the slot is free, and the
    /// list is dense and zero-terminated: every scan stops at the first zero.
    pub owner: u8,
    /// `+0x05 … +0x18` — the member county ids, zero-padded.
    pub members: [u8; MAX_BLOCK_MEMBERS],
}

impl Block {
    pub const EMPTY: Block =
        Block { population: 0, owner: 0, members: [0; MAX_BLOCK_MEMBERS] };

    /// The member ids actually present, in the order they were placed.
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

/// `Territory_BlockContains` (`0x0044B4D0`) — is this county already in a block
/// of **its own owner**?
///
/// The scan stops at the first slot with owner 0, which is the whole reason the
/// block list has to stay dense.
fn block_contains(blocks: &Blocks, counties: &[County], county: u8) -> bool {
    let owner = counties[county as usize].owner;
    for block in blocks.0.iter() {
        if block.owner == 0 {
            return false;
        }
        if block.owner == owner && block.contains(county) {
            return true;
        }
    }
    false
}

/// `Territory_ExtendBlock` (`0x0044B391`) — add `county` to the **first** block
/// of the same owner that already holds one of its neighbours.
///
/// Returns false if no block will take it. It never merges, and a full block
/// (twenty members) silently refuses — the inner loop finds no free slot and
/// falls out to the next block.
fn extend_block(blocks: &mut Blocks, counties: &[County], county: u8) -> bool {
    let owner = counties[county as usize].owner;
    for block in blocks.0.iter_mut() {
        if block.owner == 0 {
            return false;
        }
        if block.owner != owner {
            continue;
        }
        let adjacent = block
            .members()
            .any(|member| is_neighbour(counties, county, member));
        if adjacent && block.push(county) {
            return true;
        }
    }
    false
}

/// `Territory_NewBlock` (`0x0044B302`) — open the first free slot with this
/// county's owner and the county as its only member.
fn new_block(blocks: &mut Blocks, counties: &[County], county: u8) -> bool {
    for block in blocks.0.iter_mut() {
        if block.owner == 0 {
            block.owner = counties[county as usize].owner;
            block.members[0] = county;
            return true;
        }
    }
    false
}

/// `County_IsNeighbour` (`0x00467E2C`) — is `other` in `county`'s adjacency
/// list?
///
/// [`County::neighbours`] is the same sixteen bytes at `+0x5C`, walked in
/// stored order and stopping at the first zero.
pub fn is_neighbour(counties: &[County], county: u8, other: u8) -> bool {
    counties
        .get(county as usize)
        .is_some_and(|c| c.neighbours().contains(&other))
}

/// `Territory_BuildBlocks` (`0x0044B042`) — partition every owned county into
/// contiguous same-owner blocks.
///
/// The loop, exactly:
///
/// ```c
/// placed = 0; sweeps = 0;
/// while (placed < owned && ++sweeps < 100) {
///     placed = 0; progress = false;
///     for (c = 1; c <= countyCount; c++)
///         if (owned(c)) { if (inBlock(c)) placed++;
///                         else if (extend(c)) progress = true; }
///     for (c = 1; c <= countyCount && !progress; c++)
///         if (owned(c) && !inBlock(c) && newBlock(c)) progress = true;
/// }
/// ```
///
/// **A fresh block is opened only when a whole sweep extended nothing.** That
/// is what keeps a block from being started for a county that a later sweep
/// would have attached to an existing one, and it is why the pass converges on
/// the connected components even though `extend` never merges.
pub fn build_blocks(counties: &[County], county_count: usize) -> Blocks {
    let mut blocks = Blocks::empty();
    let owned = |c: usize| c < counties.len() && counties[c].owner != 0;
    let total_owned = (1..=county_count).filter(|&c| owned(c)).count();

    let mut placed = 0usize;
    let mut sweeps = 0u32;
    while placed < total_owned {
        sweeps += 1;
        if sweeps >= SWEEP_CAP {
            break;
        }
        placed = 0;
        let mut progress = false;
        for c in 1..=county_count {
            if !owned(c) {
                continue;
            }
            if block_contains(&blocks, counties, c as u8) {
                placed += 1;
            } else if extend_block(&mut blocks, counties, c as u8) {
                progress = true;
            }
        }
        for c in 1..=county_count {
            if progress {
                break;
            }
            if owned(c)
                && !block_contains(&blocks, counties, c as u8)
                && new_block(&mut blocks, counties, c as u8)
            {
                progress = true;
            }
        }
    }

    // The last loop: each occupied block's `+0x00` becomes the sum of its
    // members' populations. Nothing else writes it, and it is the only key the
    // secession pass ranks on.
    for block in blocks.0.iter_mut() {
        if block.owner == 0 {
            continue;
        }
        block.population = block
            .members
            .iter()
            .filter(|&&id| id != 0)
            .map(|&id| counties[id as usize].population)
            .sum();
    }
    blocks
}

/// What [`minor_blocks`] found for one realm.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Secession {
    /// The realm that is about to lose land, 1..=5.
    pub realm: u8,
    /// How many blocks it held. The message is suppressed when this is 1 —
    /// which is also when [`Secession::counties`] is empty.
    pub blocks: usize,
    /// The slot index of the block it keeps.
    pub kept: usize,
    /// Every county that goes independent, in the order
    /// `Territory_SecedeMinorBlocks` walks them: block slot ascending, and
    /// member slot ascending inside each block.
    pub counties: Vec<u8>,
}

/// `Territory_SecedeMinorBlocks` (`0x0044AE51`), as a decision — which counties
/// each realm loses, without touching them.
///
/// Realms **1 … 5** with a non-zero [`crate::Realm::strength`] are considered.
/// A realm out of play is skipped, and so is realm 0 — an unowned county was
/// never in a block, so a scatter of neutral counties never "secedes".
///
/// **The human is not treated differently.** The only branch on
/// `g_localPlayer` in the whole pass is the message: `0x7F` when exactly one
/// county went and `0x80` when more, sent only to the local player and only
/// when the realm held more than one block. **An AI loses its outlying counties
/// in silence** — which is why this returns the decision for every realm and
/// leaves the filtering to whoever raises the messages.
pub fn minor_blocks(blocks: &Blocks, strength: &[u8]) -> Vec<Secession> {
    let mut out = Vec::new();
    for realm in 1..MAX_REALMS as u8 {
        if strength.get(realm as usize).copied().unwrap_or(0) == 0 {
            continue;
        }
        let mut count = 0usize;
        let mut kept = 0usize;
        let mut best = 0i32;
        for (slot, block) in blocks.of_realm(realm) {
            count += 1;
            // `<=`, scanning upward: an equal population **overwrites** the
            // incumbent, so a tie goes to the *highest* block index.
            if best <= block.population {
                kept = slot;
                best = block.population;
            }
        }
        if count == 0 {
            continue;
        }
        let mut lost = Vec::new();
        for (slot, block) in blocks.of_realm(realm) {
            if slot == kept {
                continue;
            }
            lost.extend(block.members());
        }
        out.push(Secession { realm, blocks: count, kept, counties: lost });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `n` counties in a line, `1 - 2 - 3 - …`, all owned by `owner`.
    fn chain(n: usize, owner: u8) -> Vec<County> {
        let mut counties: Vec<County> = (0..=n).map(|_| County::new()).collect();
        for id in 1..=n {
            counties[id].owner = owner;
            counties[id].population = 100;
            if id > 1 {
                counties[id].add_neighbour(id as u8 - 1);
                counties[id - 1].add_neighbour(id as u8);
            }
        }
        counties
    }

    #[test]
    fn a_connected_realm_is_one_block_and_loses_nothing() {
        let counties = chain(5, 1);
        let blocks = build_blocks(&counties, 5);
        assert_eq!(blocks.count(), 1);
        assert_eq!(blocks.0[0].len(), 5);
        assert_eq!(blocks.0[0].population, 500, "the block's key is the population sum");
        let strength = [0u8, 3, 0, 0, 0, 0];
        let cut = minor_blocks(&blocks, &strength);
        assert_eq!(cut[0].blocks, 1);
        assert!(cut[0].counties.is_empty());
    }

    /// **The pass, as a sentence.** Take the county in the middle of a chain of
    /// five and the two beyond it are cut off; the realm keeps the larger half.
    #[test]
    fn cutting_the_bridge_county_costs_the_realm_the_far_half() {
        let mut counties = chain(5, 1);
        counties[3].owner = 2; // an enemy takes the middle
        let blocks = build_blocks(&counties, 5);
        assert_eq!(blocks.count(), 3, "1-2, 4-5, and the captured 3");
        let cut = minor_blocks(&blocks, &[0, 3, 3, 0, 0, 0]);
        let realm1 = cut.iter().find(|s| s.realm == 1).unwrap();
        assert_eq!(realm1.blocks, 2);
        // Both halves hold two counties of 100, so the tie goes to the *higher*
        // slot — the far half is kept and the near half secedes.
        assert_eq!(realm1.counties.len(), 2);
    }

    /// The tie-break, isolated: equal populations and the **highest** slot
    /// wins, because the comparison is `<=` and the scan runs upward.
    /// `docs/symbols.json` had this backwards.
    #[test]
    fn an_equal_population_hands_the_realm_the_higher_numbered_block() {
        let mut blocks = Blocks::empty();
        blocks.0[0] = Block { population: 400, owner: 1, members: { let mut m = [0; MAX_BLOCK_MEMBERS]; m[0] = 1; m } };
        blocks.0[1] = Block { population: 400, owner: 1, members: { let mut m = [0; MAX_BLOCK_MEMBERS]; m[0] = 2; m } };
        let cut = minor_blocks(&blocks, &[0, 3, 0, 0, 0, 0]);
        assert_eq!(cut[0].kept, 1, "the later of two equal blocks");
        assert_eq!(cut[0].counties, vec![1]);
    }

    /// …and a strictly larger block wins wherever it sits.
    #[test]
    fn the_most_populous_block_is_kept_not_the_one_with_most_counties() {
        let mut counties: Vec<County> = (0..=3).map(|_| County::new()).collect();
        for c in counties.iter_mut().take(4).skip(1) {
            c.owner = 1;
        }
        counties[1].population = 900; // alone
        counties[2].population = 100; // 2 - 3, adjacent
        counties[3].population = 100;
        counties[2].add_neighbour(3);
        counties[3].add_neighbour(2);
        let blocks = build_blocks(&counties, 3);
        assert_eq!(blocks.count(), 2);
        let cut = minor_blocks(&blocks, &[0, 3, 0, 0, 0, 0]);
        assert_eq!(cut[0].counties, vec![2, 3], "two counties lost to one bigger one");
    }

    /// A realm out of play is skipped entirely, and realm 0 is never considered
    /// — unowned counties are not in any block.
    #[test]
    fn an_eliminated_realm_and_the_neutral_counties_are_both_left_alone() {
        let mut counties = chain(4, 0);
        counties[1].owner = 2;
        counties[3].owner = 2; // realm 2 is split, but eliminated
        let blocks = build_blocks(&counties, 4);
        assert_eq!(blocks.count(), 2);
        assert!(minor_blocks(&blocks, &[0; MAX_REALMS]).is_empty(), "strength 0 everywhere");
        let cut = minor_blocks(&blocks, &[0, 0, 4, 0, 0, 0]);
        assert_eq!(cut.len(), 1);
        assert_eq!(cut[0].realm, 2);
        assert_eq!(cut[0].counties.len(), 1);
    }

    /// **The shape that makes the repeated sweep necessary.** County 2 is the
    /// bridge and it is discovered *last*, so a single sweep would leave 1 and
    /// 3 in two blocks for ever. The original sweeps until nothing changes, and
    /// so does this.
    #[test]
    fn a_bridge_discovered_last_still_ends_as_one_block() {
        let mut counties: Vec<County> = (0..=3).map(|_| County::new()).collect();
        for c in counties.iter_mut().take(4).skip(1) {
            c.owner = 1;
            c.population = 10;
        }
        // 1 - 2 - 3, and nothing joins 1 to 3.
        counties[1].add_neighbour(2);
        counties[2].add_neighbour(1);
        counties[2].add_neighbour(3);
        counties[3].add_neighbour(2);
        let blocks = build_blocks(&counties, 3);
        assert_eq!(blocks.count(), 1, "one block, whatever order they were offered in");
        assert_eq!(blocks.0[0].len(), 3);
    }

    /// The partition is over the **neighbour list**, so two counties whose ids
    /// are adjacent and whose lists are not are two blocks.
    #[test]
    fn contiguity_is_the_neighbour_list_and_nothing_else() {
        let mut counties: Vec<County> = (0..=2).map(|_| County::new()).collect();
        counties[1].owner = 1;
        counties[2].owner = 1;
        let blocks = build_blocks(&counties, 2);
        assert_eq!(blocks.count(), 2, "no neighbour entry, no contiguity");
    }

    /// Every county lands in exactly one block, and no block mixes owners.
    #[test]
    fn the_partition_is_a_partition() {
        let mut counties = chain(10, 1);
        for id in [4usize, 7] {
            counties[id].owner = 2;
        }
        counties[9].owner = 0;
        let blocks = build_blocks(&counties, 10);
        let mut seen: Vec<u8> = Vec::new();
        for block in blocks.0.iter().filter(|b| b.owner != 0) {
            for id in block.members() {
                assert_eq!(counties[id as usize].owner, block.owner, "no mixed block");
                assert!(!seen.contains(&id), "county {id} is in two blocks");
                seen.push(id);
            }
        }
        seen.sort_unstable();
        let owned: Vec<u8> =
            (1..=10u8).filter(|&id| counties[id as usize].owner != 0).collect();
        assert_eq!(seen, owned, "every owned county, exactly once");
    }
}
