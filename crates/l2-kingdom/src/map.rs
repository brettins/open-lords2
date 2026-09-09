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
    ///
    /// **It is also the bare castle plot**, and that is not a collision to tidy
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
    /// `[V]` from the writer rather than from coincidence. `Castle_StampTile`
    /// (`0x0046826C`) is an `if`/`else if` ladder over the five levels writing
    /// `0x15, 0x16, 0x17, 0x18, 0x19`, and `Unit_Step`'s code-6 branch fires
    /// `Unit_ReachCastleBuilding` on exactly `0x14 < terrain < 0x1A`. This
    /// promotes `docs/hypotheses.json` H6, which had the mapping from two
    /// counties of one save and said so.
    /// **Confirmed a third time, by a branch that did not know about the first
    /// two.** `ai-lords-play` derived the same range independently: the England
    /// fixture's five *owned* counties carry a `0x17` block and the nine
    /// unowned ones carry `0x14`, and the AI's own castle-tile finder
    /// (`FUN_004A65A3`) tests exactly `0x14 < terrain < 0x1A` — a third writer
    /// and a third reader agreeing with `Castle_StampTile` and `Unit_Step`.
    /// Its duplicate constant is gone; this is the one.
    pub const CASTLE_FROM: u8 = 0x15;
    pub const CASTLE_TO: u8 = 0x19;

    /// The castle standing on a tile, as a castle **type** 0..=5, or `None` if
    /// this terrain is not a castle plot at all.
    pub fn castle_type(terrain: u8) -> Option<u8> {
        match terrain {
            CASTLE_PLOT => Some(0),
            CASTLE_FROM..=CASTLE_TO => Some(terrain - CASTLE_PLOT),
            _ => None,
        }
    }

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

// ---------------------------------------------------------------------------
// The castle plot
// ---------------------------------------------------------------------------

/// **Where a county's castle stands** — the 2×2 block on plane-0 bit `0x80`
/// whose terrain is [`terrain::CASTLE_PLOT`] or a castle above it.
///
/// `County_FindCastleTile` (`0x00468121`) scans for it once at load and stores
/// the corner in county `+0x74`/`+0x75`; we scan, the way every other
/// settlement lookup in this workspace does, because the alternative is a
/// cached tile index that a loader has to remember to fill and that nothing
/// notices is zero. Tiles come back in index order, so the first is the
/// original's `+0x74`/`+0x75` corner.
pub fn castle_tiles(map: &CampaignMap, county: u8) -> Vec<usize> {
    (0..MAP_TILES)
        .filter(|&i| {
            map.county[i] == county
                && map.flags[i] & flags::SETTLEMENT != 0
                && terrain::castle_type(map.terrain[i]).is_some()
        })
        .collect()
}

/// The corner of [`castle_tiles`] — county `+0x74`/`+0x75`, the tile an army
/// garrisoning the castle is teleported onto.
pub fn castle_tile(map: &CampaignMap, county: u8) -> Option<usize> {
    castle_tiles(map, county).first().copied()
}

/// The plane-1 byte [`castle_stamp`] writes. Bank index
/// `(0x10 & 0x1C) >> 2 == 4`, which is `Castle1a.pl8` / `Castle2a.pl8`.
///
/// Bit `0` is set by `Map_StampBlock` on every tile it touches and nothing we
/// have read consumes it; it is carried rather than dropped.
pub const CASTLE_BANK_BYTE: u8 = 0x11;

/// **What a castle looks like** — `Castle_StampTile` (`0x0046826C`), which is
/// the reason a castle is missing from our campaign map entirely.
///
/// ```c
/// if (castleDegraded == 0) frame = level*4 + 0x50;   /* finished */
/// else if (percent < 0x32) frame = level*4 + 0x28;   /* scaffolding */
/// else                     frame = level*4 + 0x3C;   /* half-built */
/// content = 0x15 + level;
/// Map_StampBlock(frame, 2, county.castleTile, 0x10, content);
/// ```
///
/// Three appearances per level, twenty frames apart, and **the map file does
/// not hold any of them**: unlike the mine, the quarry and the forest — which
/// `L2_maps.dat` stores as real artwork that `County_PlaceResourceSites` merely
/// flags — the castle plot is plain ground in the base bank and every castle on
/// the screen is stamped in at run time. It is re-stamped **every season** by
/// `Castle_BuildTick`, which is why it lives beside the build rules rather than
/// in a load-time pass.
///
/// `frames` are in the block's own order — north-west, north-east, south-west,
/// south-east — with the quadrant offsets `[0, 2, 1, 3]` already added.
/// `Map_StampBlock`'s size-2 quadrant table was read out of `Lords2.exe` at
/// `0x004D80E0`. `[V]`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CastleStamp {
    pub bank: u8,
    pub frames: [u8; 4],
    pub terrain: u8,
}

/// The quadrant offsets `Map_StampBlock` adds for a 2×2 stamp — `0x004D80E0`,
/// read from the binary. North-west, north-east, south-west, south-east.
pub const BLOCK_QUADRANTS_2: [u8; 4] = [0, 2, 1, 3];

/// A finished castle's base frame, by level: `level * 4 + 0x50`.
pub const CASTLE_FRAME_BUILT: u8 = 0x50;
/// Under way and less than half done: `level * 4 + 0x28`.
pub const CASTLE_FRAME_SCAFFOLD: u8 = 0x28;
/// Under way and half done or more: `level * 4 + 0x3C`.
pub const CASTLE_FRAME_HALF: u8 = 0x3C;

/// The percentage at which the scaffolding becomes a half-built castle.
pub const CASTLE_HALF_PERCENT: u8 = 0x32;

/// **The simulation's half of `Castle_StampTile`** — write the castle's
/// `content` byte onto its 2×2 block.
///
/// The frame and the bank are the renderer's, but the content byte is a *rule*:
/// `Unit_TryEnterTile` masks the settlement bit off at
/// [`terrain::CASTLE_PLOT`] and leaves it set above it, so this write is what
/// turns a tile an army walks over into a castle it has to garrison or besiege.
/// Ordering a castle stamps the new level at once — the scaffolding is already
/// an obstacle — which is the original's ordering in `Castle_Order`.
///
/// Returns the tiles written, so a caller can tell whether the county had a
/// plot at all.
pub fn stamp_castle_terrain(map: &mut CampaignMap, county: u8, castle_type: u8) -> usize {
    let Some(t) = terrain::CASTLE_PLOT.checked_add(castle_type.min(5)) else { return 0 };
    let tiles = castle_tiles(map, county);
    for i in &tiles {
        map.terrain[*i] = t;
    }
    tiles.len()
}

/// See [`CastleStamp`]. `castle_type` is 1..=5; type 0 has nothing to draw and
/// answers `None`, which leaves the bare plot the map file already holds.
pub fn castle_stamp(castle_type: u8, castle_degraded: u8, castle_percent: u8) -> Option<CastleStamp> {
    if castle_type == 0 || castle_type > 5 {
        return None;
    }
    let level = castle_type - 1;
    let base = if castle_degraded == 0 {
        CASTLE_FRAME_BUILT
    } else if castle_percent < CASTLE_HALF_PERCENT {
        CASTLE_FRAME_SCAFFOLD
    } else {
        CASTLE_FRAME_HALF
    };
    let base = base + level * 4;
    Some(CastleStamp {
        bank: CASTLE_BANK_BYTE,
        frames: core::array::from_fn(|q| base + BLOCK_QUADRANTS_2[q]),
        terrain: terrain::CASTLE_PLOT + castle_type,
    })
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
