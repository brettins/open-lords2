//! Fields — what a county's twenty farm tiles are being used for.
//!
//! # The thing everything else in this crate assumed
//!
//! `County::fields_fallow`, `fields_cattle` and `fields_grain` are read by the
//! fertility rule, by sowing, by the herd's crowding and by four of the AI's
//! passes. Until now nothing in this tree **wrote** them outside a scenario
//! import: the counts arrived from a save and stayed there for the rest of the
//! game. On the England turn-one position every county's `fields_grain` is
//! **0** — verified, all fourteen — so the whole grain half of the economy was
//! finished, tested, and unreachable in play.
//!
//! The reason is that they are not primary state at all. In the original they
//! are a **cache**, recomputed by `County_RecountFields` (`FUN_00469b8d`,
//! `0x00469B8D`) from the terrain byte of the twenty map tiles named in
//! `g_countyFieldTiles` (`0x0053EA00`, 17 × 20 × `u32`). A field's *type is a
//! property of the map*, and the county record only counts them.
//!
//! # The ladder
//!
//! `County_RecountFields` is one `if`/`else if` chain over the terrain byte,
//! and it is the definition of every term in this module:
//!
//! | terrain | counted as | what it is |
//! |---|---|---|
//! | `0` | [`County::fields_waste`] | wasteland — the field is not in use |
//! | `1` | [`County::fields_fallow`] | ploughed and resting |
//! | `2 … 0x0E` | [`County::fields_grain`] | grain, at one of thirteen crop stages |
//! | `0x0F … 0x16` | [`County::fields_cattle`] | pasture |
//! | `0x17`, `0x18` | [`County::fields_waste`] | flooded / drought-struck this season |
//! | `0x19 … 0x1C` | [`County::fields_reclaiming`] | under reclamation, at each quarter |
//!
//! **`[V]`** against the England turn-one save: reading `g_countyFieldTiles`
//! and `g_tiles` straight out of the file and applying this ladder reproduces
//! all three stored counts for **all fourteen counties**, 168 field tiles — see
//! `crates/l2-kingdom/tests/fields.rs`.
//!
//! Two of those rows were not guesses either. `Field_ReclaimTick`
//! (`0x0044C093`) writes `0x19`, `0x1A`, `0x1B`, `0x1C` at 200, 400, 600 and
//! 800 units of progress and `1` when the field is finished, which is what
//! makes `0x19 … 0x1C` the four quarters and `1` the reward. `Weather_UpdateAll`
//! (`0x00449889`) writes `0x18` on one field of a county in *Drought* and
//! `0x17` on one in *Flooding*, and `FUN_0046942C` clears both back to `0` at
//! the start of the next weather pass. **`[D]`** on both.
//!
//! # The brush
//!
//! Painting a field is a **map click**, not a county-panel control: `Map_Click`
//! puts up a small popup of 48 × 48 buttons and `FUN_00438B02` passes the one
//! the player hit to `Field_SetType` (`0x00438BEC`) as a raw terrain value. The
//! five buttons are two hotspot tables in `.rdata`, and they are [`BRUSHES`]:
//!
//! | table | terrain ids | offered when the tile is |
//! |---|---|---|
//! | `0x004DC4D0`, 3 entries | `1`, `2`, `0x13` | a field: fallow, grain or pasture |
//! | `0x004DC530`, 2 entries | `0x19`, `0x0` | waste or reclaiming: start reclaiming, or abandon |
//!
//! **`[V]`** — read out of `Lords2.exe` by
//! `crates/l2-kingdom/tests/oracle.rs`, including that all five records call
//! the same handler at `0x00438B02`.
//!
//! # What `Field_SetType` does besides paint
//!
//! ```c
//! Field_PaintTile(tile, brush, 0);        /* FUN_0046D7F4 — the terrain byte */
//! County_RecountFieldsAll();              /* FUN_00469B51 — every county     */
//! if (county.fieldsReclaiming == 0) Labour_ToggleShare(county, 2, 0, 3);
//! else                             Labour_ToggleShare(county, 2, 1, 3);
//! Labour_Allocate(county); Herd_UpdateCrowding(county); County_RefreshEstimates(county, next);
//! Labour_Allocate(county); Herd_UpdateCrowding(county); County_RefreshEstimates(county, next);
//! ```
//!
//! The **reclamation labour share is switched on and off by painting**, which
//! is the answer to a question `docs/screens-county.md` §9 got wrong: it
//! guessed that county `+0x130/+0x134/+0x138` *were* the field brush. They are
//! the first three of the eight job percentages (`docs/kingdom.md` §14), and
//! the connection to fields is this one call. See
//! [`crate::labour::toggle_share`].
//!
//! And the estimate/allocate round runs **twice**. That looked like a fixpoint
//! and it is not one: reading the five estimate bodies, **no ceiling depends on
//! the current assignment** — every one comes from a search over
//! `0 … population` or from a stock figure. What the second round is for is the
//! `Herd_UpdateCrowding` in the middle, which does move an estimate's input,
//! and the panel forecasts, which the estimates fill from whatever the
//! allocator last decided. `[I]` on the reading, `[D]` on the shape.
//! [`crate::field::set_type`] reproduces the parts this crate has and says
//! plainly which it does not.

