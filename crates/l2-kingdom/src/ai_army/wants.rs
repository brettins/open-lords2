#![allow(unused_imports)]
use super::*;
use super::muster::*;
use super::raid::*;
use super::aim::*;
use super::army::*;
use crate::county::{County, MAX_COUNTIES};
use crate::kingdom::Kingdom;
use crate::levy::{self, LevyBasket, Muster};
use crate::map::{flags, CampaignMap, MAP_DIM};
use crate::math::pct;
use crate::realm::{Realm, MAX_REALMS};
use crate::tables::Tables;
use crate::unit::{TroopType, UnitKind, Units};



/// `FUN_0049D5E0` — the realm's **lowest-numbered** county, or 0.
pub fn first_owned_county(counties: &[County; MAX_COUNTIES], county_count: usize, realm: u8) -> u8 {
    (1..=county_count.min(MAX_COUNTIES - 1))
        .find(|&id| counties[id].owner == realm)
        .unwrap_or(0) as u8
}

/// Step 4 — `FUN_0049E1BF`, which fills realm `+0x70 … +0x7C`.
///
/// ```c
/// for (i = 0; i < 4; i++) realm.want[i] = 0;
/// if (realm.countyCount > 1
///     || ((c = firstOwnedCounty(realm)) > 0 && c.happiness > 14 && c.healthMeter > 19)) {
///     if (realm.iron < 50)  realm.want[iron] = 50;
///     if (realm.wood < 100) realm.want[wood] = 100;
///     for every owned county with a castle and work in progress:
///         realm.want[wood]  += county[+0x1D4];   /* wood the work still needs */
///         realm.want[stone] += county[+0x1D0];   /* stone */
/// }
/// ```
///
/// [`crate::industry::order_castle`] debits the whole cost up front, exactly
/// as [`crate::ai::choose_industry`] records for the same pair of fields. So
/// the term evaluates to zero here and the wants are the two floors. `[I]`, and
/// it is the up-front debit that makes it so
/// the binary.
///
/// The consumer is `Ai_TradeForCounty` (`0x0049E39B`), which
/// [`crate::ai_farm`] records as unimplemented — so this step changes no
/// behaviour today. It is here because it is one of the fourteen and because
/// the *gate* is a rule.
pub fn resource_wants(
    t: &Tables,
    counties: &[County; MAX_COUNTIES],
    county_count: usize,
    realms: &mut [Realm; MAX_REALMS],
    realm_id: u8,
) {
    let _ = t;
    let Some(realm) = realms.get_mut(realm_id as usize) else { return };
    realm.want = [0; 4];
    if realm.county_count <= 1 {
        let first = first_owned_county(counties, county_count, realm_id);
        if first == 0 {
            return;
        }
        let c = &counties[first as usize];
        if c.happiness <= WANT_MIN_HAPPINESS || c.health_meter <= WANT_MIN_HEALTH {
            return;
        }
    }
    if realm.iron < WANT_IRON_FLOOR {
        realm.want[WANT_IRON] = WANT_IRON_FLOOR;
    }
    if realm.wood < WANT_WOOD_FLOOR {
        realm.want[WANT_WOOD] = WANT_WOOD_FLOOR;
    }
}

/// Realm `+0x70` — wood.
pub const WANT_WOOD: usize = 0;
/// Realm `+0x74` — iron.
pub const WANT_IRON: usize = 1;
/// Realm `+0x78` — **zeroed every turn and never written.**
pub const WANT_UNUSED: usize = 2;
/// Realm `+0x7C` — stone.
pub const WANT_STONE: usize = 3;
pub const WANT_IRON_FLOOR: i32 = 50;
pub const WANT_WOOD_FLOOR: i32 = 100;
pub const WANT_MIN_HAPPINESS: i32 = 14;
pub const WANT_MIN_HEALTH: i32 = 19;

