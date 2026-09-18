#![allow(unused_imports)]
use super::*;
use super::steps::*;
use super::taxes::*;
use super::grants::*;
use super::construction::*;
use super::diplomacy::*;
use super::*;
use crate::county::County;
use crate::realm::{Realm, AI_STEP_DONE};
use crate::tables::{
    ai_grant_tier, tax_rate_for, Tables, AI_CASTLE_LADDER_LEN, AI_GOLD_GRANT_SMALL_COUNTIES,
    AI_WEAPON_ROTA_ORDER,
};

/// Step 12 — `FUN_0049E77D`, which this crate names `AI_ChooseIndustry`.
///
/// `docs/kingdom.md` calls county `+0x1B0` untraced; this loop sets it to **1
/// on every county the realm holds, unconditionally**, and
/// `AI_ManageFields(0)` sets it to 0 on the unowned ones. That is
/// [`County::castle_switch`]
/// county's castle-building job slot is live*.
///
/// **The departure this used to record is closed.** It said the original reads
/// county `+0x1D4`/`+0x1D0` — the wood and stone a build still owes — and that
/// this crate had no such counter because it debited the whole cost up front.
pub fn choose_industry(
    t: &Tables,
    counties: &mut [County],
    county_count: usize,
    realm: &mut Realm,
    realm_id: u8,
) {
    let Some(p) = t.ai_personality(realm.lord) else { return };
    let rota = p.weapon_rota;
    for id in 1..=county_count.min(counties.len() - 1) {
        if counties[id].owner != realm_id {
            continue;
        }
        let cursor = realm.weapon_rota.clamp(0, AI_WEAPON_ROTA_ORDER.len() as i32 - 1) as usize;
        counties[id].weapon_type = rota[AI_WEAPON_ROTA_ORDER[cursor]];
        realm.weapon_rota += 1;
        if realm.weapon_rota > 9 {
            realm.weapon_rota = 0;
        }
        // `FUN_0049ED13`: the blacksmith's "has resource" is not geology, it is
        // **affordability** — can the realm pay this county's chosen weapon out
        // of its wood and iron?
        let row = t.weapon[counties[id].weapon_type.min(t.weapon.len() - 1)];
        counties[id].industry[crate::tables::Commodity::Weapons as usize].has_resource =
            row.wood <= realm.wood && row.iron <= realm.iron;
    }
    for id in 1..=county_count.min(counties.len() - 1) {
        if counties[id].owner != realm_id {
            continue;
        }
        let building = counties[id].castle_degraded != 0;
        for slot in 0..counties[id].industry.len() {
            let record = &counties[id].industry[slot];
            let free = record.has_resource && record.disabled_seasons == 0;
            counties[id].industry[slot].enabled =
                free && (!building || castle_allows(slot, &counties[id]));
        }
        counties[id].castle_switch = true;
    }
}

