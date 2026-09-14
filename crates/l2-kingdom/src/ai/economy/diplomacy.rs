#![allow(unused_imports)]
use super::*;
use super::steps::*;
use super::taxes::*;
use super::grants::*;
use super::construction::*;
use super::industry::*;
use super::*;
use crate::county::County;
use crate::realm::{Realm, AI_STEP_DONE};
use crate::tables::{
    ai_grant_tier, tax_rate_for, Tables, AI_CASTLE_LADDER_LEN, AI_GOLD_GRANT_SMALL_COUNTIES,
    AI_WEAPON_ROTA_ORDER,
};

/// Step 13 — `AI_Taunt` (`0x004A13A6`).
///
/// **`docs/kingdom.md` §3.2 has this step as *"offer or break an alliance"*. It
/// is not.** Alliances are step 2's business; step 13 is a two-stage gloat, and
/// the only thing it changes about the game state is a timer, a stage byte and
/// the voice rotation.
///
/// * Only a realm ranked **first** taunts at all (`rank < 2`).
/// * **Stage 0** — above 39% of the map, count to 8, then send *"How are you
///   doing?"* (`L2.eng` group 193) to **every** live human realm, reset the
///   timer and go to stage 1.
/// * **Stage 1** — above 27% of the map, and only if the realm in **last
///   place** is human and is not this realm's ally, count to 8, then send
///   *"Helpful advice."* (group 192) to that realm and go back to stage 0.
///
/// The two thresholds are `>` on 0x27 and 0x1B
/// timer`, so it is the **ninth** consecutive qualifying turn that sends. The
/// timer only advances on a turn the share threshold is met
/// slips below 39% pauses.
///
/// `trailer` is the last-placed realm — `g_rankTrailer`, which the original
/// keeps as a global and which is derived by the caller from
/// [`rank_realms`]. `out` collects the letters; the caller decides what to do
/// with them
pub fn taunt(realm: &mut Realm, realm_id: u8, realms_snapshot: &[Realm], trailer: u8) -> Vec<Taunt> {
    let mut sent = Vec::new();
    if realm.rank >= 2 {
        return sent;
    }
    if realm.taunt_stage == 0 {
        if realm.share_of_map_pct <= 39 {
            return sent;
        }
        realm.taunt_timer = realm.taunt_timer.saturating_add(1);
        if realm.taunt_timer <= 7 {
            return sent;
        }
        for (id, other) in realms_snapshot.iter().enumerate().take(6).skip(1) {
            if id as u8 != realm_id && other.in_play && other.is_human {
                sent.push(Taunt {
                    from: realm_id,
                    to: id as u8,
                    group: TAUNT_HOW_ARE_YOU_DOING,
                    variant: realm.message_variant(),
                });
                realm.advance_voice();
            }
        }
        realm.taunt_timer = 0;
        realm.taunt_stage = 1;
        return sent;
    }
    let Some(target) = realms_snapshot.get(trailer as usize) else { return sent };
    if realm.share_of_map_pct <= 27 || !target.is_human || realm.ally == trailer {
        return sent;
    }
    realm.taunt_timer = realm.taunt_timer.saturating_add(1);
    if realm.taunt_timer <= 7 {
        return sent;
    }
    realm.taunt_timer = 0;
    realm.taunt_stage = 0;
    sent.push(Taunt {
        from: realm_id,
        to: trailer,
        group: TAUNT_HELPFUL_ADVICE,
        variant: realm.message_variant(),
    });
    realm.advance_voice();
    sent
}


