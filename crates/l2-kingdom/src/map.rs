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
//! `l2_formats::maps` reads six 64×64 planes out of `L2_maps.dat`, and the
//! runtime tile record the original builds from them is eight bytes wide. Only
//! three of those bytes are read by any rule in this crate:
//!
//! | byte | this crate calls it | what reads it |
//! |---|---|---|
//! | `+0` | [`CampaignMap::terrain`] | the cost map, field-crossing, trampling |
//! | `+1` | [`CampaignMap::flags`] | the cost map, the stepper |
//! | `+7` | [`CampaignMap::county`] | the cost map, border crossings, trampling |
//!
//! The other five are graphics (`+2`, `+3`, `+6`), a plane the renderer uses
//! (`+4`), and the occupying unit index (`+5`) — which is *not* stored here,
//! because [`crate::unit::Units::at`] answers the same question from the unit
//! array and a second copy is a second thing to keep in step. `l2-kingdom` has
//! no loader and never learns what `L2_maps.dat` is; building one of these out
//! of a map file is `l2-scenario`'s job.

use crate::tables::{MOVE_COST_BLOCKED, MOVE_COST_IMPASSABLE};

/// The campaign map is 64 × 64 tiles on every shipped map.
pub const MAP_DIM: usize = 64;

/// 4,096 tiles.
pub const MAP_TILES: usize = MAP_DIM * MAP_DIM;

/// The bits of the flags byte (`+1`, plane 0). Verified in
/// `docs/formats/maps-layers.md` §2 except where noted.
pub mod flags {
    /// A road. Makes a step cost 1 instead of 3, and — the half
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
    /// cost map makes, and the only source of true impassability.
    pub const IMPASSABLE: u8 = NO_COUNTY | ROUGH;
}

/// Terrain byte (`+0`) values the rules test by name.
///
/// The observed set on a live run is `0, 1, 4, 7, 10, 11, 20, 21, 22, 23`
/// (`maps-layers.md` §5.3), and the ladders below tile it exactly: three
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
    pub const TOWN: u8 = 0x14;

    /// A dwelling plot that is actually occupied. On plot tiles this is the
    /// only value that costs anything; every other plot is impassable.
    pub const DWELLING: u8 = 0x10;

    /// A farmland tile below this is bare or ploughed and costs an ordinary 3.
    pub const FIELD_STANDING_FROM: u8 = 2;

    /// …and at this value or above it has been harvested and costs 3 again.
    /// Between the two the crop is standing and costs 6.
    pub const FIELD_STANDING_TO: u8 = 0x17;

    /// Below this a `County_DestroyField` tile is a **grain** field; at or
    /// above it, a pasture. `[D]` — `County_DestroyField` (`0x00469E5B`)
    /// branches on exactly `terrain < 0x0F`.
    pub const PASTURE_FROM: u8 = 0x0F;
}

/// Where a tile sits, as the cost map and the distance field index it:
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
    /// Byte `+7` — the county id, 1…16, or 0 for none.
    pub county: Vec<u8>,
}

impl Default for CampaignMap {
    fn default() -> Self {
        CampaignMap::empty()
    }
}

impl CampaignMap {
    /// An all-zero map. Every tile has [`flags::NO_COUNTY`] clear and county 0,
    /// which the cost map reads as passable open ground in county 0 — a blank
    /// field, which is what a test wants and what a real scenario overwrites.
    pub fn empty() -> CampaignMap {
        CampaignMap {
            terrain: vec![0; MAP_TILES],
            flags: vec![0; MAP_TILES],
            county: vec![0; MAP_TILES],
        }
    }

    /// Build from three 4,096-byte planes. `None` if any is the wrong length —
    /// a map we cannot read is not a map to half-load, which is the rule
    /// `l2-scenario` already applies to a save.
    pub fn from_planes(terrain: &[u8], flags: &[u8], county: &[u8]) -> Option<CampaignMap> {
        if terrain.len() != MAP_TILES || flags.len() != MAP_TILES || county.len() != MAP_TILES {
            return None;
        }
        Some(CampaignMap {
            terrain: terrain.to_vec(),
            flags: flags.to_vec(),
            county: county.to_vec(),
        })
    }

    pub fn terrain_at(&self, x: u8, y: u8) -> u8 {
        self.terrain[index(x, y)]
    }

    pub fn flags_at(&self, x: u8, y: u8) -> u8 {
        self.flags[index(x, y)]
    }

    pub fn county_at(&self, x: u8, y: u8) -> u8 {
        self.county[index(x, y)]
    }

    pub fn has(&self, x: u8, y: u8, bit: u8) -> bool {
        self.flags_at(x, y) & bit != 0
    }

    pub fn set_terrain(&mut self, x: u8, y: u8, value: u8) {
        self.terrain[index(x, y)] = value;
    }

    pub fn set_flags(&mut self, x: u8, y: u8, value: u8) {
        self.flags[index(x, y)] = value;
    }

    pub fn set_county(&mut self, x: u8, y: u8, value: u8) {
        self.county[index(x, y)] = value;
    }

