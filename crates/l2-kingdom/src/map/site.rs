#![allow(unused_imports)]
use super::*;
use super::castle::*;
use super::campaign::*;
use super::cost::*;
use crate::tables::{MOVE_COST_BLOCKED, MOVE_COST_IMPASSABLE};

/// The commodity and state a settlement terrain byte names, or `None` when the
/// tile is a town, a castle plot or plain ground.
///
/// The inverse of [`terrain::INDUSTRY_IDLE`]
/// [`crate::industry::map_toggle_for_graphic`] reads from the click side — kept
/// as two functions because the click ladder folds terrain 0 into iron and
/// everything from 21 up into the castle, which are `Map_Click`'s concerns and
/// not a site's.
pub fn industry_state(terrain: u8) -> Option<(crate::tables::Commodity, SiteState)> {
    if terrain == 0 || terrain > 12 {
        return None;
    }
    let state = match (terrain - 1) % 3 {
        0 => SiteState::Idle,
        1 => SiteState::Working,
        _ => SiteState::Wrecked,
    };
    let base = terrain - (terrain - 1) % 3;
    let commodity = crate::tables::Commodity::ALL
        .into_iter()
        .find(|c| terrain::INDUSTRY_IDLE[c.index()] == base)?;
    Some((commodity, state))
}

/// **Where one county's industry sits.** The original keeps the answer on the
/// record — `Industry.siteTile`, county `+0x298 + c*0x18`, a byte offset into
/// `g_tiles` that `County_PlaceResourceSites` stores at load — and this derives
/// it instead
/// terrain ladder.
///
/// Derived. A cached tile index is a field an
/// importer has to fill and can silently fail to — which is
/// `docs/decisions.md` C30's whole shape, and this project has already paid for
/// it twice. The scan is over the county's own tiles in index order, so the
/// answer is deterministic (`docs/netcode.md` §3).
pub fn industry_site(
    map: &CampaignMap,
    county: u8,
    commodity: crate::tables::Commodity,
) -> Option<usize> {
    (0..map.terrain.len()).find(|&tile| {
        map.county[tile] == county
            && map.flags[tile] & flags::SETTLEMENT != 0
            && industry_state(map.terrain[tile]).is_some_and(|(c, _)| c == commodity)
    })
}

