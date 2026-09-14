//! The campaign map the units stand on, and what each tile costs to enter —
//! `docs/armies.md` §2.2 and `docs/formats/maps-layers.md` §5.3.
//!
//! # Why the map is simulation state
//!
//! It would be tidier if the map were scenario data handed in once and never
//! written. It is not: an army that walks over a resource site **ruins the
//! tile**, and a ruined tile's movement cost changes from 100 to
//! [`crate::tables::MOVE_COST_IMPASSABLE`] — so the same county is a different
//! shape to the pathfinder afterwards. Trampling a field likewise rewrites the
//! terrain byte, which changes that tile's cost from 6 back to 3. Two lockstep
//! peers therefore have to agree about the map, and it belongs in the state
//! rather than beside it.
//!
//! # Three planes, not six
//!
//! `l2_formats::maps` reads six 64×64 planes out of `L2_maps.dat`
//! runtime tile record the original builds from them is eight bytes wide. Only
//! three of those bytes are read by any rule in this crate:
//!
//! | byte | this crate calls it | what reads it |
//! |---|---|---|
//! | `+0` | [`CampaignMap::terrain`] | the cost map, field-crossing, trampling |
//! | `+1` | [`CampaignMap::flags`] | the cost map, the stepper |
//! | `+2` | [`CampaignMap::bank`] | `Map_ResolvePick`'s mountain-or-wood test |
//! | `+7` | [`CampaignMap::county`] | the cost map, border crossings, trampling |
//!
//! The other five are graphics (`+2`, `+3`, `+6`), a plane the renderer uses
//! (`+4`)
//! because [`crate::unit::Units::at`] answers the same question from the unit
//! array and a second copy is a second thing to keep in step. `l2-kingdom` has
//! no loader and never learns what `L2_maps.dat` is; building one of these out
//! of a map file is `l2-scenario`'s job.

mod site;
pub use site::*;
mod castle;
pub use castle::*;
mod campaign;
pub use campaign::*;
mod cost;
pub use cost::*;

use crate::tables::{MOVE_COST_BLOCKED, MOVE_COST_IMPASSABLE};

/// The campaign map is 64 × 64 tiles on every shipped map.
pub const MAP_DIM: usize = 64;

/// 4,096 tiles.
pub const MAP_TILES: usize = MAP_DIM * MAP_DIM;

/// The bits of the flags byte (`+1`, plane 0). Verified in
/// `docs/formats/maps-layers.md` §2 except where noted.
pub mod flags {
    /// A road. Makes a step cost 1 instead of 3
    /// `docs/armies.md` missed — stops the flood fill expanding diagonally out
    /// of the tile at all. See [`crate::movement::flood_fill`].
    pub const ROAD: u8 = 0x01;
    /// County boundary: set on exactly the tiles 4-adjacent to a different
    /// county. Not a river, as the bit position suggests. No rule reads it.
    pub const BOUNDARY: u8 = 0x02;
    /// The tile belongs to no county — sea. Holds for 100% of tiles with
    /// county id 0.
    pub const NO_COUNTY: u8 = 0x04;
    /// Mountain or woodland.
    pub const ROUGH: u8 = 0x08;
    /// A reserved dwelling plot.
    pub const PLOT: u8 = 0x10;
    /// Farmland. The terrain byte then says which crop state it is in.
    pub const FARMLAND: u8 = 0x20;
    /// **Misnamed: this is the county town, not the castle.** `docs/decisions.md`
    /// C25 said so from `L2.eng`'s own words; `Map_Click` (`0x0043CE1A`) proves
    /// it from behaviour, because its whole plane-0 dispatch is three bits in
    /// this order:
    ///
    /// ```c
    /// if      (flags & 0x80) { ...the industry / castle toggle ladder... }
    /// else if (flags & 0x40) { g_screenId = 2; Village_Draw(1); }   /* the village */
    /// else if (flags & 0x20) { g_screenId = 4; }                    /* the field brush */
    /// ```
    ///
    /// **Clicking a `0x40` tile opens the village**, which is the county town by
    /// definition. And the England turn-one fixture agrees: every one of the
    /// fourteen counties has **exactly four** `0x40` tiles, in one 2×2 block, all
    /// at terrain 0 — verified.
    ///
    /// The constant keeps its name here for now because `cost_map` and
    /// [`crate::movement`] read it under that name in half a dozen places and
    /// renaming it means re-reading `Unit_TryEnterTile`'s return codes with it.
    /// **Nothing behaves wrongly** — the bit is read consistently; only the word
    /// beside it is wrong, which is exactly C25's shape.
    pub const CASTLE: u8 = 0x40;
    /// A settlement: an industry site **or the castle**.
    ///
    /// `County_PlaceResourceSites` puts the iron, stone and wood records on
    /// tiles carrying this bit, and `Map_Click` sends a click on one into
    /// `Industry_ToggleFromMap` through a ladder on the *terrain* byte
    /// ([`crate::industry::map_toggle_for_graphic`]).
    ///
    /// The England fixture shows the whole set, six or seven tiles a county and
    /// the same shape every time: **one iron site, one stone, one weapons, one
    /// wood, and a 2×2 block that is either terrain `0x14` or terrain `0x17`.**
    /// The five counties with a `0x17` block are exactly the five that start
    /// owned — so `0x17` is a standing castle and `0x14` is the empty plot it
    /// gets built on, which closes the loose end C25 left open.
    pub const SETTLEMENT: u8 = 0x80;

