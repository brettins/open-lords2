#![allow(unused_imports)]
use super::*;
use super::factors::*;
use super::steps::*;
use super::actions::*;
use super::bands::*;
use super::*;

/// **`Grain_SeasonTick`'s last two lines (`0x0044C8AE`), which had no
/// counterpart at all.**
///
/// ```c
/// band = FUN_0044CF6F(<this season's crop word>, (byte)county.field_0x206);
/// variant = band < 3 ? 0 : (band - 3) / 4 + 1;
/// FUN_00469D21(county, band, variant, 2, 0xE);
/// ```
///
/// `FUN_00469D21(county, terrain, variant, lo, hi)` sweeps the whole tile array
/// in index order and calls `Terrain_Set(tile, terrain, variant)` on every tile
/// of that county carrying plane-0 bit `0x20` whose current `content` is in
/// `lo ..= hi`. So the repaint is bounded to tiles that are *already* growing
/// grain — a fallow or a pasture tile in the same county is untouched — and the
/// crop's stage is written onto the map.
///
/// `sowShortfall` is `Grain_Sow`'s *"could not afford one sack a field and fell
/// back to a token handful"* flag. When it is set, **the first grain tile in
/// index order takes the whole crop's appearance and every other one is
/// repainted bare.** That is the game showing a failed sowing as one green field
/// among the empty ones, and it is a picture that is a rule — the same shape as
/// the pasture herd and the mine's animation rate. `[D]`
pub fn grain_repaint_fields(
    county_id: usize,
    county: &County,
    season: Season,
    map: &mut crate::map::CampaignMap,
) {
    let band = grain_stage_band(county, season);
    let mut seen = false;
    for tile in 0..map.terrain.len() {
        if map.county[tile] as usize != county_id {
            continue;
        }
        if map.flags[tile] & crate::map::flags::FARMLAND == 0 {
            continue;
        }
        if !(2..=0x0E).contains(&map.terrain[tile]) {
            continue;
        }
        let painted = if county.sow_shortfall && seen { 2 } else { band };
        crate::field::paint_tile(map, tile, painted);
        seen = true;
    }
}

/// **`Grain_LabourEstimate`'s tail (`0x0044D374`) — the grain row's forecast.**
///
/// A player: *"Sidebar doesn't show grain being planted as a negative number."*
/// This is the number that would have said so, and until this function existed
/// nothing in the workspace computed it. `docs/decisions.md` C123
/// has the whole of it; the short version is that
/// [`grain_labour_estimate`] ports the **search loop** of `Grain_LabourEstimate`
/// and the original writes four more things after the loop ends:
///
/// * **`crop[2]` and `+0x2FC` are written only in their own seasons** and keep
/// their previous value otherwise.
///
/// * **`+0x22C` is zeroed before the `popBand` guard**, so an empty county
///   forecasts nothing. The same shape
/// as [`herd_preview`], and the same reason.
///
/// `[D]`, read out of `0x0044D374` and matched against the row that draws it.
pub fn grain_preview(t: &Tables, county: &mut County, season_next: Season, advanced_farming: bool) {
    county.grain_change_expected = 0;
    if county.pop_band == 0 {
        return;
    }
    let staff = county.labour[crate::tables::JOB_GRAIN_FARMING];
    county.grain_sown_expected =
        sow_seed(t, county.fields_grain, county.grain, staff, advanced_farming).0;
    match season_next {
        Season::Winter => {
            county.crop[2] = harvest_step(t, county, staff, county.crop[1], advanced_farming);
        }
        Season::Summer | Season::Autumn => {
            county.grain_grown_expected =
                grow_step(t, county, staff, county.crop[1], advanced_farming);
        }
        Season::Spring => {}
    }
    county.grain_change_expected = match season_next {
        Season::Spring => -county.grain_sown_expected - county.grain_eaten,
        Season::Winter => county.crop[2] - county.grain_eaten,
        _ => -county.grain_eaten,
    };
}

/// `Grain_LabourEstimate` (`0x0044D374`) — the **grain** ceiling, for the
/// season the sowing happens in.
///
/// Because `Grain_Sow` is monotone in labour and the test is strict, the answer
/// *is* "the fewest farmers that reach the best yield" — but by scanning, and
/// the loop stops one short of the population, so a county can never want every
/// one of its people on the fields. **`[D]`**, and reproduced by search here
/// for the same reason the original does: the
/// integer truncation inside `Grain_Sow` is part of the answer.
///
///   `Labour_Allocate` reads only the ceiling — **verified by exhaustion over
///   its 2,147 bytes: it reads `+0xCC + slot*0x0C` eight times and
///   `+0xC8 + slot*0x0C` not once** — so the floor is a display value and
///   nothing in the simulation depends on it.
pub fn grain_labour_estimate(
    t: &Tables,
    county: &County,
    season_next: Season,
    advanced_farming: bool,
) -> Option<GrainEstimate> {
    if county.pop_band == 0 {
        return None;
    }
    let store = county.grain - county.grain_eaten;
    let mut best = 1;
    let mut estimate = GrainEstimate { wanted: crate::county::LABOUR_NO_FLOOR, useful: 0 };
    for workers in 0..county.population {
        let got = match season_next {
            Season::Spring => sow_seed(t, county.fields_grain, store, workers, advanced_farming).0,
            Season::Winter => {
                harvest_step(t, county, workers, county.crop[1], advanced_farming)
            }
            _ => grow_step(t, county, workers, county.crop[1], advanced_farming),
        };
        if best < got {
            estimate = GrainEstimate { wanted: workers, useful: workers };
            best = got;
        }
    }
    Some(estimate)
}