    /// `Move_BuildCostMap` (`0x0046FF43`) — the whole 64×64 `i16` cost map,
    /// rebuilt from the three planes.
    ///
    /// **The test order is the rule**, because the bits combine: a farmland
    /// tile that is also a road is a road, and a settlement that has been
    /// ruined is impassable. The chain, in the order the original's nested
    /// `if`/`else` makes it:
    ///
    /// ```text
    /// 1. flags & 0x0C            -> 0     sea, mountain or woodland
    /// 2. county >= 17            -> 0     not a county on this map
    /// 3. flags & 0x01            -> 1     road
    /// 4. flags & 0x20            -> 3 if terrain < 2; 6 if terrain < 0x17; else 3
    /// 5. flags & 0x40            -> 100   castle site
    /// 6. flags & 0x80            -> 3 if terrain == 0x14
    ///                               0 if terrain in {3, 6, 12, 9}   (ruined)
    ///                               else 100
    /// 7. flags & 0x10            -> 100 if terrain == 0x10; else 0
    /// 8. otherwise               -> 3     open ground
    /// ```
    ///
    /// `[V]` — road 1, open 3 and field 6 agree exactly with the *stepper*,
    /// which classifies the same bits independently (`docs/armies.md` §2.2),
    /// and the field's 6 is assembled there from two separate `+3`s. `[D]` on
    /// the four 100s and the two 0s, which the stepper never has an opinion
    /// about because those tiles are never actually entered.
    ///
    /// **Nothing about units or ownership enters this.** Byte `+5` of the tile
    /// record — the occupying unit — is never read, so armies path straight
    /// through each other and through enemy stacks; blocking is resolved at
    /// step time instead. Ownership only decides what a step *does* to the
    /// tile, never whether it is cheap.
    pub fn cost_map(&self) -> CostMap {
        let mut cost = vec![0i16; MAP_TILES];
        for i in 0..MAP_TILES {
            let f = self.flags[i];
            let t = self.terrain[i];
            cost[i] = if f & flags::IMPASSABLE != 0 {
                MOVE_COST_IMPASSABLE as i16
            } else if self.county[i] > crate::county::MAX_COUNTY_ID {
                MOVE_COST_IMPASSABLE as i16
            } else if f & flags::ROAD != 0 {
                crate::tables::STEP_COST_ROAD as i16
            } else if f & flags::FARMLAND != 0 {
                if t < terrain::FIELD_STANDING_FROM || t >= terrain::FIELD_STANDING_TO {
                    crate::tables::STEP_COST_OPEN as i16
                } else {
                    (crate::tables::STEP_COST_OPEN + crate::tables::STEP_COST_FIELD_EXTRA) as i16
                }
            } else if f & flags::CASTLE != 0 {
                MOVE_COST_BLOCKED as i16
            } else if f & flags::SETTLEMENT != 0 {
                if t == terrain::TOWN {
                    crate::tables::STEP_COST_OPEN as i16
                } else if terrain::RUINED.contains(&t) {
                    MOVE_COST_IMPASSABLE as i16
                } else {
                    MOVE_COST_BLOCKED as i16
                }
            } else if f & flags::PLOT != 0 {
                if t == terrain::DWELLING {
                    MOVE_COST_BLOCKED as i16
                } else {
                    MOVE_COST_IMPASSABLE as i16
                }
            } else {
                crate::tables::STEP_COST_OPEN as i16
            };
        }
        CostMap { cost }
    }

    /// Is this tile a standing crop — the test `Unit_CrossField` makes before
    /// it destroys anything. `[D]` — terrain 2…0x16 on a farmland tile, the
    /// same window the cost map charges 6 for.
    pub fn is_standing_field(&self, x: u8, y: u8) -> bool {
        let t = self.terrain_at(x, y);
        self.has(x, y, flags::FARMLAND)
            && (terrain::FIELD_STANDING_FROM..terrain::FIELD_STANDING_TO).contains(&t)
    }
}

/// `g_moveCost` (`0x004F4080`) — a 64×64 `i16` grid, **0 meaning impassable**.
///
/// The zero is not a sentinel bolted on: the flood fill's only blocked test is
/// `cost != 0`, so "free to enter" is not representable and never needs to be.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CostMap {
    cost: Vec<i16>,
}

impl CostMap {
    /// A map where every tile costs the same. Only useful in tests; a real one
    /// comes from [`CampaignMap::cost_map`].
    pub fn uniform(cost: i16) -> CostMap {
        CostMap { cost: vec![cost; MAP_TILES] }
    }

    #[inline]
    pub fn at(&self, x: u8, y: u8) -> i16 {
        self.cost[index(x, y)]
    }

    #[inline]
    pub fn at_index(&self, i: usize) -> i16 {
        self.cost[i]
    }

    pub fn set(&mut self, x: u8, y: u8, cost: i16) {
        self.cost[index(x, y)] = cost;
    }

    /// A tile no unit can ever enter.
    #[inline]
    pub fn is_impassable(&self, i: usize) -> bool {
        self.cost[i] == MOVE_COST_IMPASSABLE as i16
    }

    pub fn as_slice(&self) -> &[i16] {
        &self.cost
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
    /// resource site is not merely dead, it is a hole in the map.
    #[test]
    fn a_ruined_resource_site_becomes_impassable() {
        for ruined in terrain::RUINED {
            assert_eq!(one_tile(flags::SETTLEMENT, ruined, 1), 0, "terrain {ruined}");
        }
        // …where the same tile before it was ruined merely cost a lot.
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
        // And a standing crop on a tile with no farmland bit is not a field.
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
        assert!(CampaignMap::from_planes(&[0; MAP_TILES], &[0; MAP_TILES], &[0; MAP_TILES]).is_some());
        assert!(CampaignMap::from_planes(&[0; 10], &[0; MAP_TILES], &[0; MAP_TILES]).is_none());
    }
}
