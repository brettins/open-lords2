#![allow(unused_imports)]
use super::*;
use super::ranking::*;
use super::outcome::*;
use super::tests_part::*;
use crate::county::{County, MAX_COUNTIES};
use crate::realm::{Realm, MAX_REALMS};
use crate::tables::Tables;
use crate::unit::{UnitKind, Units};
use l2_net::{Quirk, Quirks};

/// `3 * counties + 1 * armies` — realm `+0x04`, rebuilt from scratch.
///
/// **Armies only.** `FUN_0049B42B` counts `g_units` slots 1..=150 whose type byte
/// is 1; merchants, transports and peasant mobs are in the same array and do not
/// count (`docs/armies.md`). The result cannot overflow the byte it lands in:
/// sixteen counties and a hundred and fifty armies is 198.
pub fn strength(
    counties: &[County; MAX_COUNTIES],
    county_count: usize,
    units: &Units,
    realm: u8,
) -> u8 {
    let mut s: u8 = 0;
    for id in 1..=county_count.min(MAX_COUNTIES - 1) {
        if counties[id].owner == realm {
            s = s.wrapping_add(3);
        }
    }
    for (_, u) in units.iter() {
        if u.owner == realm && u.kind == UnitKind::Army {
            s = s.wrapping_add(1);
        }
    }
    s
}

/// `FUN_0049B42B` — recount one realm's strength and notice if it has just died.
///
/// Called from four places in the original, and the four are worth having in one
/// list because they are the whole of "when can a realm be eliminated":
///
/// | caller | when |
/// |---|---|
/// | `AI_RunTurnStep` step 0 | the top of **every** realm's turn, human included |
/// | `County_ChangeOwner` (`0x004A72FE`) | a county changes hands — **for the realm that is losing it** |
/// | `Battle_Resolve` (`0x004A4E..`), twice | after the loser's army is destroyed |
///
/// **The `County_ChangeOwner` call cannot eliminate anybody**, and that is a
/// quirk of the original
/// *before* `g_counties[c].owner = newOwner` runs, so the county being lost is
/// still counted. A realm losing its last county therefore survives until its own
/// step 0 comes round. Reproduced by [`crate::conquest::change_owner`]'s caller
/// doing the same thing in the same order.
///
/// # The guard
///
/// The whole body is skipped for a realm whose strength is **already** zero, so
/// an eliminated realm is never re-eliminated and never sends its message twice.
///
/// Returns the message the original enqueues, if any. The caller must run
/// [`rank_and_crown`] afterwards — the original calls `Score_RankRealms` at the
/// bottom of this function, and it is split out only so the two halves can be
/// tested apart.
pub fn recount_strength(
    realms: &mut [Realm; MAX_REALMS],
    counties: &[County; MAX_COUNTIES],
    county_count: usize,
    units: &Units,
    realm: u8,
    local_player: u8,
) -> Option<Ending> {
    let id = realm as usize;
    if id == 0 || id >= MAX_REALMS || realms[id].strength == 0 {
        return None;
    }
    let s = strength(counties, county_count, units, realm);
    realms[id].strength = s;
    realms[id].in_play = s != 0;
    if s != 0 {
        return None;
    }

    // The three-way split is on **the local player**, not on `is_human`. A
    // second human in a network game falls through both arms and is told
    // nothing, which is a real difference and not an oversight here.
    let msg = if local_player == realm {
        // `Msg_Enqueue(g_localPlayer, g_localPlayer, 0xE0, 0, 0x0E, …)` —
        // **variant 0**, so your own defeat is always group 224's first string.
        Some(Ending {
            group: MSG_DEFEAT,
            from: local_player,
            to: local_player,
            category: CATEGORY_ENDING,
            variant: 0,
        })
    } else if !realms[id].is_human {
        // `Msg_Enqueue(realm, 0, 0xC2, (rotation - 4) + lord * 4, 0x0E, …)`.
        Some(Ending {
            group: MSG_AI_ELIMINATED,
            from: realm,
            to: 0,
            category: CATEGORY_ENDING,
            variant: voice_variant(&realms[id]),
        })
    } else {
        None
    };
    // Advanced in all three cases, the silent one included.
    advance_voice(&mut realms[id]);
    msg
}

/// Realm `+0x159`, wrapping at 4. `docs/diplomacy.md` §0.
pub(super) fn advance_voice(realm: &mut Realm) {
    realm.voice_rotation += 1;
    if realm.voice_rotation > 3 {
        realm.voice_rotation = 0;
    }
}