use crate::county::{County, MAX_FIELDS};
use crate::map::CampaignMap;

/// The terrain byte values `County_RecountFields` (`FUN_00469b8d`) branches on.
///
/// These are *field* terrains. The same byte on a settlement tile means an
/// industry state and on a plot tile means occupancy — see
/// [`crate::map::terrain`], which documents the values the **cost map** reads.
/// Nothing reconciles the two namespaces, because the original does not either:
/// the flags byte says which kind of tile it is and the terrain byte is read in
/// that light.
pub mod terrain {
    /// Wasteland. A field tile in no use — the state a blighted field and an
    /// abandoned one both end in.
    pub const WASTE: u8 = 0x00;
    /// Ploughed and resting. Worth [`crate::land::FERTILITY_PER_FALLOW`] a
    /// season.
    pub const FALLOW: u8 = 0x01;
    /// Grain, freshly sown — the value the brush paints.
    pub const GRAIN: u8 = 0x02;
    /// …and the last of the thirteen grain stages.
    pub const GRAIN_LAST: u8 = 0x0E;
    /// The first pasture terrain.
    pub const PASTURE_FIRST: u8 = 0x0F;
    /// Pasture — the value the brush paints, which sits inside the range.
    ///
    /// It is also **an empty pasture**: `Herd_UpdateCrowding` writes it when
    /// the county's herd is zero, and `FUN_004071A0` draws no animals on it.
    /// So the brush paints grass and the next herd pass puts the cattle in.
    pub const PASTURE: u8 = 0x13;
    /// Pasture grazed at **ten head a field or fewer** — the first of the three
    /// stocked pictures. `l2_kingdom::land::herd_graphic`.
    pub const PASTURE_LOW: u8 = 0x14;
    /// …eleven to twenty…
    pub const PASTURE_CROWDED: u8 = 0x15;
    /// …and twenty-one or more, which is also the picture for a county with no
    /// pasture at all. The map cannot tell *"Herd overcrowded."* from
    /// *"Massive overcrowding!!"*.
    pub const PASTURE_PACKED: u8 = 0x16;
    /// …and the last.
    pub const PASTURE_LAST: u8 = 0x16;
    /// A field ruined by *Flooding* this season (`Weather_UpdateAll`).
    pub const FLOODED: u8 = 0x17;
    /// A field ruined by *Drought* this season.
    pub const PARCHED: u8 = 0x18;
    /// Reclamation, first quarter — the value the brush paints.
    pub const RECLAIM_FIRST: u8 = 0x19;
    /// …and the fourth, at 600 of 800 units of progress.
    pub const RECLAIM_LAST: u8 = 0x1C;
}

/// What one field tile is being used for.
///
/// Five states, because [`County`] keeps five counts: the three the rules read
/// plus the two `County_RecountFields` also fills in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FieldType {
    /// Not in use. Includes the two weather-blighted terrains, which is the
    /// original's own grouping and not a simplification.
    Waste,
    Fallow,
    Grain,
    Pasture,
    Reclaiming,
}

impl FieldType {
    /// The terrain value the **brush** paints for this type, or `None` for a
    /// type no button offers.
    ///
    /// Grain is painted as `2` and pasture as `0x13` — the *first* grain stage
    /// and the *middle* of the pasture range, which is what the hotspot table
    /// holds.
    pub const fn brush(self) -> u8 {
        match self {
            FieldType::Waste => terrain::WASTE,
            FieldType::Fallow => terrain::FALLOW,
            FieldType::Grain => terrain::GRAIN,
            FieldType::Pasture => terrain::PASTURE,
            FieldType::Reclaiming => terrain::RECLAIM_FIRST,
        }
    }

