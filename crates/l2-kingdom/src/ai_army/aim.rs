#![allow(unused_imports)]
use super::*;
use super::wants::*;
use super::muster::*;
use super::raid::*;
use super::army::*;
use crate::county::{County, MAX_COUNTIES};
use crate::kingdom::Kingdom;
use crate::levy::{self, LevyBasket, Muster};
use crate::map::{flags, CampaignMap, MAP_DIM};
use crate::math::pct;
use crate::realm::{Realm, MAX_REALMS};
use crate::tables::Tables;
use crate::unit::{TroopType, UnitKind, Units};


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aim {
    /// `FUN_004A6735` — the **county town**: plane-0 `0x40`. Stepping onto it
    /// is `Army_AttackCounty`, so this is how a county is taken.
    Town,
    /// `FUN_004A65A3` — the **castle**: plane-0 `0x80` with terrain `0x15 …
    /// 0x19`, which is the five built castle types and **not** the empty
    /// `0x14` plot. Stepping onto it garrisons the army when the county is the
    /// realm's own and lays a siege when it is not.
    Castle,
    /// `FUN_004A689D` — the nearest **standing crop**: plane-0 `0x20` with
    /// terrain `3 … 22`, which is exactly
    /// [`crate::map::terrain::FIELD_STANDING_FROM`] `+1` up to
    /// [`crate::map::terrain::FIELD_STANDING_TO`] `−1`. Every field an army
    /// crosses in a county it does not own is trampled, so **a raid is aimed
    /// at the harvest** and does its damage on the way in.
    StandingCrop,
}

impl Aim {
    fn accepts(self, terrain: u8, tile_flags: u8) -> bool {
        match self {
            Aim::Town => tile_flags & flags::CASTLE != 0,
            Aim::Castle => tile_flags & flags::SETTLEMENT != 0 && (0x15..=0x19).contains(&terrain),
            Aim::StandingCrop => {
                tile_flags & flags::FARMLAND != 0 && terrain > 2 && terrain < 0x17
            }
        }
    }
}

/// The fallback differs from the original in one place and it is stated: for
/// [`Aim::Castle`] the original falls back to the county's stored castle tile
/// (`+0x74`/`+0x75`, `County_FindCastleTile`'s output), which this crate does
/// not carry. The anchor is used instead. `[I]`, and it only fires when the
/// county has no castle tile on the map at all — in which case the stored pair
/// would be stale.
pub fn aim_tile(
    map: &CampaignMap,
    counties: &[County; MAX_COUNTIES],
    from: (u8, u8),
    county: u8,
    aim: Aim,
) -> (u8, u8) {
    let mut best = 1000;
    let mut found: Option<(u8, u8)> = None;
    for y in 0..MAP_DIM as u8 {
        for x in 0..MAP_DIM as u8 {
            if x == 0 && y == 0 {
                continue; // the offset-0 sentinel
            }
            if map.county_at(x, y) != county {
                continue;
            }
            if !aim.accepts(map.terrain_at(x, y), map.flags_at(x, y)) {
                continue;
            }
            let d = manhattan(from, (x, y));
            if d < best {
                best = d;
                found = Some((x, y));
            }
        }
    }
    found.unwrap_or_else(|| {
        counties
            .get(county as usize)
            .map_or((0, 0), |c| (c.anchor_x, c.anchor_y))
    })
}

/// `FUN_004A64CA` — the choice missions 2 and 3 make between the town and the
/// castle.
///
/// > *"This shire contains a garrisoned castle my lord. We must lay siege to
/// > that, to gain control of the county."* — `L2.eng` group 286, which is the
/// > game telling the **player** the rule this function is the AI's half of.
///
/// > `[V]`
pub fn aim_for_county(counties: &[County; MAX_COUNTIES], county: u8) -> Aim {
    match counties.get(county as usize) {
        Some(c) if c.castle_type != 0 && c.garrison_unit != 0 => Aim::Castle,
        _ => Aim::Town,
    }
}

