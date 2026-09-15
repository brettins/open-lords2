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
/// | caller | when |
/// |---|---|
/// | `AI_RunTurnStep` step 0 | the top of **every** realm's turn, human included |
/// | `County_ChangeOwner` (`0x004A72FE`) | a county changes hands — **for the realm that is losing it** |
/// | `Battle_Resolve` (`0x004A4E..`), twice | after the loser's army is destroyed |
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

