#![allow(unused_imports)]
use super::*;
use super::partition::*;
use super::tests_part::*;
use crate::county::County;
use crate::realm::MAX_REALMS;

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
/// never in a block,
///
/// **The human is not treated differently.** The only branch on
/// `g_localPlayer` in the whole pass is the message: `0x7F` when exactly one
/// county went and `0x80` when more, sent only to the local player and only
/// when the realm held more than one block. **An AI loses its outlying counties
/// in silence** —
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
            // incumbent,
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