    /// `plane0 & 0x0C` — sea, mountain or woodland. The **first** test the
    /// cost map makes
    pub const IMPASSABLE: u8 = NO_COUNTY | ROUGH;
}

/// Terrain byte (`+0`) values the rules test by name.
///
/// The observed set on a live run is `0, 1, 4, 7, 10, 11, 20, 21, 22, 23`
/// (`maps-layers.md` §5.3)
/// unruined states per industry, a ruined state per industry, and four crop
/// states on farmland.
pub mod terrain {
    /// The four **ruined** states `Unit_TrampleTile` writes — one per industry
    /// record, in commodity order iron, stone, weapons, wood. A tile in one of
    /// these is *impassable* to the cost map, which is how a trampled
    /// settlement stops being a route as well as stopping being a mine.
    pub const RUINED: [u8; 4] = [3, 6, 9, 12];

    /// The one settlement terrain the cost map lets an army walk over for 3:
    /// the county town itself.
    ///
/// **It is also the bare castle plot**
    /// away. `County_FindCastleTile` stamps `0x14` over the 2×2 it finds on bit
    /// `0x80`, and `Unit_TryEnterTile` masks the settlement bit off whenever the
    /// terrain is `0x14` — which is exactly what lets an army walk across a
    /// castle plot with no castle on it. One value, one meaning: *nothing built
    /// here*. See [`CASTLE_PLOT`].
    pub const TOWN: u8 = 0x14;

    /// The **bare castle plot** — a `SETTLEMENT` tile with no castle on it.
    /// Same value as [`TOWN`]; see the note there.
    pub const CASTLE_PLOT: u8 = 0x14;

    /// A **standing castle**: `CASTLE_PLOT + castleType`, so `0x15` is a wooden
    /// palisade and `0x19` a royal castle.
    ///
/// `[V]` from the writer. `Castle_StampTile`
    /// (`0x0046826C`) is an `if`/`else if` ladder over the five levels writing
    /// `0x15, 0x16, 0x17, 0x18, 0x19`, and `Unit_Step`'s code-6 branch fires
    /// `Unit_ReachCastleBuilding` on exactly `0x14 < terrain < 0x1A`. This
    /// promotes `docs/hypotheses.json` H6, which had the mapping from two
    /// counties of one save and said so.
    /// **Confirmed a third time, by a branch that did not know about the first
    /// two.** `ai-lords-play` derived the same range independently: the England
    /// fixture's five *owned* counties carry a `0x17` block and the nine
    /// unowned ones carry `0x14`
    /// (`FUN_004A65A3`) tests exactly `0x14 < terrain < 0x1A` — a third writer
    /// and a third reader agreeing with `Castle_StampTile` and `Unit_Step`.
    /// Its duplicate constant is gone; this is the one.
    pub const CASTLE_FROM: u8 = 0x15;
    pub const CASTLE_TO: u8 = 0x19;

    /// The castle standing on a tile, as a castle **type** 0..=5, or `None` if
    pub fn castle_type(terrain: u8) -> Option<u8> {
        match terrain {
            CASTLE_PLOT => Some(0),
            CASTLE_FROM..=CASTLE_TO => Some(terrain - CASTLE_PLOT),
            _ => None,
        }
    }

/// A dwelling plot that is occupied. On plot tiles this is the
    /// only value that costs anything; every other plot is impassable.
    pub const DWELLING: u8 = 0x10;

    /// **A dwelling burnt down by an army that marched through it.**
    /// `Unit_BurnDwelling` (`0x00468AE2`) writes `content = 0x13` over a `0x10`
    /// plot and `frame = 0x3C` beside it; the frame is the renderer's and this
    /// crate carries no frame plane.
    ///
    /// `[V]` and it is the *only* writer of this value: `County_UpdateDwellings`
    /// (`0x004684C6`) never leaves a live dwelling at `0x13` — every branch
    /// steps it up to `0x12` or beyond — so a plot at `0x13` was burnt.
    /// `docs/draws-map.md` §3.2, whose arm 3 draws the sixteen damage frames
    /// over exactly this.
    pub const DWELLING_BURNT: u8 = 0x13;

