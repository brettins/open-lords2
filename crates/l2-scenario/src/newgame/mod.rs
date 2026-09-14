//! **`Map_InitScenario` (`0x004676E0`)** — a world built out of an
//! `L2_maps.dat` slot.
//!
//! # The second constructor
//!
//! [`crate::Scenario::from_save`] reads a position the original had already
//! built. This module *builds* one, from the six 64 × 64 planes of a map slot
//! and nothing else, and hands back the same [`crate::Scenario`] — so
//! [`crate::Scenario::starting_kingdom`] does not learn which of the two it
//! got, and `crates/l2-scenario/tests/newgame.rs` can diff them field by field.
//!
//! # What the original does, in its own order
//!
//! `Game_NewGame` (`0x00497CED`) is the whole new-game path. Six of its calls
//! build the world; the rest set the clock, hand out lords, and start the first
//! season. **[V]** for the call list — it is one straight-line function.
//!
//! | address | call | what it does | here |
//! |---|---|---|---|
//! | `0x0046C5B8` ×6 | `FUN_0046C5B8(n)` | zero all eight bytes of every tile record | [`Tiles::blank`] |
//! | `0x00467770` | `Map_LoadPlanes` | the six planes → the tile array; `g_countyCount`; the player-start table; the merchant routes | [`Tiles::from_slot`] |
//! | `0x0046DF51` | — | clears bank bit `0x20` on every tile | graphics only, §"What is not here" |
//! | `0x0046DBF5`, `0x0046DD34` | — | randomise the coast and sea variants | graphics only |
//! | `0x0046A037` | `Minimap_Load` | reads `MAPnn.PL8` | `l2-view`'s |
//! | `0x00467CA6` | — | the county adjacency lists | [`adjacency`] |
//! | `0x00468D4F` | `Counties_PlaceSites` | town, dwelling plots, resource sites, blacksmith, castle plot | [`place_sites`] |
//! | `0x00467A36` | `Map_PlaceStartingFields` | every farm tile's opening crop, by difficulty | [`place_starting_fields`] |
//! | `0x0046DA4B` | `County_CollectFieldTiles` | the twenty field tiles per county — **and it razes the overflow** | [`collect_field_tiles`] |
//! | `0x004291B3` | `Merchant_PickStartCounties` | one start county per trade route | [`pick_merchant_starts`] |
//!
//! and then, outside `Map_InitScenario`:
//!
//! | `0x00451150` | `County_Reset` | every county's opening economy | [`county_reset`] |
//! | `0x00427ED0` | `Merchant_SpawnAll` | six merchants on six roads | [`spawn_merchants`] |
//! | `0x0049BC5F` | `PlayerStart_Compact` | drop the starts above the lord count | [`start_counties`] |
//! | `0x0049BD99` | `Game_SetupRealmsAndCounties` | seats the realms; the stores and the treasury | **split** — see below |
//!
//! # What is deliberately *not* here
//!
//! * **The artwork.** Three of `Map_InitScenario`'s calls only permute frame
//! indices — the coast and sea variants, and the bank's draw bits. The tile
//!   record's `bank` and `frame` bytes *are* carried, because two rules read
//!   them ([`place_resource_sites`] keys on the Town bank and on frames 0, 20
//!   and 30, and [`collect_field_tiles`] writes frame 6 over a razed field),
//!   but nothing here randomises a variant and nothing here draws.
//! * **The built castle.** `County_FindCastleTile` stamps terrain `0x14`, the
//!   bare plot, and that is load-time and is done here. Raising an actual
//!   castle on it is `FUN_0046826C`, which is keyed on the castle's *level* and
//!   its build percentage and is re-stamped every season — so it belongs beside
//!   the castle rules and is not this module's.
//! * **The stores, the treasury and the armoury.** `Game_SetupRealmsAndCounties`
//!   does two jobs: it *seats* the realms from the player-start table, which
//!   needs the map and is done here, and it applies the twelve custom-game
//!   options, which is `l2_game::setup::Settings::apply_to` and already exists.
//!   The split is the same one the original makes between `County_Reset`'s
//! defaults and the option rows that overwrite them.
//! * **The starting garrison.** `Army_Create` at setup is
//! `Settings::unhonoured`'s.

mod reset;
pub use reset::*;
mod placement;
pub use placement::*;
mod lords;
pub use lords::*;
mod scenario;
pub use scenario::*;

