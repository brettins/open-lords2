#![allow(unused_imports)]
use super::*;
use super::operations::*;
use super::tests::*;
use crate::county::{County, MAX_FIELDS};
use crate::map::CampaignMap;

/// Why a brush stroke was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BrushRefusal {
    /// The tile is not one of this county's twenty fields.
    NotAField,
    /// The tile is blighted this season. `FUN_00438990` masks the hotspot off
    /// for terrain `0x17` and `0x18`, so no menu opens on a ruined field.
    Blighted,
    /// The brush is not one the tile's own menu offers — a field cannot be
    /// abandoned or reclaimed, and waste cannot be sown directly.
    WrongMenu,
}

/// Which of the two brush menus a tile opens, or `None` if it opens neither.
///
/// **`[D]`** — `FUN_00438990`'s test, exactly: terrain `0x17` and `0x18` are
/// excluded before anything else, then `0` or `> 0x18` gets [`WASTE_BRUSHES`]
/// and everything left gets [`FIELD_BRUSHES`].
pub fn menu_for(terrain: u8) -> Option<&'static [FieldType]> {
    if terrain == terrain::FLOODED || terrain == terrain::PARCHED {
        return None;
    }
    if terrain == terrain::WASTE || terrain > terrain::PARCHED {
        Some(&WASTE_BRUSHES)
    } else {
        Some(&FIELD_BRUSHES)
    }
}

/// `Field_SetType` (`0x00438BEC`) — a brush stroke, end to end.
///
/// ```c
/// Field_PaintTile(tile, brush, 0);
/// County_RecountFieldsAll();
/// Labour_ToggleShare(county, 2, county.fieldsReclaiming != 0, 3);
/// twice: { Labour_Allocate(county); Herd_UpdateCrowding(county);
///          County_RefreshEstimates(county, g_seasonNext); }
/// ```
///
/// **The doubled round is not a typo and it is not a fixpoint.** No ceiling
/// depends on the current assignment — every one comes from a search over
/// `0 … population` or from a stock figure. What the second round is for is
/// `Herd_UpdateCrowding` in the middle, which *does* move an estimate input,
/// and the panel forecasts the estimates fill from the final assignment.
/// **`[I]`** on the reading, `[D]` on the shape.
///
/// # All nine ceilings are computed here
///
/// [`refresh_estimates`] is `County_RefreshEstimates` in full — the recount,
/// reclamation
/// castle. That is why this takes the **realms**: the industry ceilings read the
/// owning realm's stockpile
/// function at all.
///
/// The one thing still inferred is the castle's materials gate; see
/// [`crate::industry::castle_labour_estimate`] and
/// `crates/l2-kingdom/tests/labour_gap.rs`.
///
/// The argument list is long because a brush stroke reaches six things and
/// this crate takes its world as values —
/// [`crate::Kingdom::paint_field`] is the call a caller should make.
#[allow(clippy::too_many_arguments)]
pub fn set_type(
    counties: &mut [County],
    county_count: usize,
    map: &mut CampaignMap,
    county: usize,
    tile: usize,
    brush: FieldType,
    season_next: crate::tables::Season,
    tables: &crate::tables::Tables,
    advanced_farming: bool,
    realms: &[crate::realm::Realm],
) -> Result<(), BrushRefusal> {
    if counties[county].field_slot(tile).is_none() {
        return Err(BrushRefusal::NotAField);
    }
    let menu = menu_for(map.terrain[tile]).ok_or(BrushRefusal::Blighted)?;
    if !menu.contains(&brush) {
        return Err(BrushRefusal::WrongMenu);
    }
    paint_tile(map, tile, brush.brush());
    recount_all(counties, county_count, map);
    // `if (county.fieldsReclaiming == 0) off else on`, on job 2 of the
    // three-job farm group.
    let on = counties[county].fields_reclaiming != 0;
    crate::labour::toggle_share(
        &mut counties[county],
        crate::tables::JOB_FIELD_RECLAMATION,
        on,
        crate::labour::FARM_GROUP_DIVISOR,
    );
    let owner = counties[county].owner;
// an unowned county's industry ceilings are all 0
    // whatever is in this record, because `Industry_LabourEstimate` tests the
    // owner first.
    let neutral = crate::realm::Realm::new();
    for _ in 0..2 {
        crate::labour::allocate(&mut counties[county]);
        // The blacksmith's share moves with who is *staffed*
        // allocation just above is what staffs them, so this is recomputed
        // inside the loop
        // `resourceLimit`.
        let share = crate::industry::weapon_shares(tables, counties, county_count, owner);
        let realm = realms.get(owner as usize).unwrap_or(&neutral);
        let c = &mut counties[county];
        // `Field_SetType`'s own call, both halves of it. Painting a pasture
        // writes `0x13`, and this is the line that puts the animals on it —
        // before `refresh_estimates` reads the map back.
        herd_update_crowding(tables, c, map);
        refresh_estimates(c, map, season_next, tables, advanced_farming, realm, share);
    }
    Ok(())
}

