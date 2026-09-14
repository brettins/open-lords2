#![allow(unused_imports)]
use super::*;
use super::placement::*;
use super::lords::*;
use super::scenario::*;
use l2_formats::maps::{MapSlot, Plane, PLANE_DIM};
use l2_kingdom::county::{MAX_COUNTIES, MAX_COUNTY_ID, MAX_FIELDS, MAX_NEIGHBOURS};
use l2_kingdom::map::{CampaignMap, MAP_TILES};
use l2_kingdom::merchant::{self, MerchantRoutes, ROUTES, ROUTE_SLOTS};
use l2_kingdom::mercenary::MercenaryBands;
use l2_kingdom::realm::MAX_REALMS;
use l2_kingdom::tables::{Weather, JOB_COUNT};
use l2_kingdom::unit::{Unit, UnitKind, Units};
use l2_kingdom::Options;
use crate::{Clock, CountyState, IndustryState, RealmState, Scenario};

];

/// `County_Reset`'s opening numbers. **`[D]`** — `FUN_00451150`, straight down.
mod reset {
    pub const POPULATION: i32 = 150;
    pub const HEALTH_METER: i32 = 50;
    pub const HEALTH_BAND: u8 = 2;
    pub const HAPPINESS: i32 = 50;
    pub const RATION_WANTED: i32 = 3;
    pub const RATION_SPLIT: i32 = 100;
    pub const DRYNESS: i32 = 50;
    pub const GRAIN: i32 = 100;
    pub const HERD: i32 = 40;
    pub const INDUSTRY_SHARE: i32 = 25;
    /// `Labour_DefaultShares` (`0x004514F8`): farm 33/50/17 and the whole
    /// industry share on wood. Both halves sum to 100, which is the invariant
    /// that identifies the array.
    pub const LABOUR_SHARE: [i32; 8] = [33, 50, 17, 0, 0, 0, 100, 0];
    /// The weather every county opens on. `g_weatherCounty` opens on 1.
    pub const WEATHER: u8 = 3;
}