use l2_formats::maps::{MapSlot, Plane, PLANE_DIM};
use l2_kingdom::county::{MAX_COUNTIES, MAX_COUNTY_ID, MAX_FIELDS, MAX_NEIGHBOURS};
use l2_kingdom::map::{CampaignMap, MAP_TILES};
use l2_kingdom::merchant::{self, MerchantRoutes, ROUTES, ROUTE_SLOTS};
use l2_kingdom::mercenary::MercenaryBands;
use l2_kingdom::realm::MAX_REALMS;
use l2_kingdom::tables::{Weather, JOB_COUNT};
use l2_kingdom::unit::{Unit, UnitKind, Units};
use l2_kingdom::Options;

use crate::{Clock, CountyState, IndustryState, RealmState, Scenario};

// ------------------------------------------------------------------ constants

/// Plane-0 bits, under the names `docs/decisions.md` C25 settled.
mod bit {
    /// The county town, a 2×2 block, one per county.
    pub const TOWN: u8 = 0x40;
    /// The castle plot **or** an industry site.
    pub const SITE: u8 = 0x80;
    /// A reserved dwelling plot; exactly four per county.
    pub const PLOT: u8 = 0x10;
    /// Farmland.
    pub const FARM: u8 = 0x20;
    /// The county boundary. `FUN_0046C147` masks it off before classifying a
/// neighbour, so it can never make a tile stop being farmland.
    pub const BOUNDARY: u8 = 0x02;
}

/// `bank & 0x1C` — the tile-set selector. Only `TOWN_BANK` is read by a rule.
const BANK_LAYER: u8 = 0x1C;
const BANK_MTNS: u8 = 0x04;
const BANK_ROADS: u8 = 0x08;
const BANK_TOWN: u8 = 0x0C;
/// The building-overlay draw bit `Map_RenderIso` tests. Graphics.
const BANK_OVERLAY: u8 = 0x80;

/// `Town1?.pl8` frames the *file* stores on an industry site, and what each
/// one is. `County_PlaceResourceSites` (`0x00468E61`) reads exactly these three.
const FRAME_QUARRY: u8 = 0;
const FRAME_FOREST: u8 = 20;
const FRAME_MINE: u8 = 30;

/// The four industry records, in the order `l2-kingdom` holds them —
/// **wood, iron, weapons, stone**, which is `Industry_ToggleFromMap`'s
/// numbering.
const IND_WOOD: usize = 0;
const IND_IRON: usize = 1;
const IND_WEAPONS: usize = 2;
const IND_STONE: usize = 3;

/// The terrain byte each intact industry site carries — `maps-layers.md` §5.4's
/// idle rung of each triple.
const TERRAIN_IRON: u8 = 1;
const TERRAIN_STONE: u8 = 4;
const TERRAIN_WEAPONS: u8 = 7;
const TERRAIN_WOOD: u8 = 10;
/// The bare castle plot `County_FindCastleTile` stamps.
const TERRAIN_CASTLE_PLOT: u8 = 0x14;

/// The 2×2 village `County_FindTownTile` stamps over the file's frames 0…3.
/// `docs/formats/maps-layers.md` §5.4 and `docs/decisions.md` C41: the three
/// groups 47–50, 51–54 and 55–58 are the three village sizes and the
/// population pass re-stamps them, so this is only the smallest.
const FRAME_VILLAGE_SMALL: u8 = 47;

/// The frame a razed field falls back to — `County_CollectFieldTiles` writes a
/// literal 6, the first grass frame of the base bank.
const FRAME_GRASS: u8 = 6;

/// The order `FUN_0046AC22` lays a 2×2 object's frames out in, as offsets from
/// the group's base frame for tiles NW, NE, SW, SE.
///
/// **`[D]`, and nothing in the simulation reads it.** `maps-layers.md` §5.1
/// observed a live run stamping frames 88, 90, 89, 91 onto plane-3 parts
/// 0, 1, 2, 3, which is this permutation. It is carried so the frame plane this
/// module produces is the one the game would produce; no rule here or in
/// `l2-kingdom` tests a frame in the 47…58 range.
const ISO_2X2: [u8; 4] = [0, 2, 1, 3];