/// `County_RefreshEstimates` (`0x004485A5`) — **all nine calls.**
///
/// ```c
/// County_RecountFields(county);
/// Field_ReclaimEstimate(county);
/// Grain_LabourEstimate(county, seasonNext);
/// Herd_LabourEstimate(county, seasonNext);
/// Industry_LabourEstimate(county, 1, 4, 15, 1);   /* iron      */
/// Industry_LabourEstimate(county, 3, 5, 15, 2);   /* stone     */
/// Industry_LabourEstimate(county, 0, 6, 20, 1);   /* wood      */
/// Industry_LabourEstimate(county, 2, 7, 15, 4);   /* weapons   */
/// Castle_BuildEstimate(county);
/// ```
///
/// The four `Industry_LabourEstimate` argument lists are the same job, base
/// efficiency and divisor mapping [`crate::tables::COMMODITY`] already holds —
/// which is a second, independent reading of that table.
///
/// **It reads the owning realm**, so this takes one: the blacksmith's
/// ceiling is a share of the realm's wood and iron
/// ([`crate::industry::weapon_shares`]), and every industry's ceiling is 0 in a
/// county nobody owns. That is the whole reason the shipped save's owned
/// counties are full of foresters and its neutral ones full of idlers.
///
/// The recount is the caller's — the original recounts *this* county here and
/// every county from `Field_SetType`.
pub fn refresh_estimates(
    county: &mut County,
    map: &CampaignMap,
    season_next: crate::tables::Season,
    tables: &crate::tables::Tables,
    advanced_farming: bool,
    realm: &crate::realm::Realm,
    weapon_share: crate::industry::WeaponShare,
) {
    use crate::county::LABOUR_NO_FLOOR;
    use crate::tables::{JOB_CATTLE_FARMING, JOB_FIELD_RECLAMATION, JOB_GRAIN_FARMING};

    recount(county, map);

    county.labour_wanted[JOB_FIELD_RECLAMATION] = LABOUR_NO_FLOOR;
    county.labour_useful[JOB_FIELD_RECLAMATION] =
        crate::land::reclaim_labour_estimate(tables, county, map);
    // **`Field_ReclaimEstimate`'s tail** — the third of the evening. The loop
    // above is its first half; this is the season it then simulates, which
    // writes the two figures the sidebar's reclamation row draws.
    // `docs/decisions.md` C129.
    crate::land::reclaim_preview(tables, county, map);

    // Grain and the herd both write nothing at all in a county with no people —
    // `popBand == 0` is the original's guard on both — so a ceiling that was
    // never computed keeps `LABOUR_UNSET`, which `Labour_Allocate` reads as 0.
    if let Some(grain) =
        crate::land::grain_labour_estimate(tables, county, season_next, advanced_farming)
    {
        county.labour_wanted[JOB_GRAIN_FARMING] = grain.wanted;
        county.labour_useful[JOB_GRAIN_FARMING] = grain.useful;
    }
    // **`Grain_LabourEstimate`'s tail, which is one function in the original and
    // two here.** The estimate above is its search loop; this is what it writes
    // afterwards — the sowing
    // change the sidebar's grain row draws. It runs unconditionally because the
    // original's `+0x22C = 0` is *outside* the `popBand` guard, so an empty
// county forecasts nothing.
    //
// It is called from here
    // where the original computes it: `County_RefreshEstimates` runs **after**
    // `Labour_Allocate` in each of the round's two passes
    // `labour[0].workers` — the allocator's answer, not the search's.
    // `docs/decisions.md` C123.
    crate::land::grain_preview(tables, county, season_next, advanced_farming);
    if county.pop_band != 0 {
        // **Both words of labour record 1.** `Herd_LabourEstimate` writes the
        // break-even staffing to `+0xD4` and the growth-maximising one to
        // `+0xD8` out of one loop; ours took only the second for a long time,
        // so the milkmaid count never went red however far the herd was from
        // being tended. `docs/decisions.md` C187.
        let herd = crate::land::herd_labour_estimate(tables, county, season_next.index());
        county.labour_wanted[JOB_CATTLE_FARMING] = herd.wanted;
        county.labour_useful[JOB_CATTLE_FARMING] = herd.useful;
    }
    // **`Herd_LabourEstimate`'s tail**.
    //
    // A player: *"I right now have −11 cattle. If I move it so the people are
    // eating cattle, it still says −11 cattle in the sidebar."* He is right, and
    // the number itself
    // `L2.eng` 77/28 *"Overall change"*, and named `herdChangeFromFarming`
    // after 77/7 until this was read) is
    // `(births − deaths) − herdEaten`
    // is what misleads. What was wrong is *when it is computed*.
    //
    // `Herd_LabourEstimate` (`0x0044DD4D`) is one function: a search loop that
    // fills the cattle ceiling, then a tail that re-runs `Herd_BirthsAndDeaths`
    // at the **actual** staffing and writes the three forecast fields. The
    // original calls it from **both** `Herd_SeasonTick`'s last line and
    // `County_RefreshEstimates` — so every control that reallocates labour or
    // re-applies the ration moves the forecast. We had the loop here and the
    // tail in `herd_season_tick` alone, so the number only moved once a season.
    //
    // That is the third time this exact split has bitten: the loop is the part
    // that looks like the function
    // reads. `docs/decisions.md` C128.
    crate::land::herd_preview(tables, county, season_next.index());

    for c in crate::tables::INDUSTRY_ESTIMATE_ORDER {
        // **`Industry_LabourEstimate`, loop and tail.** The tail is one
        // function in the original and a second half here for the same reason
        // `grain_preview` is: the search loop answers a question and the tail
        // *writes* four things
        // forecast is the one the sidebar draws. `docs/decisions.md` C123 is
        // the grain case; C136 is this one.
        crate::industry::refresh(tables, county, c, realm, weapon_share, advanced_farming);
    }

    let (wanted, useful) = crate::industry::castle_labour_estimate(tables, county);
    county.labour_wanted[crate::tables::JOB_CASTLE_BUILDING] = wanted;
    county.labour_useful[crate::tables::JOB_CASTLE_BUILDING] = useful;
}

/// **`Herd_UpdateCrowding` (`0x0044D913`) in full** — the crowding level *and*
/// the picture on the ground.
///
/// The original is one function and does both, which is the only reason the
/// pasture on a map ever shows the right number of animals: nothing else writes
/// terrain `0x14 … 0x16`. Splitting them was how our map came to have pastures
/// with no cattle in them — [`crate::land::herd_crowding`] was reproduced and
/// its other half was not.
///
/// Call it wherever the herd or the pasture count can have moved. The original
/// calls it from `Field_SetType`, `Herd_SeasonTick`, the trade screen, county
/// setup and four places in the AI's farm pass, and every one of those is a
/// place `county.herd` or `county.fields_cattle` has just changed.
pub fn herd_update_crowding(
    tables: &crate::tables::Tables,
    county: &mut County,
    map: &mut CampaignMap,
) {
    county.herd_crowding = crate::land::herd_crowding(tables, county.herd, county.fields_cattle);
    let graphic = crate::land::herd_graphic(tables, county.herd, county.fields_cattle);
    repaint_range(county, map, graphic, (terrain::PASTURE, terrain::PASTURE_LAST));
}

