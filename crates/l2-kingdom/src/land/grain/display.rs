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
/// A player: *"The wheat fields don't show the wheat growing."* This is half the
/// answer — the other half is [`l2_view::campaign::field_variant`], and
/// **neither half alone changes a pixel**, so the bug survived a
/// season pass this project believes it has read.
///
/// ```c
/// band = FUN_0044CF6F(<this season's crop word>, (byte)county.field_0x206);
/// variant = band < 3 ? 0 : (band - 3) / 4 + 1;
/// FUN_00469D21(county, band, variant, 2, 0xE);
/// ```
///
/// **The first argument changes with the season** — [`grain_stage_band`].
///
/// `FUN_00469D21(county, terrain, variant, lo, hi)` sweeps the whole tile array
/// in index order and calls `Terrain_Set(tile, terrain, variant)` on every tile
/// of that county carrying plane-0 bit `0x20` whose current `content` is in
/// `lo ..= hi`. So the repaint is bounded to tiles that are *already* growing
/// grain — a fallow or a pasture tile in the same county is untouched — and the
/// crop's stage is written onto the map.
///
/// **The one wrinkle, reproduced:** its shortfall arm.
///
/// ```c
/// if (lo == 2 && county.sowShortfall && notTheFirstTileThisSweep)
///     Terrain_Set(tile, 2, '\0');       /* this one gets nothing */
/// else
///     Terrain_Set(tile, terrain, variant);
/// ```
///
/// `sowShortfall` is `Grain_Sow`'s *"could not afford one sack a field and fell
/// back to a token handful"* flag. When it is set, **the first grain tile in
/// index order takes the whole crop's appearance and every other one is
/// repainted bare.** That is the game showing a failed sowing as one green field
/// among the empty ones, and it is a picture that is a rule — the same shape as
/// the pasture herd and the mine's animation rate. `[D]`
///
/// `season` is `g_season`, the season whose clause just ran, because that is
/// what chose the crop word the band is read from.
pub fn grain_repaint_fields(
    county_id: usize,
    county: &County,
    season: Season,
    map: &mut crate::map::CampaignMap,
) {
    // The variant `band < 3 ? 0 : (band - 3) / 4 + 1` is not stored: it is
    // recoverable from the band, which is the terrain byte, and
    // `l2_view::campaign::field_variant` recovers it.
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
/// ```c
/// staff = county.labour[0].workers;
/// county.field_0x230 = Grain_Sow(county, staff, county.grain);
/// if (season == 4)                county.crop[2]     = Grain_Harvest(county, staff, crop[1]);
/// if (season == 2 || season == 3) county.field_0x2FC = Grain_Grow   (county, staff, crop[1]);
///
/// if      (season == 1) county.field_0x22C = -county.field_0x230 - county.grainEaten;
/// else if (season == 4) county.field_0x22C =  county.crop[2]     - county.grainEaten;
/// else                  county.field_0x22C = -county.grainEaten;
/// ```
///
/// **In Spring the answer is `−sown − eaten` and cannot be positive**, which is
/// the player's sentence with the arithmetic under it: sowing spends grain, so
/// the store's forecast for the season you are about to enter is a loss twice
/// over.
///
/// # Three things that are easy to get wrong here, and one that was
///
/// * **The tail is not a by-product of the loop.** The loop calls
///   `Grain_Sow(county, workers, grain − grainEaten)` over every possible
///   staffing; the tail calls `Grain_Sow(county, staff, grain)` — the *actual*
/// allocation and the *undiminished* store. Two different questions, and
///   folding them would produce a plausible wrong number.
/// * **`crop[2]` and `+0x2FC` are written only in their own seasons** and keep
/// their previous value otherwise.
///   pass. Winter's arm then reads the `crop[2]` it has just written.
/// * **`+0x22C` is zeroed before the `popBand` guard**, so an empty county
///   forecasts nothing. The same shape
/// as [`herd_preview`], and the same reason.
/// * **This is why the estimate round runs twice.** `crate::field`'s module docs
///   worked that out — *"the panel forecasts, which the estimates fill from
///   whatever the allocator last decided"* — and then the port carried none of
///   them. Knowing why a pass exists is not the same as carrying what it writes.
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
/// The original is not the inversion of [`sacks_per_field`] it looks like; it
/// is a plain forward scan:
///
/// ```c
/// best = 1; ceiling = 0;
/// for (workers = 0; workers < population; workers++) {
///     got = Grain_Sow(county, workers, grain - grainEaten);   /* season 1 */
///     if (best < got) { ceiling = workers; best = got; }
/// }
/// wanted[0] = (nothing beat 1) ? -1 : ceiling;
/// useful[0] = ceiling;
/// ```
///
/// Because `Grain_Sow` is monotone in labour and the test is strict, the answer
/// *is* "the fewest farmers that reach the best yield" — but by scanning, and
/// the loop stops one short of the population, so a county can never want every
/// one of its people on the fields. **`[D]`**, and reproduced by search here
/// for the same reason the original does: the
/// integer truncation inside `Grain_Sow` is part of the answer.
///
/// Two details that are the original's and look like slips:
///
/// * the search subtracts **`grainEaten`** from the store and the panel
///   forecast one line below it does not;
/// * `wanted` and `useful` are assigned in the same `if`, so for grain alone
/// the floor and the ceiling agree — **except when the search found
///   nothing**, where they are −1 and 0 because that is what they were
///   initialised to. That is the only reason [`GrainEstimate`] has two fields.
///   `Labour_Allocate` reads only the ceiling — **verified by exhaustion over
///   its 2,147 bytes: it reads `+0xCC + slot*0x0C` eight times and
///   `+0xC8 + slot*0x0C` not once** — so the floor is a display value and
///   nothing in the simulation depends on it.
///
/// # All four seasons
///
/// The search runs whichever of the three grain functions next season will
/// use — `Grain_Sow` entering Spring, `Grain_Harvest` entering Winter, and
/// `Grain_Grow` for the two in between, all against the crop the county is
/// carrying now. Summer, Autumn and Winter used to return `None` because
/// [`grow`] and [`harvest`] were a weather multiplier and nothing more; they
/// are now [`grow_step`] and [`harvest_step`], which are the original's, so
/// the ceiling is a real number in every season.
///
/// Returns `None` only where the original writes nothing: `popBand == 0`, an
/// empty county, whose ceiling keeps whatever it had — which for a county that
/// has never been estimated is [`crate::county::LABOUR_UNSET`], and
/// `Labour_Allocate` reads that back as zero.
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


