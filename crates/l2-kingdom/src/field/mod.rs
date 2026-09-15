//! The reason is that they are not primary state at all. In the original they
//! are a **cache**, recomputed by `County_RecountFields` (`FUN_00469b8d`,
//! `0x00469B8D`) from the terrain byte of the twenty map tiles named in
//! `g_countyFieldTiles` (`0x0053EA00`, 17 × 20 × `u32`). A field's *type is a
//! property of the map*
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
//! and it is not one: reading the five estimate bodies, **no ceiling depends on
//! the current assignment** — every one comes from a search over
//! `0 … population` or from a stock figure. What the second round is for is the
//! `Herd_UpdateCrowding` in the middle, which does move an estimate's input,
//! and the panel forecasts
//! allocator last decided. `[I]` on the reading, `[D]` on the shape.

mod operations;
pub use operations::*;
mod ui;
pub use ui::*;
mod tests;
pub use tests::*;

use crate::county::{County, MAX_FIELDS};
use crate::map::CampaignMap;

/// The terrain byte values `County_RecountFields` (`FUN_00469b8d`) branches on.
pub mod terrain {
    pub const WASTE: u8 = 0x00;
    pub const FALLOW: u8 = 0x01;
    pub const GRAIN: u8 = 0x02;
    pub const GRAIN_LAST: u8 = 0x0E;
    pub const PASTURE_FIRST: u8 = 0x0F;
    /// It is also **an empty pasture**: `Herd_UpdateCrowding` writes it when
    /// the county's herd is zero, and `FUN_004071A0` draws no animals on it.
    pub const PASTURE: u8 = 0x13;
    pub const PASTURE_LOW: u8 = 0x14;
    pub const PASTURE_CROWDED: u8 = 0x15;
    pub const PASTURE_PACKED: u8 = 0x16;
    pub const PASTURE_LAST: u8 = 0x16;
    pub const FLOODED: u8 = 0x17;
    pub const PARCHED: u8 = 0x18;
    pub const RECLAIM_FIRST: u8 = 0x19;
    pub const RECLAIM_LAST: u8 = 0x1C;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FieldType {
    Waste,
    Fallow,
    Grain,
    Pasture,
    Reclaiming,
}

impl FieldType {
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

/// `FUN_00438990` picks between them on the *tile's own* terrain: `0` or above
/// `0x18` gets the two-button table, anything from `1` to `0x16` the
/// three-button one — and `0x17` and `0x18`, the two blighted terrains, are
/// refused a menu at all. **`[D]`.**
pub const FIELD_BRUSHES: [FieldType; 3] =
    [FieldType::Fallow, FieldType::Grain, FieldType::Pasture];
pub const WASTE_BRUSHES: [FieldType; 2] = [FieldType::Reclaiming, FieldType::Waste];

pub const fn classify(terrain: u8) -> FieldType {
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

