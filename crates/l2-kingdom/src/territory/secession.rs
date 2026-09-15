#![allow(unused_imports)]
use super::*;
use super::partition::*;
use super::tests_part::*;
use crate::county::County;
use crate::realm::MAX_REALMS;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Secession {
    pub realm: u8,
    pub blocks: usize,
    pub kept: usize,
    pub counties: Vec<u8>,
}

/// `Territory_SecedeMinorBlocks` (`0x0044AE51`), as a decision — which counties
/// each realm loses, without touching them.
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