    /// The `L2.eng`-independent English name, for our own UI. **Ours** — the
    /// original labels the buttons with pictures.
    pub const fn name(self) -> &'static str {
        match self {
            FieldType::Waste => "Waste",
            FieldType::Fallow => "Fallow",
            FieldType::Grain => "Grain",
            FieldType::Pasture => "Pasture",
            FieldType::Reclaiming => "Reclaiming",
        }
    }
}

/// The five brush buttons, in the order the two hotspot tables hold them.
///
/// **`[V]`** out of `Lords2.exe`: `0x004DC4D0` holds three 24-byte records with
/// ids 1, 2 and 0x13 at x 240/304/368, and `0x004DC530` two with ids 0x19 and 0
/// at x 304/368; every one calls `0x00438B02`.
pub const BRUSHES: [FieldType; 5] = [
    FieldType::Fallow,
    FieldType::Grain,
    FieldType::Pasture,
    FieldType::Reclaiming,
    FieldType::Waste,
];

/// The three offered on a tile that is already a field, and the two offered on
/// a tile that is not.
///
/// `FUN_00438990` picks between them on the *tile's own* terrain: `0` or above
/// `0x18` gets the two-button table, anything from `1` to `0x16` the
/// three-button one — and `0x17` and `0x18`, the two blighted terrains, are
/// refused a menu at all. **`[D]`.**
pub const FIELD_BRUSHES: [FieldType; 3] =
    [FieldType::Fallow, FieldType::Grain, FieldType::Pasture];
pub const WASTE_BRUSHES: [FieldType; 2] = [FieldType::Reclaiming, FieldType::Waste];

/// `County_RecountFields`' ladder, as a function.
pub const fn classify(terrain: u8) -> FieldType {
    // Written in the original's own order, because the order is the rule: `0`,
    // `0x17` and `0x18` are tested *before* the ranges, which is the only
    // reason two values inside the pasture-and-beyond span come out as waste.
    if terrain == terrain::WASTE || terrain == terrain::FLOODED || terrain == terrain::PARCHED {
        FieldType::Waste
    } else if terrain < terrain::FALLOW + 1 {
        FieldType::Fallow
    } else if terrain <= terrain::GRAIN_LAST {
        FieldType::Grain
    } else if terrain <= terrain::PASTURE_LAST {
        FieldType::Pasture
    } else {
        FieldType::Reclaiming
    }
}

/// `County_RecountFields` (`FUN_00469b8d`) — rewrite one county's five field
/// counts from the map.
///
/// A slot holding tile 0 is empty and is skipped, which is the original's test
/// (`g_countyFieldTiles` stores a **byte offset** into an 8-byte-per-tile
/// array, so 0 doubles as "no field" and as tile (0, 0); every shipped map has
/// its counties well inside the plane, so the ambiguity never arises).
pub fn recount(county: &mut County, map: &CampaignMap) {
    county.fields_fallow = 0;
    county.fields_cattle = 0;
    county.fields_grain = 0;
    county.fields_waste = 0;
    county.fields_reclaiming = 0;
    for slot in 0..MAX_FIELDS {
        let Some(tile) = county.field_tile(slot) else { continue };
        match classify(map.terrain[tile]) {
            FieldType::Waste => county.fields_waste += 1,
            FieldType::Fallow => county.fields_fallow += 1,
            FieldType::Grain => county.fields_grain += 1,
            FieldType::Pasture => county.fields_cattle += 1,
            FieldType::Reclaiming => county.fields_reclaiming += 1,
        }
    }
}