/// `g_lordChoice` (`0x004DC17C`) — four scenario groups of five colour slots of
/// four candidate lord ids.
///
/// **`[V]`, read out of `Lords2.exe` at `0x004DC17C` with `tools/maps/pe.js`.**
/// `Realms_AssignLords` (`0x0049CAAA`) indexes it
/// `0x004DC17C + (g_scenarioIndex & 3) * 0x14 + realm.shieldIndex * 4 + n` and
/// takes the first candidate no other realm has taken. Held flat because the
/// index arithmetic runs off the end of its own group: `shieldIndex` is 1…5, so
/// slot 5 of group *g* is slot 0 of group *g+1*, and reproducing that needs one
/// array.
///
/// **Eighty-four bytes, not eighty, and the four extra are the overrun's own.**
/// Group 3's colour slot 5 lands at offset 80, one row past a 4 × 5 × 4 table,
/// and the four bytes there are `1, 3, 4, 2` — a well-formed row of the four
/// lord ids, immediately before `g_realmColour` at `0x004DC1CE`. Whether the
/// author declared twenty-one rows or twenty and got lucky is not settled; what
/// is settled is which bytes the code reads, and those are these.
const LORD_CHOICE: [u8; 84] = [
    0, 0, 0, 0, 2, 1, 3, 4, 1, 3, 4, 2, 2, 1, 3, 4, 4, 3, 2, 1, //
    3, 2, 4, 1, 1, 3, 4, 2, 4, 1, 3, 2, 2, 1, 3, 4, 4, 2, 3, 1, //
    1, 3, 2, 4, 1, 3, 4, 2, 4, 1, 2, 3, 3, 2, 1, 4, 1, 4, 2, 3, //
    2, 1, 3, 4, 3, 2, 4, 1, 2, 1, 3, 4, 3, 1, 2, 4, 4, 3, 2, 1, //
    1, 3, 4, 2,
// --------------------------------------------------------------- the failures

/// A map slot that cannot be made into a world.
///
/// Each of these is a refusal, for the reason
/// [`crate::Scenario::from_save`]'s are: a map we have misread is not a map to
/// half-load.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MapError {
    /// The county plane holds no id in `1..=16` — an empty slot, or one this
    /// code cannot read.
    NoCounties,
    /// More counties than `g_counties` holds.
    CountyCount(usize),
    /// The map carries no player-start marker at all, so no realm can be
    /// seated. Every one of the 44 shipped maps carries at least two.
    NoPlayerStarts,
    /// A game asking for more lords than the map has castles for.
    /// `FUN_004335F0` refuses to press *Start* at all in that case; reaching
    /// here means the guard was skipped.
    TooManyLords { lords: usize, seats: usize },
    /// A start marker naming a county that is not on this map.
    StartCounty { marker: usize, county: u8 },
    /// `g_localPlayer` outside `1..=5`, or above the lord count.
    LocalPlayer(u8),
    /// A chosen shield outside `1..=5`.
    ///
/// **A refusal**, and deliberately the
    /// opposite of what [`crate::Scenario::from_save`] does with the realm
    /// colour byte it reads out of a file. That byte is somebody else's and a
    /// clamp there would hide a misread offset; this one is *ours*, chosen on a
    /// screen that can only produce 1 … 5,
    /// carry we got wrong and quietly turning it into blue would hide exactly
    /// the class of defect this field exists to fix.
    Shield(u8),
}

impl core::fmt::Display for MapError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            MapError::NoCounties => write!(f, "the county plane is empty"),
            MapError::CountyCount(n) => {
                write!(f, "{n} counties, and the array holds 1..={MAX_COUNTY_ID}")
            }
            MapError::NoPlayerStarts => write!(f, "the map has no player start"),
            MapError::TooManyLords { lords, seats } => {
                write!(f, "{lords} lords on a map that seats {seats}")
            }
            MapError::StartCounty { marker, county } => {
                write!(f, "start {marker} names county {county}, which is not on this map")
            }
            MapError::LocalPlayer(p) => write!(f, "g_localPlayer is {p}"),
            MapError::Shield(s) => write!(f, "shield {s}, and the five colours are 1..=5"),
        }
    }
}

impl std::error::Error for MapError {}

// ------------------------------------------------------------- what to build

