#![allow(unused_imports)]
use super::*;
use super::tables::*;
use super::*;

pub const AI_CASTLE_LADDER_LEN: usize = 5;

#[inline]
pub fn ai_tax_ladder(lord: u8) -> Option<&'static TaxLadder> {
    let index = (lord as usize).checked_sub(1)?;
    let row = *AI_PERSONALITY_TAX_LADDER.get(index)?;
    AI_TAX_LADDERS.get(row)
}

/// `AI_ManageFields` (`FUN_0049DD01`, AI step 5) adds a field to a county whose
/// field total is below the first threshold its population clears.
///
/// `(fields_below, population_above, fields_added)`, walked in order; the last
/// row has no field cap at all. `[V]`
pub const AI_FIELD_LADDER: [(i32, i32, i32); 6] = [
    (1, -1, 1),        // no fields at all: always add one
    (3, 200, 1),
    (5, 400, 1),
    (7, 600, 1),
    (9, 1000, 1),
    (i32::MAX, 1200, 2), // a big county adds two at once
];


