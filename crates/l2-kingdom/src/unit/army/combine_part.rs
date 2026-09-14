#![allow(unused_imports)]
use super::*;
use super::unit_impl::*;
use super::units_impl::*;
use super::wages::*;
use super::starvation::*;
use super::destroy_part::*;
use super::*;
use crate::county::{County, MAX_COUNTIES};
use crate::realm::{Realm, MAX_REALMS};
use crate::tables::Tables;

/// Why [`combine`] refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CombineRefusal {
    /// Together they would exceed [`crate::tables::ARMY_MAX_MEN`]. The original
    /// says nothing at all in this case — no message is raised.
    TooMany,
    /// Both carry a mercenary band. `L2.eng` **167**: *"Cannot combine armies.
    /// The mercenaries in these armies will not fight together."*
    TwoMercenaryBands,
    /// One of the slots is empty, or is not an army.
    NotAnArmy,
}

/// `Army_Combine` (`0x004AA181`) — merge `from` into `into` and destroy `from`.
///
/// ```text
/// if (men[into] + men[from] < 0x5DD)            /* 1501: at most 1500 merged */
///     if (merc[into] == 0 || merc[from] == 0)   /* two bands refuse */
///         ...merge...
///     else message 0xA7                          /* group 167 */
/// ```
///
/// **`[V]` 1500 is exact**, and it is the player's *"maximum army size is about
/// 1500"*. What the merge takes:
///
/// * `men` and the seven troop counts are summed;
/// * `movesUsed` takes **the higher of the two**
///   into a spent one is spent;
/// * the band, if only one side has it, moves across whole;
/// * the siege links move across
///
/// Returns the men in the merged army, or why it refused. The caller destroys
/// `from`: this function only moves what is on the two records, because
/// `Army_Destroy` needs the realm array and this does not.
pub fn combine(units: &mut Units, into: usize, from: usize) -> Result<i32, CombineRefusal> {
    let (a, b) = match (units.get(into), units.get(from)) {
        (Some(a), Some(b)) if a.kind == UnitKind::Army && b.kind == UnitKind::Army => (a, b),
        _ => return Err(CombineRefusal::NotAnArmy),
    };
    if a.men + b.men > crate::tables::ARMY_MAX_MEN {
        return Err(CombineRefusal::TooMany);
    }
    if a.mercenaries.is_some() && b.mercenaries.is_some() {
        return Err(CombineRefusal::TwoMercenaryBands);
    }
    let absorbed = units.remove(from).expect("checked just above");
    let into_unit = units.get_mut(into).expect("checked just above");
    // **The higher, not the lower.** `docs/armies.md` §2.7 says the merge
    // "takes the *lower* of the two `movesUsed`", which reads as a refund and
    // is the opposite of what the code does:
    //
    // ```c
    // if ((char)movesUsed[from] < (char)movesUsed[into]) v = movesUsed[into];
    // else                                               v = movesUsed[from];
    // movesUsed[into] = v;
    // ```
    //
    // Both arms select the maximum. Merging a fresh army into a spent one
    // leaves the result spent
    // which is a real tactical rule
    // Corrected in the document. `[D]`
    into_unit.moves_used = into_unit.moves_used.max(absorbed.moves_used);
    if into_unit.besieging_county == 0 {
        into_unit.besieging_county = absorbed.besieging_county;
    }
    if into_unit.besieged_by == 0 {
        into_unit.besieged_by = absorbed.besieged_by;
    }
    if into_unit.mercenaries.is_none() {
        into_unit.mercenaries = absorbed.mercenaries;
    }
    into_unit.men += absorbed.men;
    for t in 0..TROOP_TYPES {
        into_unit.troops[t] += absorbed.troops[t];
    }
    Ok(into_unit.men)
}