    /// A farmland tile below this is bare or ploughed and costs an ordinary 3.
    pub const FIELD_STANDING_FROM: u8 = 2;

    /// …and at this value or above it has been harvested and costs 3 again.
    /// Between the two the crop is standing and costs 6.
    pub const FIELD_STANDING_TO: u8 = 0x17;

    /// Below this a `County_DestroyField` tile is a **grain** field; at or
    /// above it, a pasture. `[D]` — `County_DestroyField` (`0x00469E5B`)
    /// branches on exactly `terrain < 0x0F`.
    pub const PASTURE_FROM: u8 = 0x0F;

    /// **The first of one industry's three terrain values**, in *commodity*
    /// order — wood, iron, weapons, stone — so it indexes the same way
    /// [`crate::county::County::industry`] does.
    ///
    /// The ladder is `idle`, `working`, `wrecked` in consecutive values:
    ///
    /// | commodity | idle | working | wrecked |
    /// |---|---:|---:|---:|
    /// | iron | 1 | 2 | 3 |
    /// | stone | 4 | 5 | 6 |
    /// | weapons | 7 | 8 | 9 |
    /// | wood | 10 | 11 | 12 |
    ///
    /// **`[V]` from two writers that were read independently.**
    /// `Industry_UpdateSiteTile` (`0x0044EDC2`) writes `base + (enabled != 0)`
    /// with `base` 1 iron, 4 stone, 7 weapons, 10 wood; `Unit_TrampleTile`
    /// (`0x0046873F`) writes the third value of each triple and disables the
    /// matching record. [`RUINED`] is that third column and already agreed.
    pub const INDUSTRY_IDLE: [u8; 4] = [10, 1, 7, 4];
}

/// **What one industry's site tile is doing**, read off its terrain byte.
///
/// The three states are the three pictures `Sprite_TopIt` distinguishes and
/// the three the player can see: switched off, working (which is the *animated*
/// one), and wrecked by an army that walked over it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SiteState {
    /// `base + 0` — the industry's switch is off. A still picture.
    Idle,
    /// `base + 1` — switched on. **The wheel turns**, and how fast is a rule:
    /// `Sprite_TopIt` bands `total − totalSnapshot` at 10, 25 and 50 onto four
    /// different pulses.
    Working,
    /// `base + 2` — trampled. Three seasons or more of
    /// [`crate::county::Industry::disabled_seasons`]
    /// is drawn over it as well.
    Wrecked,
}

/// Where a tile sits
/// `y * 64 + x`.
#[inline]
pub fn index(x: u8, y: u8) -> usize {
    y as usize * MAP_DIM + x as usize
}

/// The inverse of [`index`].
#[inline]
pub fn coords(i: usize) -> (u8, u8) {
    ((i % MAP_DIM) as u8, (i / MAP_DIM) as u8)
}

/// The three tile planes the simulation reads, plus the county count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CampaignMap {
    /// Byte `+0` — the object/state class. Crop stage on farmland, industry
    /// state on a settlement, occupancy on a plot.
    pub terrain: Vec<u8>,
    /// Byte `+1` — [`flags`].
    pub flags: Vec<u8>,
    /// Byte `+2` — the tile-set selector in bits `0x1C`, plus run-time draw
    /// bits. **One rule reads it**: `Map_ResolvePick` (`0x0046D5FE`) sets
    /// `DAT_005651BC` on `(bank & 0x1C) == 4`, which is what tells
    /// `TileInfo_Draw`'s rough arm a mountain from a wood. `docs/formats/maps-layers.md` §1.
    pub bank: Vec<u8>,
    /// Byte `+7` — the county id, 1…16, or 0 for none.
    pub county: Vec<u8>,
}

/// `(bank & 0x1C) == 4` — the `Mtns` tile-set, and the whole of
/// `Map_ResolvePick`'s mountain test.
pub const BANK_SELECTOR: u8 = 0x1C;
/// The `Mtns` bank's selector value.
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

    /// The whole ladder, one row per branch, in the order the original tests
    /// them.
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

    /// **The consequence of trampling that the cost map carries**: a ruined
/// resource site is a hole in the map.
    #[test]
    fn a_ruined_resource_site_becomes_impassable() {
        for ruined in terrain::RUINED {
            assert_eq!(one_tile(flags::SETTLEMENT, ruined, 1), 0, "terrain {ruined}");
        }
// …where the same tile before it was ruined cost a lot.
        assert_eq!(one_tile(flags::SETTLEMENT, 2, 1), 100);
    }

    /// A road beats every other bit, because it is tested before them.
    #[test]
    fn a_road_across_farmland_costs_one() {
        assert_eq!(one_tile(flags::ROAD | flags::FARMLAND, 10, 1), 1);
        assert_eq!(one_tile(flags::ROAD | flags::SETTLEMENT, 0, 1), 1);
        // …but sea still wins over a road, because 0x0C is tested first.
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

