#![allow(unused_imports)]
use super::*;
use super::secession::*;
use super::tests_part::*;
use crate::county::County;
use crate::realm::MAX_REALMS;

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