/// `County_RecountFieldsAll` (`FUN_00469b51`) — every county, ascending.
pub fn recount_all(counties: &mut [County], county_count: usize, map: &CampaignMap) {
    for county in counties.iter_mut().take(county_count + 1).skip(1) {
        recount(county, map);
    }
}

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
/// reclamation, grain in every season, the herd, the four industries and the
/// castle. That is why this takes the **realms**: the industry ceilings read the
/// owning realm's stockpile, so `County_RefreshEstimates` is not a one-county
/// function at all.
///
/// The one thing still inferred is the castle's materials gate; see
/// [`crate::industry::castle_labour_estimate`] and
/// `crates/l2-kingdom/tests/labour_gap.rs`.
///
/// The argument list is long because a brush stroke reaches six things and
/// this crate takes its world as values rather than owning it —
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
    // Realm 0 is not a realm; an unowned county's industry ceilings are all 0
    // whatever is in this record, because `Industry_LabourEstimate` tests the
    // owner first.
    let neutral = crate::realm::Realm::new();
    for _ in 0..2 {
        crate::labour::allocate(&mut counties[county]);
        // The blacksmith's share moves with who is *staffed*, and the
        // allocation just above is what staffs them, so this is recomputed
        // inside the loop exactly as the original recomputes it inside every
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
/// **It reads the owning realm**, which is why this takes one: the blacksmith's
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
    // afterwards — the sowing, growth and harvest forecasts and the signed
    // change the sidebar's grain row draws. It runs unconditionally because the
    // original's `+0x22C = 0` is *outside* the `popBand` guard, so an empty
    // county forecasts nothing rather than keeping last season's number.
    //
    // It is called from here rather than from a season tick because this is
    // where the original computes it: `County_RefreshEstimates` runs **after**
    // `Labour_Allocate` in each of the round's two passes, and the tail reads
    // `labour[0].workers` — the allocator's answer, not the search's.
    // `docs/decisions.md` CNEW-grain-forecast.
    crate::land::grain_preview(tables, county, season_next, advanced_farming);
    if county.pop_band != 0 {
        county.labour_useful[JOB_CATTLE_FARMING] =
            crate::land::herd_labour_estimate(tables, county, season_next.index());
    }

    for c in crate::tables::INDUSTRY_ESTIMATE_ORDER {
        let job = tables.commodity[c.index()].job;
        let (wanted, useful) = crate::industry::labour_estimate(
            tables,
            county,
            c,
            realm,
            weapon_share,
            advanced_farming,
        );
        county.labour_wanted[job] = wanted;
        county.labour_useful[job] = useful;
    }

    let (wanted, useful) = crate::industry::castle_labour_estimate(tables, county);
    county.labour_wanted[crate::tables::JOB_CASTLE_BUILDING] = wanted;
    county.labour_useful[crate::tables::JOB_CASTLE_BUILDING] = useful;
}

/// `Field_PaintTile` (`FUN_0046D7F4`) — the terrain byte, and nothing else.
///
/// The original also rewrites the tile's graphics bank and frame from a ladder
/// on the same value. That is presentation and lives in `l2-view`
/// (`campaign::field_graphic`); this crate holds no graphics.
pub fn paint_tile(map: &mut CampaignMap, tile: usize, terrain: u8) {
    map.terrain[tile] = terrain;
}