/// The choices a map slot cannot answer for itself.
///
/// Everything here is a *setting*, and every one of them is read by
/// `Map_InitScenario` or by the seating pass — so the world builder
/// needs them and why none of them is a map fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NewGame {
    /// `g_scenarioIndex`. The **only** thing the world builder reads it for is
    /// `Realms_AssignLords`' `g_scenarioIndex & 3`, which picks the lord
    /// group; the planes are handed in already.
    pub slot: usize,
    /// The six rule flags a game runs on. `options.difficulty` is
    /// `g_optDifficulty`, which is what [`place_starting_fields`] reads.
    pub options: Options,
    /// How many realms have a lord — one human plus `ai_lords`, which is
    /// `Setup_CommitOptions`' *Nobles* minus the number of people. Realms
    /// `1..=lords` are in play and realms above it are not.
    pub lords: usize,
    /// `g_localPlayer`.
    pub local_player: u8,
    /// **The colour the person picked on setup page 4**, 1 … 5 — red, yellow,
    /// black, magenta, blue.
    ///
    /// The original keeps it twice: `g_playerSlots + realm * 0x2C + 0x25` (the record
    /// `g_playerNames` is the `+0x04` of, so four bytes lower than the name),
    /// which is what `Realms_AssignLords` reads, and `g_realms[p].shieldIndex`,
    /// which is what everything that *draws* reads. `FUN_00432FAB`
    /// (`0x00432FAB`) writes both from the clicked shield and `FUN_004978AD`
    ///
    /// **It is the human's choice and it moves every AI**, because
    /// [`assign_lords`] hands the AIs the shields the humans left and then
    /// picks each AI's *lord from its shield*. `docs/rules.md` §7a.
    pub shield: u8,
    /// The seed [`shuffle_starts`] draws from.
    ///
    /// **Which realm gets which start county is rolled**, and this is the
    /// roll. `docs/environment.md` records it as an observed fact — two
    /// independently created England turn-one saves disagree about it, so the
    /// fixture's fingerprint deliberately excludes it — and `FUN_00497E65` is
/// the code. Passing the seed in
    /// generator keeps [`Scenario::from_map`] a pure function of its inputs,
    /// which is what lets two lockstep peers build the same world from the same
    /// lobby settings.
    pub seed: u64,
}

impl Default for NewGame {
    fn default() -> NewGame {
        NewGame {
            slot: 0,
            options: Options::default(),
            lords: 5,
            local_player: 1,
            // `FUN_004978AD`'s seed: shield 1, red. A default game is the one
            // §7a's first row describes.
            shield: 1,
            seed: 0,
        }
    }
}

// ------------------------------------------------------ the runtime tile array

/// `g_tiles` (`0x00522F90`) — 4,096 eight-byte records, `y * 64 + x`, as seven
/// planes.
///
/// The eighth byte, `+5`, is the plane-4 marker at load and the occupying unit
/// afterwards; it is consumed by `Map_LoadPlanes` and never read again, so it
/// is not kept. `docs/formats/maps-layers.md` §5.3 has the record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tiles {
    /// `+0` — what stands on the tile. All zero until the site passes run.
    pub content: Vec<u8>,
    /// `+1` — the plane-0 flag bits, then whatever the load-time passes make
    /// of them (the blacksmith *adds* [`bit::SITE`]; a razed field loses all
    /// of them).
    pub flags: Vec<u8>,
    /// `+2` — the bank selector in bits `0x1C` plus the run-time draw bits.
    pub bank: Vec<u8>,
    /// `+3` — the frame index inside the bank.
    pub frame: Vec<u8>,
    /// `+4` — plane 3, the part index inside a multi-tile object, verbatim.
    pub part: Vec<u8>,
    /// `+6` — the terrain frame saved before a building went on the tile, so
    /// razing it can put the ground back.
    pub saved_frame: Vec<u8>,
    /// `+7` — plane 5, the county id, verbatim.
    pub county: Vec<u8>,
}

impl Tiles {
    /// `FUN_0046C5B8`, six times: every byte of every record zeroed.
    pub fn blank() -> Tiles {
        Tiles {
            content: vec![0; MAP_TILES],
            flags: vec![0; MAP_TILES],
            bank: vec![0; MAP_TILES],
            frame: vec![0; MAP_TILES],
            part: vec![0; MAP_TILES],
            saved_frame: vec![0; MAP_TILES],
            county: vec![0; MAP_TILES],
        }
    }

    /// `Map_LoadPlanes` (`0x00467770`)'s copy half: five of the six planes
    /// straight into the record, in the same `y` outer / `x` inner nest.
    ///
    /// Plane 4 is *not* copied — it is dispatched and dropped, which
    /// [`load_planes`] does because both of its destinations are tables rather
    /// than tiles.
    pub fn from_slot(slot: &MapSlot<'_>) -> Tiles {
        let mut t = Tiles::blank();
        for y in 0..PLANE_DIM {
            for x in 0..PLANE_DIM {
                let i = y * PLANE_DIM + x;
                t.flags[i] = slot.at(Plane::Flags, x, y);
                t.bank[i] = slot.at(Plane::GfxBank, x, y);
                t.frame[i] = slot.at(Plane::GfxIndex, x, y);
                t.part[i] = slot.at(Plane::ObjectPart, x, y);
                t.county[i] = slot.at(Plane::County, x, y);
            }
        }
        t
    }

