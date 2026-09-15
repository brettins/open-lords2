//! **`Map_InitScenario` (`0x004676E0`)** — a world built out of an
//! `L2_maps.dat` slot.
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
//! | `0x00451150` | `County_Reset` | every county's opening economy | [`county_reset`] |
//! | `0x00427ED0` | `Merchant_SpawnAll` | six merchants on six roads | [`spawn_merchants`] |
//! | `0x0049BC5F` | `PlayerStart_Compact` | drop the starts above the lord count | [`start_counties`] |
//! | `0x0049BD99` | `Game_SetupRealmsAndCounties` | seats the realms; the stores and the treasury | **split** — see below |
//!
//! * **The built castle.** `County_FindCastleTile` stamps terrain `0x14`, the
//!   bare plot, and that is load-time and is done here. Raising an actual
//!   castle on it is `FUN_0046826C`, which is keyed on the castle's *level* and
//!   its build percentage and is re-stamped every season — so it belongs beside
//!   the castle rules and is not this module's.

mod build_part;
pub use build_part::*;
mod sites;
pub use sites::*;
mod fields;
pub use fields::*;
mod setup;
pub use setup::*;
mod scenario;
pub use scenario::*;
mod tests_part;
pub use tests_part::*;

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


/// Plane-0 bits, under the names `docs/decisions.md` C25 settled.
mod bit {
    pub const TOWN: u8 = 0x40;
    pub const SITE: u8 = 0x80;
    pub const PLOT: u8 = 0x10;
    pub const FARM: u8 = 0x20;
    /// The county boundary. `FUN_0046C147` masks it off before classifying a
/// neighbour, so it can never make a tile stop being farmland.
    pub const BOUNDARY: u8 = 0x02;
}

const BANK_LAYER: u8 = 0x1C;
const BANK_MTNS: u8 = 0x04;
const BANK_ROADS: u8 = 0x08;
const BANK_TOWN: u8 = 0x0C;
const BANK_OVERLAY: u8 = 0x80;

/// `Town1?.pl8` frames the *file* stores on an industry site, and what each
/// one is. `County_PlaceResourceSites` (`0x00468E61`) reads exactly these three.
const FRAME_QUARRY: u8 = 0;
const FRAME_FOREST: u8 = 20;
const FRAME_MINE: u8 = 30;

const IND_WOOD: usize = 0;
const IND_IRON: usize = 1;
const IND_WEAPONS: usize = 2;
const IND_STONE: usize = 3;

const TERRAIN_IRON: u8 = 1;
const TERRAIN_STONE: u8 = 4;
const TERRAIN_WEAPONS: u8 = 7;
const TERRAIN_WOOD: u8 = 10;
const TERRAIN_CASTLE_PLOT: u8 = 0x14;

/// `docs/formats/maps-layers.md` §5.4 and `docs/decisions.md` C41: the three
/// groups 47–50, 51–54 and 55–58 are the three village sizes and the
/// population pass re-stamps them, so this is only the smallest.
const FRAME_VILLAGE_SMALL: u8 = 47;

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
///
/// `Realms_AssignLords` (`0x0049CAAA`) indexes it
/// `0x004DC17C + (g_scenarioIndex & 3) * 0x14 + realm.shieldIndex * 4 + n` and
/// takes the first candidate no other realm has taken. Held flat because the
/// index arithmetic runs off the end of its own group: `shieldIndex` is 1…5, so
/// slot 5 of group *g* is slot 0 of group *g+1*, and reproducing that needs one
/// array.
///
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
];

/// `County_Reset`'s opening numbers. **`[D]`** — `FUN_00451150`, straight down.
mod reset {
    pub const POPULATION: i32 = 150;
    pub const HEALTH_METER: i32 = 50;
    pub const HEALTH_BAND: u8 = 2;
    pub const HAPPINESS: i32 = 50;
    pub const RATION_WANTED: i32 = 3;
    pub const RATION_SPLIT: i32 = 100;
    pub const DRYNESS: i32 = 50;
    pub const GRAIN: i32 = 100;
    pub const HERD: i32 = 40;
    pub const INDUSTRY_SHARE: i32 = 25;
    /// `Labour_DefaultShares` (`0x004514F8`): farm 33/50/17 and the whole
    /// industry share on wood. Both halves sum to 100, which is the invariant
    /// that identifies the array.
    pub const LABOUR_SHARE: [i32; 8] = [33, 50, 17, 0, 0, 0, 100, 0];
    pub const WEATHER: u8 = 3;
}


#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MapError {
    NoCounties,
    CountyCount(usize),
    NoPlayerStarts,
    /// `FUN_004335F0` refuses to press *Start* at all in that case; reaching
    /// here means the guard was skipped.
    TooManyLords { lords: usize, seats: usize },
    StartCounty { marker: usize, county: u8 },
    LocalPlayer(u8),
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


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NewGame {
    pub slot: usize,
    pub options: Options,
    pub lords: usize,
    pub local_player: u8,
    /// The original keeps it twice: `g_playerSlots + realm * 0x2C + 0x25` (the record
    /// `g_playerNames` is the `+0x04` of, so four bytes lower than the name),
    /// which is what `Realms_AssignLords` reads, and `g_realms[p].shieldIndex`,
    /// which is what everything that *draws* reads. `FUN_00432FAB`
    /// (`0x00432FAB`) writes both from the clicked shield and `FUN_004978AD`
    pub shield: u8,
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


/// `g_tiles` (`0x00522F90`) — 4,096 eight-byte records, `y * 64 + x`, as seven
/// planes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tiles {
    pub content: Vec<u8>,
    pub flags: Vec<u8>,
    pub bank: Vec<u8>,
    pub frame: Vec<u8>,
    pub part: Vec<u8>,
    pub saved_frame: Vec<u8>,
    pub county: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapWorld {
    pub tiles: Tiles,
    pub county_count: usize,
    pub player_start: [u8; 6],
    pub player_start_count: usize,
    pub routes: MerchantRoutes,
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
    pub field_tiles: Vec<[u16; MAX_FIELDS]>,
    /// County `+0x205` — how many farm tiles the county had **before** the
    /// twenty-slot table razed the overflow. Carried because it is the bound
    /// on the reclamation cursor and nothing else in the workspace has it.
    pub farm_tile_count: Vec<u8>,
}

