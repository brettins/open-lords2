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
//! property of the map*
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
//! `crates/l2-kingdom/tests/fields/main.rs`.
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
//! And the estimate/allocate round runs **twice**.
//! and it is not one: reading the five estimate bodies, **no ceiling depends on
//! the current assignment** — every one comes from a search over
//! `0 … population` or from a stock figure. What the second round is for is the
//! `Herd_UpdateCrowding` in the middle, which does move an estimate's input,
//! and the panel forecasts
//! allocator last decided. `[I]` on the reading, `[D]` on the shape.
//! [`crate::field::set_type`] reproduces the parts this crate has and says
//! plainly which it does not.

mod operations;
pub use operations::*;
mod ui;
pub use ui::*;
mod tests;
pub use tests::*;

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
    /// …and the fourth
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
    /// and the *middle* of the pasture range
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

/// The three offered on a tile that is already a field
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