    /// The four planes the simulation reads.
    pub fn campaign_map(&self) -> CampaignMap {
        CampaignMap::from_planes(&self.content, &self.flags, &self.bank, &self.county)
            .expect("Tiles holds MAP_TILES of each")
    }

    fn bank_layer(&self, tile: usize) -> u8 {
        self.bank[tile] & BANK_LAYER
    }

    /// `FUN_0046AC22(frameBase, 2, tile, layerBit, content)` — stamp a 2×2
/// object. The bank is rebuilt: `(bank | 1) & 0xE3 | bit`.
    fn stamp_2x2(&mut self, tile: usize, frame_base: u8, bank_bits: u8, content: u8) {
        for (n, offset) in [0usize, 1, PLANE_DIM, PLANE_DIM + 1].into_iter().enumerate() {
            let Some(t) = tile.checked_add(offset).filter(|t| *t < MAP_TILES) else { continue };
            self.bank[t] = ((self.bank[t] | 1) & 0xE3) | bank_bits;
            self.frame[t] = frame_base.wrapping_add(ISO_2X2[n]);
            self.content[t] = content;
        }
    }
}

// --------------------------------------------------------- the map's own facts

/// Everything the six planes and the load-time passes say, before any option
/// touches it.
///
/// Public because it is the *only* place a caller can see the town tile, the
/// castle plot, the dwelling plots and the industry site tiles: none of them
/// has a home in [`l2_kingdom::county::County`] yet, and inventing four fields
/// there to hold numbers no rule reads would be worse than saying so here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapWorld {
    pub tiles: Tiles,
    pub county_count: usize,
    /// `g_playerStartTable` — the county each start marker names, indexed by
    /// the marker `1..=5`. Index 0 is never a start.
    pub player_start: [u8; 6],
    /// `g_playerStartCount`.
    pub player_start_count: usize,
    /// `g_merchantRoutes` — six rows of sixteen county ids.
    pub routes: MerchantRoutes,
    /// `g_merchantStartCounty`.
    pub merchant_start: [u8; ROUTES],
    /// County `+0x5C` — the adjacency lists, ascending.
    pub neighbours: Vec<Vec<u8>>,
    /// County `+0x70` and `+0x6C`/`+0x6D` — the town tile and the county
    /// anchor, which is the **south-east** quadrant of the town block.
    pub town_tile: Vec<usize>,
    pub anchor: Vec<(u8, u8)>,
    /// County `+0x78` and `+0x74`/`+0x75` — the castle plot and its
    /// **north-west** quadrant.
    pub castle_tile: Vec<usize>,
    pub castle: Vec<(u8, u8)>,
    /// County `+0x80` — the four dwelling plots, as tile indices.
    pub dwelling_plots: Vec<[usize; 4]>,
    /// County `+0x298 + c*0x18` — each industry's site tile, as a tile index.
    pub industry_site: Vec<[usize; 4]>,
    /// County `+0x295 + c*0x18`.
    pub has_resource: Vec<[bool; 4]>,
    /// `g_countyFieldTiles` — the twenty field tiles per county, as tile
    /// indices, 0 for an empty slot.
    pub field_tiles: Vec<[u16; MAX_FIELDS]>,
    /// County `+0x205` — how many farm tiles the county had **before** the
    /// twenty-slot table razed the overflow. Carried because it is the bound
    /// on the reclamation cursor and nothing else in the workspace has it.
    pub farm_tile_count: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use l2_formats::maps::{MapSet, PLANE_LEN, SLOT_LEN};
    use l2_kingdom::map::index;

    /// `plane1 == 0` — the base tile set.
    const BANK_BASE: u8 = 0x00;

    /// A one-slot `L2_maps.dat` built by hand: one county, one 2×2 town, one
    /// 2×2 castle plot, one mine, one field, one plot.
    fn one_county_slot() -> Vec<u8> {
        let mut buf = vec![0u8; SLOT_LEN];
        let put = |buf: &mut Vec<u8>, plane: Plane, x: usize, y: usize, v: u8| {
            buf[plane as usize * PLANE_LEN + y * PLANE_DIM + x] = v;
        };
        // The whole map is county 1 except where stated, so the "no county"
        // bit never has to be set.
        for y in 0..PLANE_DIM {
            for x in 0..PLANE_DIM {
                put(&mut buf, Plane::County, x, y, 1);
            }
        }
        // The county town at (10, 10), 2x2, with a merchant-route marker.
        for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            put(&mut buf, Plane::Flags, 10 + dx, 10 + dy, bit::TOWN);
            put(&mut buf, Plane::GfxBank, 10 + dx, 10 + dy, BANK_TOWN);
        }
        put(&mut buf, Plane::Marker, 10, 10, 1);
        // The castle plot at (20, 20), 2x2 on grass, carrying start marker 1.
        for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            put(&mut buf, Plane::Flags, 20 + dx, 20 + dy, bit::SITE);
            put(&mut buf, Plane::GfxBank, 20 + dx, 20 + dy, BANK_BASE);
            put(&mut buf, Plane::GfxIndex, 20 + dx, 20 + dy, 6);
        }
        put(&mut buf, Plane::Marker, 20, 20, 1);
        // An iron mine: a Town-bank tile drawing frame 30, with the site bit.
        put(&mut buf, Plane::Flags, 30, 30, bit::SITE);
        put(&mut buf, Plane::GfxBank, 30, 30, BANK_TOWN);
        put(&mut buf, Plane::GfxIndex, 30, 30, FRAME_MINE);
        // Four dwelling plots.
        for i in 0..4 {
            put(&mut buf, Plane::Flags, 40 + i, 40, bit::PLOT);
            put(&mut buf, Plane::GfxIndex, 40 + i, 40, 7 + i as u8);
        }
        // Two farm fields, side by side,
        // blacksmith.
        for i in 0..2 {
            put(&mut buf, Plane::Flags, 12 + i, 12, bit::FARM);
            put(&mut buf, Plane::GfxBank, 12 + i, 12, BANK_ROADS);
            put(&mut buf, Plane::GfxIndex, 12 + i, 12, 80);
        }
        buf
    }

    fn world() -> MapWorld {
        let buf = one_county_slot();
        let set = MapSet::parse(&buf).unwrap();
        let slot = set.slot(0).unwrap();
        build(&slot, &NewGame::default()).unwrap()
    }

    #[test]
    fn the_town_gives_the_anchor_and_the_castle_gives_the_plot() {
        let w = world();
        assert_eq!(w.county_count, 1);
        // The anchor is the *fourth* town tile — the south-east one.
        assert_eq!(w.anchor[1], (11, 11));
        assert_eq!(w.town_tile[1], index(10, 10));
        // The castle plot is the *first* — the north-west one — and all four
        // carry the bare-plot terrain.
        assert_eq!(w.castle[1], (20, 20));
        for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            assert_eq!(w.tiles.content[index(20 + dx, 20 + dy)], TERRAIN_CASTLE_PLOT);
        }
    }

    #[test]
    fn the_mine_is_read_off_the_map_and_the_blacksmith_is_derived() {
        let w = world();
        assert!(w.has_resource[1][IND_IRON], "frame 30 on the Town bank is iron");
        assert_eq!(w.tiles.content[index(30, 30)], TERRAIN_IRON);
        // No quarry frame anywhere, and the iron guard would have blocked one
        // anyway.
        assert!(!w.has_resource[1][IND_STONE]);
        assert!(!w.has_resource[1][IND_WOOD]);
        // Every county gets a blacksmith, and it is on a tile the file gave no
        // flags at all.
        assert!(w.has_resource[1][IND_WEAPONS]);
        let smith = w.industry_site[1][IND_WEAPONS];
        assert_eq!(w.tiles.content[smith], TERRAIN_WEAPONS);
        assert_ne!(w.tiles.flags[smith] & bit::SITE, 0);
    }

    #[test]
    fn the_dwelling_plots_keep_the_ground_they_covered() {
        let w = world();
        assert_eq!(
            w.dwelling_plots[1],
            [index(40, 40), index(41, 40), index(42, 40), index(43, 40)]
        );
        for i in 0..4usize {
            let t = index(40 + i as u8, 40);
            assert_eq!(w.tiles.saved_frame[t], 7 + i as u8, "the terrain frame is remembered");
            assert_eq!(w.tiles.content[t], 0);
        }
    }

    #[test]
    fn the_two_fields_become_pasture_and_land_in_the_field_table() {
        let w = world();
        // Difficulty 0, fewer than eight fields: both are pasture.
        for i in 0..2u8 {
            assert_eq!(w.tiles.content[index(12 + i, 12)], 0x14);
            assert_eq!(w.tiles.frame[index(12 + i, 12)] & !3, 104);
        }
        assert_eq!(w.field_tiles[1][0], index(12, 12) as u16);
        assert_eq!(w.field_tiles[1][1], index(13, 12) as u16);
        assert_eq!(w.field_tiles[1][2], 0);
        assert_eq!(w.farm_tile_count[1], 2);
    }

    /// The rule that only exists because the table is twenty slots wide.
    #[test]
    fn a_twenty_first_field_is_razed_rather_than_dropped() {
        let mut buf = one_county_slot();
        for i in 0..25usize {
            let (x, y) = (i % 5 + 20, i / 5 + 40);
            buf[Plane::Flags as usize * PLANE_LEN + y * PLANE_DIM + x] = bit::FARM;
            buf[Plane::GfxBank as usize * PLANE_LEN + y * PLANE_DIM + x] = BANK_ROADS;
            buf[Plane::GfxIndex as usize * PLANE_LEN + y * PLANE_DIM + x] = 80;
        }
        let set = MapSet::parse(&buf).unwrap();
        let w = build(&set.slot(0).unwrap(), &NewGame::default()).unwrap();
        assert_eq!(w.farm_tile_count[1], 27, "two originals plus twenty-five");
        let used = w.field_tiles[1].iter().filter(|&&t| t != 0).count();
        assert_eq!(used, MAX_FIELDS, "the table fills");
        // Seven tiles were farmland in the file and are plain grass now.
        let razed = (0..MAP_TILES)
            .filter(|&t| w.tiles.frame[t] == FRAME_GRASS && w.tiles.flags[t] == 0)
            .filter(|&t| {
                let (x, y) = (t % PLANE_DIM, t / PLANE_DIM);
                (20..25).contains(&x) && (40..45).contains(&y)
            })
            .count();
        assert_eq!(razed, 7, "27 fields, 20 slots");
    }

    #[test]
    fn the_difficulty_ladder_is_the_whole_of_its_effect_on_the_land() {
        // Sixteen fields in one county, so every rung is reached.
        let mut buf = one_county_slot();
        for i in 0..16usize {
            let (x, y) = (i + 20, 50);
            buf[Plane::Flags as usize * PLANE_LEN + y * PLANE_DIM + x] = bit::FARM;
            buf[Plane::GfxBank as usize * PLANE_LEN + y * PLANE_DIM + x] = BANK_ROADS;
            buf[Plane::GfxIndex as usize * PLANE_LEN + y * PLANE_DIM + x] = 80;
        }
        let set = MapSet::parse(&buf).unwrap();
        let slot = set.slot(0).unwrap();
        // The two originals at (12, 12) and (13, 12) come first in row-major
        // order, so the sixteen at y = 50 are ordinals 2..18.
        let counts = |difficulty: u8| {
            let mut setup = NewGame::default();
            setup.options.difficulty = difficulty;
            let w = build(&slot, &setup).unwrap();
            let mut pasture = 0;
            let mut fallow = 0;
            let mut wild = 0;
            for t in w.field_tiles[1].iter().filter(|&&t| t != 0) {
                match w.tiles.content[*t as usize] {
                    0x14 => pasture += 1,
                    1 => fallow += 1,
                    0 => wild += 1,
                    other => panic!("terrain {other}"),
                }
            }
            (pasture, fallow, wild)
        };
        // Eighteen fields in the county: the two originals plus the sixteen.
        assert_eq!(counts(0), (8, 10, 0), "easiest: eight pasture, the rest fallow");
        assert_eq!(counts(1), (6, 2, 10));
        assert_eq!(counts(2), (4, 2, 12));
        assert_eq!(counts(3), (4, 0, 14), "hardest: four pasture and nothing else sown");
    }

    #[test]
    fn a_map_with_no_county_is_refused_rather_than_half_loaded() {
        let buf = vec![0u8; SLOT_LEN];
        let set = MapSet::parse(&buf).unwrap();
        assert_eq!(build(&set.slot(0).unwrap(), &NewGame::default()), Err(MapError::NoCounties));
    }

    #[test]
    fn more_lords_than_the_map_seats_is_refused() {
        let w = world();
        let setup = NewGame { lords: 3, ..NewGame::default() };
        assert_eq!(
            Scenario::from_map_world(&w, &setup),
            Err(MapError::TooManyLords { lords: 3, seats: 1 })
        );
    }

    #[test]
    fn the_start_county_is_owned_switched_on_and_building() {
        let w = world();
        let setup = NewGame { lords: 1, ..NewGame::default() };
        let s = Scenario::from_map_world(&w, &setup).unwrap();
        let c = s.counties[1].as_ref().unwrap();
        assert_eq!(c.owner, 1);
        assert!(c.castle_switch, "the castle switch opens on");
        assert!(c.industry[IND_IRON].enabled, "the first non-weapons resource is switched on");
        assert!(!c.industry[IND_WEAPONS].enabled, "record 2 is skipped");
        assert!(c.industry[IND_WEAPONS].has_resource, "…but the blacksmith is still there");
        assert_eq!(s.clock, Clock { season: 3, season_next: 4, year: 1267, turn_count: 0 });
    }

    /// The lord table is real data and each realm gets a different lord —
    /// **whichever of the five colours the person takes**.
    #[test]
    fn the_lords_are_distinct_for_every_scenario_group_and_every_colour() {
        for slot in 0..8usize {
            for shield in 1..=5u8 {
                let a = assign_lords(&NewGame { slot, shield, ..NewGame::default() }, 5);
                let given: Vec<u8> = (1..=5).filter(|&r| r != 1).map(|r| a.lord[r]).collect();
                assert!(
                    given.iter().all(|&l| l != 0),
                    "slot {slot} shield {shield} left a realm lordless"
                );
                for (i, x) in given.iter().enumerate() {
                    for y in given.iter().skip(i + 1) {
                        assert_ne!(x, y, "slot {slot} shield {shield} gave one lord twice");
                    }
                }
                assert_eq!(a.lord[1], 0, "the person has no AI lord");
                // And five realms fly five different colours, with the person
                // on the one they asked for.
                assert_eq!(a.shield[1], shield, "the person did not get the colour they picked");
                let mut flown = (1..=5).map(|r| a.shield[r]).collect::<Vec<_>>();
                flown.sort_unstable();
                assert_eq!(flown, vec![1, 2, 3, 4, 5], "slot {slot} shield {shield}");
            }
        }
    }

    /// **A realm above the lord count gets no shield from the walk**, so the
    /// caller is the only thing that can decide what it flies. Written here
    /// because a zero that quietly became colour 1 would be invisible from
    /// [`Scenario::from_map_world`], where every realm ends up with a colour
    /// either way.
    #[test]
    fn the_walk_hands_out_exactly_one_shield_per_lord() {
        let a = assign_lords(&NewGame { lords: 3, shield: 4, ..NewGame::default() }, 3);
        assert_eq!(a.shield[1], 4, "the person's own choice");
        assert_eq!(a.shield[2], 1, "the lowest colour nobody took");
        assert_eq!(a.shield[3], 2);
        assert_eq!(a.shield[4], 0, "out of play, and the walk says nothing about it");
        assert_eq!(a.shield[5], 0);
        assert_eq!(a.lord[4], 0);
    }

/// **A colour outside the five is refused**, for the
    /// reason [`MapError::Shield`] gives: it can only get here by our own carry
    /// being wrong, and a clamp would turn that into a plausible blue.
    #[test]
    fn a_shield_outside_the_five_is_refused() {
        let w = world();
        for bad in [0u8, 6, 255] {
            let setup = NewGame { lords: 1, shield: bad, ..NewGame::default() };
            assert_eq!(Scenario::from_map_world(&w, &setup), Err(MapError::Shield(bad)));
        }
        // And every one of the five is accepted, so the guard is a range and
        // not a rejection of everything that is not the default.
        for good in 1..=5u8 {
            let setup = NewGame { lords: 1, shield: good, ..NewGame::default() };
            let s = Scenario::from_map_world(&w, &setup).expect("shield {good} builds");
            assert_eq!(s.realms[1].shield_index, good);
        }
    }

    #[test]
    fn the_labour_shares_sum_to_a_hundred_in_both_halves() {
        let c = county_reset(1);
        assert_eq!(c.labour_share[0..3].iter().sum::<i32>(), 100, "the farm half");
        assert_eq!(c.labour_share[3..].iter().sum::<i32>(), 100, "the industry half");
    }
}

