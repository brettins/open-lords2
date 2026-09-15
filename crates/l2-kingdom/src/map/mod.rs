
mod site;
pub use site::*;
mod castle;
pub use castle::*;
mod campaign;
pub use campaign::*;
mod cost;
pub use cost::*;

use crate::tables::{MOVE_COST_BLOCKED, MOVE_COST_IMPASSABLE};

pub const MAP_DIM: usize = 64;

pub const MAP_TILES: usize = MAP_DIM * MAP_DIM;

pub mod flags {
    pub const ROAD: u8 = 0x01;
    pub const BOUNDARY: u8 = 0x02;
    pub const NO_COUNTY: u8 = 0x04;
    pub const ROUGH: u8 = 0x08;
    pub const PLOT: u8 = 0x10;
    pub const FARMLAND: u8 = 0x20;
    /// **Misnamed: this is the county town, not the castle.** `docs/decisions.md`
    /// C25 said so from `L2.eng`'s own words; `Map_Click` (`0x0043CE1A`) proves
    /// it from behaviour, because its whole plane-0 dispatch is three bits in
    /// this order:
    ///
    /// **Nothing behaves wrongly** — the bit is read consistently; only the word
    /// beside it is wrong, which is exactly C25's shape.
    pub const CASTLE: u8 = 0x40;
    /// The five counties with a `0x17` block are exactly the five that start
    /// owned — so `0x17` is a standing castle and `0x14` is the empty plot it
    /// gets built on, which closes the loose end C25 left open.
    pub const SETTLEMENT: u8 = 0x80;

    pub const IMPASSABLE: u8 = NO_COUNTY | ROUGH;
}

pub mod terrain {
    pub const RUINED: [u8; 4] = [3, 6, 9, 12];

    pub const TOWN: u8 = 0x14;

    pub const CASTLE_PLOT: u8 = 0x14;

/// `[V]` from the writer. `Castle_StampTile`
    /// (`0x0046826C`) is an `if`/`else if` ladder over the five levels writing
    /// `0x15, 0x16, 0x17, 0x18, 0x19`, and `Unit_Step`'s code-6 branch fires
    /// `Unit_ReachCastleBuilding` on exactly `0x14 < terrain < 0x1A`. This
    /// promotes `docs/hypotheses.json` H6, which had the mapping from two
    /// counties of one save and said so.
    ///
    /// **Confirmed a third time, by a branch that did not know about the first
    /// two.** `ai-lords-play` derived the same range independently: the England
    /// fixture's five *owned* counties carry a `0x17` block and the nine
    /// unowned ones carry `0x14`
    /// (`FUN_004A65A3`) tests exactly `0x14 < terrain < 0x1A` — a third writer
    /// and a third reader agreeing with `Castle_StampTile` and `Unit_Step`.
    pub const CASTLE_FROM: u8 = 0x15;
    pub const CASTLE_TO: u8 = 0x19;

    pub fn castle_type(terrain: u8) -> Option<u8> {
        match terrain {
            CASTLE_PLOT => Some(0),
            CASTLE_FROM..=CASTLE_TO => Some(terrain - CASTLE_PLOT),
            _ => None,
        }
    }

    pub const DWELLING: u8 = 0x10;

    /// `Unit_BurnDwelling` (`0x00468AE2`) writes `content = 0x13` over a `0x10`
    /// plot and `frame = 0x3C` beside it; the frame is the renderer's and this
    /// crate carries no frame plane.
    ///
    /// `[V]` and it is the *only* writer of this value: `County_UpdateDwellings`
    /// (`0x004684C6`) never leaves a live dwelling at `0x13` — every branch
    /// steps it up to `0x12` or beyond — so a plot at `0x13` was burnt.
    pub const DWELLING_BURNT: u8 = 0x13;

    pub const FIELD_STANDING_FROM: u8 = 2;

    pub const FIELD_STANDING_TO: u8 = 0x17;

    /// Below this a `County_DestroyField` tile is a **grain** field; at or
    /// above it, a pasture. `[D]` — `County_DestroyField` (`0x00469E5B`)
    /// branches on exactly `terrain < 0x0F`.
    pub const PASTURE_FROM: u8 = 0x0F;