/// `FUN_00469D21(county, terrain, 0, first, last)` — **repaint every one of a
/// county's field tiles whose terrain is in `first ..= last`.**
///
/// The original sweeps all 4,096 tiles rather than the county's twenty slots,
/// testing `tile.county == county && (tile.flags & 0x20)`; the two are the same
/// set by construction ([`recount`] builds the slots from exactly that test)
/// and the slots are what this crate has.
///
/// **The `param_4 == 2` clause is not reproduced and this is why.** The
/// original carries an extra arm — *if the range starts at 2 and the county's
/// `+0x1A7` is set and this is not the first tile matched, write `2` instead of
/// the requested terrain* — which is `Grain_SeasonTick`'s business, not the
/// herd's, and `Herd_UpdateCrowding` passes `first = 0x13`. Naming it here
/// rather than silently narrowing the helper: a caller that wants the grain
/// half needs to bring it. **[V]**
pub fn repaint_range(county: &County, map: &mut CampaignMap, terrain: u8, range: (u8, u8)) {
    for slot in 0..MAX_FIELDS {
        let Some(tile) = county.field_tile(slot) else { continue };
        if (range.0..=range.1).contains(&map.terrain[tile]) {
            map.terrain[tile] = terrain;
        }
    }
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

/// Whether `terrain` is a field the **AI's brush** counts as `kind`.
///
/// This is deliberately *not* [`classify`], and the difference is a real one
/// worth stating. `FUN_004697CD` and `FUN_0046988D` both test grain as
/// `1 < t && t < 0x0F` — the whole crop range, which agrees with the recount —
/// but pasture as `0x12 < t && t < 0x17`, which is only `0x13 … 0x16`. **A
/// pasture at terrain `0x0F … 0x12` is counted by the recount and invisible to
/// the AI's brush.** `[D]`, and the two comparisons sit four lines apart in
/// both functions, so it is not a decompiler artefact.
fn ai_brush_matches(kind: FieldType, terrain: u8) -> bool {
    match kind {
        FieldType::Grain => (terrain::GRAIN..=terrain::GRAIN_LAST).contains(&terrain),
        FieldType::Pasture => (terrain::PASTURE..=terrain::PASTURE_LAST).contains(&terrain),
        _ => false,
    }
}

/// `FUN_0044C6C4` — order `want` of the county's fields put under
/// reclamation. Returns how many wasteland tiles were actually started.
///
/// **This is what the AI's "add a field" ladder really does**, and it is not
/// what it looks like. `crate::tables::AI_FIELD_LADDER` reads as *"give the
/// county another field"*, and until now this crate implemented it by adding
/// one to `County::fields_fallow` — a counter [`recount`] overwrites from the
/// map on the next pass, so the field evaporated. The original paints
/// [`terrain::RECLAIM_FIRST`] onto a **wasteland** tile and lets
/// `Field_ReclaimTick` finish it over four stages; the county's counts follow
/// from the map, as everything in this module does.
///
/// The walk is one pass in slot order with a quota, and the quota is spent by
/// two different things:
///
/// ```c
/// if (terrain < 0x19) {
///     if (terrain != 0) continue;     /* a field in use: skipped, quota intact */
///     paint(tile, 0x19);              /* wasteland: started */
/// }
/// if (--want < 1) return;             /* reached for a fresh start *and* for
///                                        a field already reclaiming */
/// ```
///
/// So **a field already under reclamation consumes a place in the quota**
/// without anything happening. A county with one field already being reclaimed
/// that is told to add one adds nothing at all, and only the county told to add
/// two gets a second going. `[V]` — the `goto` in the decompilation skips the
/// decrement for an in-use field and falls through to it for a reclaiming one,
/// which is the whole of the difference.
///
/// A quota of zero or less starts nothing: the decrement runs before the test,
/// so the first tile considered ends it.
pub fn order_reclamation(county: &County, map: &mut CampaignMap, mut want: i32) -> i32 {
    let mut started = 0;
    for slot in 0..MAX_FIELDS {
        let Some(tile) = county.field_tile(slot) else { continue };
        let here = map.terrain[tile];
        if here < terrain::RECLAIM_FIRST {
            if here != terrain::WASTE {
                continue;
            }
            map.terrain[tile] = terrain::RECLAIM_FIRST;
            started += 1;
        }
        want -= 1;
        if want < 1 {
            break;
        }
    }
    started
}

/// `FUN_004697CD` — turn every field of one type back to fallow.
///
/// The AI's farming styles open with this: *"forget what I said last year"*.
/// It matches by `ai_brush_matches` above, so a grain field halfway through its
/// thirteen stages is cleared as readily as a freshly sown one.
pub fn clear_type(county: &County, map: &mut CampaignMap, kind: FieldType) {
    for slot in 0..MAX_FIELDS {
        let Some(tile) = county.field_tile(slot) else { continue };
        if ai_brush_matches(kind, map.terrain[tile]) {
            map.terrain[tile] = terrain::FALLOW;
        }
    }
}

/// `FUN_0046988D` — make exactly `count` of the county's fields this type, and
/// return everything else of that type to fallow.
///
/// This is how an AI lord farms (`FUN_004A3C67` and its siblings call it with
/// grain and half the county's fields, every Winter). The walk is one pass in
/// slot order: a **fallow** field is converted while the quota lasts, and a
/// field already of this type either consumes a quota place or is turned back
/// to fallow. Returns how many places went unused.
pub fn set_count(
    county: &County,
    map: &mut CampaignMap,
    kind: FieldType,
    mut count: i32,
) -> i32 {
    for slot in 0..MAX_FIELDS {
        let Some(tile) = county.field_tile(slot) else { continue };
        let here = map.terrain[tile];
        if here == terrain::FALLOW && count > 0 {
            map.terrain[tile] = kind.brush();
            count -= 1;
        } else if ai_brush_matches(kind, here) {
            if count < 1 {
                map.terrain[tile] = terrain::FALLOW;
            } else {
                count -= 1;
            }
        }
    }
    count.max(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// No realms at all: every county here is unowned, so every industry
    /// ceiling is 0 whatever record the estimate would have read.
    const NO_REALMS: [crate::realm::Realm; 0] = [];
    use crate::tables::{Season, Tables, JOB_FIELD_RECLAMATION};

    /// One county with `n` field tiles laid along row 8, all fallow.
    fn county_with(n: usize) -> (County, CampaignMap) {
        let mut c = County::new();
        let mut map = CampaignMap::empty();
        for i in 0..n {
            let tile = crate::map::index(i as u8, 8);
            c.set_field_tile(i, Some(tile));
            map.terrain[tile] = terrain::FALLOW;
        }
        (c, map)
    }

    /// The ladder, at every boundary the original branches on. This is the
    /// whole rule, and the boundaries are where a transcription slip would
    /// hide.
    #[test]
    fn the_ladder_classifies_every_terrain_the_original_branches_on() {
        use FieldType::*;
        let want = [
            (0x00, Waste),
            (0x01, Fallow),
            (0x02, Grain),
            (0x0E, Grain),
            (0x0F, Pasture),
            (0x13, Pasture),
            (0x16, Pasture),
            (0x17, Waste),
            (0x18, Waste),
            (0x19, Reclaiming),
            (0x1C, Reclaiming),
            (0xFF, Reclaiming),
        ];
        for (terrain, kind) in want {
            assert_eq!(classify(terrain), kind, "terrain {terrain:#04x}");
        }
    }

    /// Every one of the 256 terrain bytes lands in exactly one bucket, and the
    /// five buckets tile the range with no gap — which is what makes the five
    /// counts sum to the field total.
    #[test]
    fn the_five_counts_always_sum_to_the_number_of_fields() {
        let (mut c, mut map) = county_with(20);
        for t in 0..=255u8 {
            for slot in 0..20 {
                map.terrain[c.field_tile(slot).unwrap()] = t;
            }
            recount(&mut c, &map);
            let total = c.fields_fallow
                + c.fields_cattle
                + c.fields_grain
                + c.fields_waste
                + c.fields_reclaiming;
            assert_eq!(total, 20, "terrain {t:#04x} counted {total} of 20 fields");
        }
    }

    /// **The hole this module exists to close.** A county starts with no grain
    /// fields at all; painting one and recounting gives it one.
    #[test]
    fn painting_a_fallow_field_to_grain_gives_the_county_a_grain_field() {
        let mut counties = vec![County::new(); 3];
        let (c, mut map) = county_with(6);
        counties[1] = c;
        recount(&mut counties[1], &map);
        assert_eq!(counties[1].fields_grain, 0, "nothing is sown to begin with");
        assert_eq!(counties[1].fields_fallow, 6);

        let tile = counties[1].field_tile(0).unwrap();
        set_type(&mut counties, 2, &mut map, 1, tile, FieldType::Grain, Season::Spring, &Tables::DEFAULT, false, &NO_REALMS).unwrap();
        assert_eq!(counties[1].fields_grain, 1);
        assert_eq!(counties[1].fields_fallow, 5);
        assert_eq!(map.terrain[tile], terrain::GRAIN);
    }

    /// A tile that is not one of the county's twenty fields is not a field,
    /// however much it looks like one.
    #[test]
    fn only_the_countys_own_field_tiles_can_be_painted() {
        let mut counties = vec![County::new(); 3];
        let (c, mut map) = county_with(4);
        counties[1] = c;
        let stranger = crate::map::index(40, 40);
        assert_eq!(
            set_type(&mut counties, 2, &mut map, 1, stranger, FieldType::Grain, Season::Spring, &Tables::DEFAULT, false, &NO_REALMS),
            Err(BrushRefusal::NotAField)
        );
    }

    /// The two menus are disjoint, and a brush from the wrong one is refused —
    /// which is the original's hotspot geometry restated as a rule.
    #[test]
    fn a_field_cannot_be_reclaimed_and_waste_cannot_be_sown() {
        let mut counties = vec![County::new(); 3];
        let (c, mut map) = county_with(4);
        counties[1] = c;
        let tile = counties[1].field_tile(0).unwrap();

        assert_eq!(
            set_type(&mut counties, 2, &mut map, 1, tile, FieldType::Reclaiming, Season::Spring, &Tables::DEFAULT, false, &NO_REALMS),
            Err(BrushRefusal::WrongMenu),
            "a standing field has no reclaim button"
        );

        map.terrain[tile] = terrain::WASTE;
        assert_eq!(
            set_type(&mut counties, 2, &mut map, 1, tile, FieldType::Grain, Season::Spring, &Tables::DEFAULT, false, &NO_REALMS),
            Err(BrushRefusal::WrongMenu),
            "waste has to be reclaimed before it can be sown"
        );
        set_type(&mut counties, 2, &mut map, 1, tile, FieldType::Reclaiming, Season::Spring, &Tables::DEFAULT, false, &NO_REALMS).unwrap();
        assert_eq!(map.terrain[tile], terrain::RECLAIM_FIRST);

        map.terrain[tile] = terrain::FLOODED;
        assert_eq!(
            set_type(&mut counties, 2, &mut map, 1, tile, FieldType::Fallow, Season::Spring, &Tables::DEFAULT, false, &NO_REALMS),
            Err(BrushRefusal::Blighted),
            "a ruined field opens no menu at all"
        );
    }

    /// Painting is what switches the reclamation share in and out of the farm
    /// group — the connection `docs/screens-county.md` §9 half-saw and read
    /// backwards.
    #[test]
    fn starting_a_reclamation_gives_the_job_a_share_of_the_farm() {
        let mut counties = vec![County::new(); 3];
        let (c, mut map) = county_with(4);
        counties[1] = c;
        counties[1].labour_share = [50, 50, 0, 0, 0, 0, 100, 0];
        let tile = counties[1].field_tile(0).unwrap();
        map.terrain[tile] = terrain::WASTE;

        set_type(&mut counties, 2, &mut map, 1, tile, FieldType::Reclaiming, Season::Spring, &Tables::DEFAULT, false, &NO_REALMS).unwrap();
        assert!(
            counties[1].labour_share[JOB_FIELD_RECLAMATION] > 0,
            "somebody has to do the reclaiming: {:?}",
            counties[1].labour_share
        );
        let farm: i32 = counties[1].labour_share[..3].iter().sum();
        assert_eq!(farm, 100, "and the farm group still closes");

        // Abandon it again and the share goes back where it came from.
        set_type(&mut counties, 2, &mut map, 1, tile, FieldType::Waste, Season::Spring, &Tables::DEFAULT, false, &NO_REALMS).unwrap();
        assert_eq!(counties[1].labour_share[JOB_FIELD_RECLAMATION], 0);
        assert_eq!(counties[1].labour_share[..3].iter().sum::<i32>(), 100);
    }

    /// The AI's brush: "make six of them grain" is idempotent, and shrinking
    /// the number puts the difference back to fallow rather than to waste.
    #[test]
    fn setting_a_count_converts_up_and_down_and_never_creates_waste() {
        let (c, mut map) = county_with(12);
        assert_eq!(set_count(&c, &mut map, FieldType::Grain, 6), 0);
        let mut n = c.clone();
        recount(&mut n, &map);
        assert_eq!((n.fields_grain, n.fields_fallow, n.fields_waste), (6, 6, 0));

        set_count(&c, &mut map, FieldType::Grain, 6);
        recount(&mut n, &map);
        assert_eq!(n.fields_grain, 6, "asking for the same number changes nothing");

        set_count(&c, &mut map, FieldType::Grain, 2);
        recount(&mut n, &map);
        assert_eq!((n.fields_grain, n.fields_fallow, n.fields_waste), (2, 10, 0));

        // More than there are fields: the quota simply runs out.
        assert_eq!(set_count(&c, &mut map, FieldType::Grain, 30), 18);
        recount(&mut n, &map);
        assert_eq!(n.fields_grain, 12);
    }

    /// Clearing matches the whole crop range, not just the brush value: a
    /// field halfway through its growing season is still a grain field.
    #[test]
    fn clearing_a_type_matches_every_stage_of_it() {
        let (c, mut map) = county_with(3);
        for (slot, t) in [terrain::GRAIN, terrain::GRAIN_LAST, terrain::PASTURE_LAST]
            .into_iter()
            .enumerate()
        {
            map.terrain[c.field_tile(slot).unwrap()] = t;
        }
        clear_type(&c, &mut map, FieldType::Grain);
        let mut n = c.clone();
        recount(&mut n, &map);
        assert_eq!((n.fields_grain, n.fields_cattle, n.fields_fallow), (0, 1, 2));
    }
}