    /// **`[V]` from two writers that were read independently.**
    ///
    /// `Industry_UpdateSiteTile` (`0x0044EDC2`) writes `base + (enabled != 0)`
    /// with `base` 1 iron, 4 stone, 7 weapons, 10 wood; `Unit_TrampleTile`
    /// (`0x0046873F`) writes the third value of each triple and disables the
    /// matching record. [`RUINED`] is that third column and already agreed.
    pub const INDUSTRY_IDLE: [u8; 4] = [10, 1, 7, 4];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SiteState {
    Idle,
    Working,
    Wrecked,
}

#[inline]
pub fn index(x: u8, y: u8) -> usize {
    y as usize * MAP_DIM + x as usize
}

#[inline]
pub fn coords(i: usize) -> (u8, u8) {
    ((i % MAP_DIM) as u8, (i / MAP_DIM) as u8)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CampaignMap {
    pub terrain: Vec<u8>,
    pub flags: Vec<u8>,
    /// Byte `+2` — the tile-set selector in bits `0x1C`, plus run-time draw
    /// bits. **One rule reads it**: `Map_ResolvePick` (`0x0046D5FE`) sets
    /// `DAT_005651BC` on `(bank & 0x1C) == 4`, which is what tells
    /// `TileInfo_Draw`'s rough arm a mountain from a wood. `docs/formats/maps-layers.md` §1.
    pub bank: Vec<u8>,
    pub county: Vec<u8>,
}

pub const BANK_SELECTOR: u8 = 0x1C;
pub const BANK_MOUNTAIN: u8 = 0x04;

impl Default for CampaignMap {
    fn default() -> Self {
        CampaignMap::empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one_tile(f: u8, t: u8, county: u8) -> i16 {
        let mut m = CampaignMap::empty();
        m.set_flags(10, 10, f);
        m.set_terrain(10, 10, t);
        m.set_county(10, 10, county);
        m.cost_map().at(10, 10)
    }

    #[test]
    fn the_cost_ladder_reproduces_every_branch_in_order() {
        assert_eq!(one_tile(flags::NO_COUNTY, 0, 1), 0, "sea");
        assert_eq!(one_tile(flags::ROUGH, 0, 1), 0, "mountain or woodland");
        assert_eq!(one_tile(0, 0, 17), 0, "county 17 is not a county");
        assert_eq!(one_tile(flags::ROAD, 0, 1), 1, "road");
        assert_eq!(one_tile(flags::FARMLAND, 0, 1), 3, "bare farmland");
        assert_eq!(one_tile(flags::FARMLAND, 1, 1), 3, "ploughed");
        assert_eq!(one_tile(flags::FARMLAND, 2, 1), 6, "the crop is up");
        assert_eq!(one_tile(flags::FARMLAND, 0x16, 1), 6, "still standing");
        assert_eq!(one_tile(flags::FARMLAND, 0x17, 1), 3, "harvested");
        assert_eq!(one_tile(flags::CASTLE, 0, 1), 100, "castle site");
        assert_eq!(one_tile(flags::SETTLEMENT, terrain::TOWN, 1), 3, "the county town");
        assert_eq!(one_tile(flags::SETTLEMENT, 0, 1), 100, "an intact industry site");
        assert_eq!(one_tile(flags::PLOT, terrain::DWELLING, 1), 100, "an occupied plot");
        assert_eq!(one_tile(flags::PLOT, 0, 1), 0, "an empty plot is impassable");
        assert_eq!(one_tile(0, 0, 1), 3, "open ground");
    }

    #[test]
    fn a_ruined_resource_site_becomes_impassable() {
        for ruined in terrain::RUINED {
            assert_eq!(one_tile(flags::SETTLEMENT, ruined, 1), 0, "terrain {ruined}");
        }
        assert_eq!(one_tile(flags::SETTLEMENT, 2, 1), 100);
    }

    #[test]
    fn a_road_across_farmland_costs_one() {
        assert_eq!(one_tile(flags::ROAD | flags::FARMLAND, 10, 1), 1);
        assert_eq!(one_tile(flags::ROAD | flags::SETTLEMENT, 0, 1), 1);
        assert_eq!(one_tile(flags::ROAD | flags::NO_COUNTY, 0, 1), 0);
    }

    #[test]
    fn the_standing_field_window_matches_the_six_cost_window() {
        let mut m = CampaignMap::empty();
        m.set_flags(5, 5, flags::FARMLAND);
        for t in 0..=0x20u8 {
            m.set_terrain(5, 5, t);
            let standing = m.is_standing_field(5, 5);
            let costs_six = m.cost_map().at(5, 5) == 6;
            assert_eq!(standing, costs_six, "terrain {t}");
        }
        m.set_flags(5, 5, 0);
        m.set_terrain(5, 5, 10);
        assert!(!m.is_standing_field(5, 5));
    }

    #[test]
    fn the_index_and_its_inverse_agree_over_the_whole_map() {
        for i in 0..MAP_TILES {
            let (x, y) = coords(i);
            assert_eq!(index(x, y), i);
        }
    }

    #[test]
    fn a_map_built_from_planes_of_the_wrong_size_is_refused() {
        let p = [0u8; MAP_TILES];
        assert!(CampaignMap::from_planes(&p, &p, &p, &p).is_some());
        assert!(CampaignMap::from_planes(&[0; 10], &p, &p, &p).is_none());
        assert!(CampaignMap::from_planes(&p, &p, &[0; 10], &p).is_none());
    }
}

