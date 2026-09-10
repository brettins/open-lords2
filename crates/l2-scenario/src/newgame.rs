//! **`Map_InitScenario` (`0x004676E0`)** — a world built out of an
//! `L2_maps.dat` slot rather than out of a save.
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
//!   indices — the coast and sea variants, and the bank's draw bits. The tile
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
//!   defaults and the option rows that overwrite them.
//! * **The starting garrison.** `Army_Create` at setup is
//!   `Settings::unhonoured`'s, exactly as it is on the save path.

use l2_formats::maps::{MapSlot, Plane, PLANE_DIM};
use l2_kingdom::county::{MAX_COUNTIES, MAX_COUNTY_ID, MAX_FIELDS, MAX_NEIGHBOURS};
use l2_kingdom::map::{CampaignMap, MAP_TILES};
use l2_kingdom::merchant::{self, MerchantRoutes, ROUTES, ROUTE_SLOTS};
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
    /// neighbour, which is why it can never make a tile stop being farmland.
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
/// array rather than four.
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
    /// The weather every county opens on. `g_weatherCounty` opens on 1.
    pub const WEATHER: u8 = 3;
}

// --------------------------------------------------------------- the failures

/// A map slot that cannot be made into a world.
///
/// Each of these is a refusal rather than a default, for the reason
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
    /// **A refusal rather than `FUN_004171EE`'s clamp**, and deliberately the
    /// opposite of what [`crate::Scenario::from_save`] does with the realm
    /// colour byte it reads out of a file. That byte is somebody else's and a
    /// clamp there would hide a misread offset; this one is *ours*, chosen on a
    /// screen that can only produce 1 … 5, so a value outside the range is a
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
/// `Map_InitScenario` or by the seating pass — which is why the world builder
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
    /// The original keeps it twice: `g_playerNames + realm * 0x2C + 0x25`,
    /// which is what `Realms_AssignLords` reads, and `g_realms[p].shieldIndex`,
    /// which is what everything that *draws* reads. `FUN_00432FAB`
    /// (`0x00432FAB`) writes both from the clicked shield and `FUN_004978AD`
    /// seeds both at 1, which is why a game nobody touches the page of is red.
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
    /// the code. Passing the seed in rather than drawing from the kingdom's
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

    /// The three planes the simulation reads.
    pub fn campaign_map(&self) -> CampaignMap {
        CampaignMap::from_planes(&self.content, &self.flags, &self.county)
            .expect("Tiles holds MAP_TILES of each")
    }

    fn bank_layer(&self, tile: usize) -> u8 {
        self.bank[tile] & BANK_LAYER
    }

    /// `FUN_0046AC22(frameBase, 2, tile, layerBit, content)` — stamp a 2×2
    /// object. The bank is rebuilt rather than or-ed: `(bank | 1) & 0xE3 | bit`.
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

/// The whole of `Map_InitScenario`, in its order.
pub fn build(slot: &MapSlot<'_>, setup: &NewGame) -> Result<MapWorld, MapError> {
    let mut w = load_planes(slot)?;
    adjacency(&mut w);
    place_sites(&mut w);
    place_starting_fields(&mut w, setup.options.difficulty);
    collect_field_tiles(&mut w);
    pick_merchant_starts(&mut w);
    Ok(w)
}

/// `Map_LoadPlanes` (`0x00467770`) whole: the copy, the county count, and the
/// plane-4 dispatch.
///
/// **The dispatch is two tables and the flag bit picks which.** `[V]` and it
/// was written the other way round until `docs/decisions.md` C25:
///
/// ```c
/// if (marker != 0) {
///     if ((flags & 0x40) == 0) { if (flags & 0x80) PlayerStart_Record(county, marker); }
///     else                     Merchant_RouteAppend(county, marker);
/// }
/// ```
///
/// so a marker on the **county town** extends a trade route and a marker on the
/// **castle** is a player start — which is what both of those are.
fn load_planes(slot: &MapSlot<'_>) -> Result<MapWorld, MapError> {
    let tiles = Tiles::from_slot(slot);

    let mut county_count = 0usize;
    let mut player_start = [0u8; 6];
    let mut player_start_count = 0usize;
    let mut routes = MerchantRoutes::none();
    let mut rows = [[0u8; ROUTE_SLOTS]; ROUTES];

    for y in 0..PLANE_DIM {
        for x in 0..PLANE_DIM {
            let i = y * PLANE_DIM + x;
            let county = tiles.county[i];
            // `if ((county < 0x11) && (g_countyCount < county))` — 32 is in the
            // plane and is not a county, and this is the clamp that says so.
            if county < 0x11 && county_count < county as usize {
                county_count = county as usize;
            }
            let marker = slot.at(Plane::Marker, x, y);
            if marker == 0 {
                continue;
            }
            if tiles.flags[i] & bit::TOWN != 0 {
                // `Merchant_RouteAppend(county, marker)`: the first free cell
                // of row `marker - 1`. A marker of 0 cannot reach here.
                let row = marker as usize - 1;
                if let Some(cells) = rows.get_mut(row) {
                    if let Some(cell) = cells.iter_mut().find(|c| **c == 0) {
                        *cell = county;
                    }
                }
            } else if tiles.flags[i] & bit::SITE != 0 {
                // `PlayerStart_Record(county, marker)`. The counter counts
                // *writes*, not distinct markers — see the note on
                // `player_start_count` below.
                if let Some(cell) = player_start.get_mut(marker as usize) {
                    *cell = county;
                }
                player_start_count += 1;
            }
        }
    }

    if county_count == 0 {
        return Err(MapError::NoCounties);
    }
    if county_count > MAX_COUNTY_ID as usize {
        return Err(MapError::CountyCount(county_count));
    }
    for (row, cells) in rows.iter().enumerate() {
        routes.set_row(row, *cells);
    }
    // **The counter and the table disagree on a map that repeats a marker.**
    // `PlayerStart_Record` increments on every write, so two castle tiles
    // carrying the same marker count twice and store once.
    // `l2_formats::maps::MapSlot::player_start_count` counts distinct markers,
    // which is what the table can hold; the seat count the *screen* shows comes
    // from there. This one reproduces the original's own arithmetic so that a
    // map where they differ is visible rather than smoothed over — no shipped
    // map does, which `tests/newgame.rs` asserts over all 44.
    Ok(MapWorld {
        tiles,
        county_count,
        player_start,
        player_start_count,
        routes,
        merchant_start: [0; ROUTES],
        neighbours: vec![Vec::new(); MAX_COUNTIES],
        town_tile: vec![0; MAX_COUNTIES],
        anchor: vec![(0, 0); MAX_COUNTIES],
        castle_tile: vec![0; MAX_COUNTIES],
        castle: vec![(0, 0); MAX_COUNTIES],
        dwelling_plots: vec![[0; 4]; MAX_COUNTIES],
        industry_site: vec![[0; 4]; MAX_COUNTIES],
        has_resource: vec![[false; 4]; MAX_COUNTIES],
        field_tiles: vec![[0; MAX_FIELDS]; MAX_COUNTIES],
        farm_tile_count: vec![0; MAX_COUNTIES],
    })
}

/// The four 4-adjacent neighbours of a tile, in `FUN_0046C080`'s order —
/// north, east, south, west — with the edge reading as county 0.
fn four_neighbours(x: usize, y: usize) -> [Option<usize>; 4] {
    [
        if y > 0 { Some((y - 1) * PLANE_DIM + x) } else { None },
        if x + 1 < PLANE_DIM { Some(y * PLANE_DIM + x + 1) } else { None },
        if y + 1 < PLANE_DIM { Some((y + 1) * PLANE_DIM + x) } else { None },
        if x > 0 { Some(y * PLANE_DIM + x - 1) } else { None },
    ]
}

/// `FUN_00467CA6` — county `+0x5C`, the zero-terminated adjacency list.
///
/// **Ascending by discovery, not sorted.** The original appends in the scan's
/// own order (`y` outer, `x` inner, then N/E/S/W), stops at sixteen entries,
/// and counts them into `+0x5B`. Reproducing the order matters: the list is
/// walked by index in half a dozen places and two peers must walk it the same
/// way (`docs/netcode.md` D-4).
fn adjacency(w: &mut MapWorld) {
    let n = w.county_count;
    for y in 0..PLANE_DIM {
        for x in 0..PLANE_DIM {
            let i = y * PLANE_DIM + x;
            let c = w.tiles.county[i] as usize;
            if c == 0 || c >= 0x12 {
                continue;
            }
            for slot in four_neighbours(x, y) {
                let other = slot.map(|t| w.tiles.county[t]).unwrap_or(0);
                if other == 0 || other as usize >= 0x12 || other as usize == c {
                    continue;
                }
                let list = &mut w.neighbours[c];
                if list.len() >= MAX_NEIGHBOURS || list.contains(&other) {
                    continue;
                }
                list.push(other);
            }
        }
    }
    // A county id above the count cannot be a county; the original's clamp is
    // `< 0x12` and the shipped maps never exercise the difference.
    for c in 0..MAX_COUNTIES {
        if c == 0 || c > n {
            w.neighbours[c].clear();
        } else {
            w.neighbours[c].retain(|&o| o as usize <= n);
        }
    }
}

/// `Counties_PlaceSites` (`0x00468D4F`) — five passes per county, and **the
/// order is load-bearing**.
///
/// The town goes first because the anchor it leaves is what the blacksmith
/// measures distance from; the resource sites go before the castle because they
/// stamp a terrain onto the Town-bank `0x80` tiles, and the castle search is
/// *"a `0x80` tile with no terrain yet"* — which is exactly the tiles the
/// resource sites did not take. Reorder them and the county's mine becomes its
/// castle.
fn place_sites(w: &mut MapWorld) {
    for county in 1..=w.county_count {
        let Some((town, anchor)) = find_town_tile(w, county) else { continue };
        w.town_tile[county] = town;
        w.anchor[county] = anchor;
        w.dwelling_plots[county] = find_dwelling_plots(w, county);
        place_resource_sites(w, county);
        place_blacksmith(w, county);
        if let Some((tile, xy)) = find_castle_tile(w, county) {
            w.castle_tile[county] = tile;
            w.castle[county] = xy;
        }
    }
}

/// `County_FindTownTile` (`0x00467FD1`).
///
/// Returns the block's **north-west** tile and the county anchor, which is its
/// **south-east** one — the original stores the fourth match's `x`/`y` and
/// returns the first match's offset, and those are not the same tile.
fn find_town_tile(w: &mut MapWorld, county: usize) -> Option<(usize, (u8, u8))> {
    let mut found = 0;
    let mut first = 0usize;
    for y in 0..PLANE_DIM {
        for x in 0..PLANE_DIM {
            let i = y * PLANE_DIM + x;
            if w.tiles.county[i] as usize != county
                || w.tiles.flags[i] & bit::TOWN == 0
                || w.tiles.content[i] != 0
            {
                continue;
            }
            if found == 0 {
                first = i;
                w.tiles.bank[i] |= BANK_OVERLAY;
                w.tiles.stamp_2x2(i, FRAME_VILLAGE_SMALL, BANK_TOWN, 0);
            }
            if found == 2 {
                w.tiles.bank[i] |= BANK_OVERLAY;
            }
            if found == 3 {
                return Some((first, (x as u8, y as u8)));
            }
            found += 1;
        }
    }
    None
}

/// `County_FindDwellingPlots` (`0x00468C41`) — the four `0x10` tiles, their
/// terrain frames saved so razing one can put the ground back.
///
/// **Four is the storage, not a rule.** The original's counter has no bound and
/// the four `i32` slots at county `+0x80` end exactly on `fieldProgress`, so a
/// fifth plot in one county would corrupt a field's reclamation. All 434
/// counties of the 44 shipped maps have exactly four
/// (`docs/formats/maps-layers.md` §2.3); this one drops the fifth rather than
/// reproducing the overrun, because the overrun is a memory bug and not a rule.
fn find_dwelling_plots(w: &mut MapWorld, county: usize) -> [usize; 4] {
    let mut plots = [0usize; 4];
    let mut n = 0;
    for i in 0..MAP_TILES {
        if w.tiles.flags[i] & bit::PLOT == 0 || w.tiles.county[i] as usize != county {
            continue;
        }
        if n < plots.len() {
            plots[n] = i;
        }
        w.tiles.saved_frame[i] = w.tiles.frame[i];
        w.tiles.content[i] = 0;
        n += 1;
    }
    plots
}

/// `County_PlaceResourceSites` (`0x00468E61`) — **where a county's resources
/// come from is the map, not the county record.**
///
/// Three of the four industries: a Town-bank tile of this county drawing frame
/// 30 is the iron mine, frame 0 is the quarry and frame 20 is the forest. The
/// fourth, weapons, is [`place_blacksmith`].
///
/// **The stone test carries an `iron == 0` guard and it is not symmetric.** The
/// scan is row-major and iron is tested first at every tile, so a county whose
/// quarry comes *before* its mine gets both and one whose mine comes first gets
/// only the mine. That is the original's arithmetic and it is why iron and
/// stone are complementary in thirteen of England's fourteen counties rather
/// than in all of them.
fn place_resource_sites(w: &mut MapWorld, county: usize) {
    for i in 0..MAP_TILES {
        if w.tiles.county[i] as usize != county || w.tiles.bank_layer(i) != BANK_TOWN {
            continue;
        }
        let frame = w.tiles.frame[i];
        if frame == FRAME_MINE {
            set_site(w, county, i, IND_IRON, TERRAIN_IRON);
        }
        if frame == FRAME_QUARRY && !w.has_resource[county][IND_IRON] {
            set_site(w, county, i, IND_STONE, TERRAIN_STONE);
        }
        if frame == FRAME_FOREST {
            set_site(w, county, i, IND_WOOD, TERRAIN_WOOD);
        }
    }
}

fn set_site(w: &mut MapWorld, county: usize, tile: usize, record: usize, terrain: u8) {
    w.has_resource[county][record] = true;
    w.industry_site[county][record] = tile;
    w.tiles.content[tile] = terrain;
    w.tiles.bank[tile] |= BANK_OVERLAY;
}

/// `FUN_0046C147` — the flag byte of one 4-neighbour, classified.
///
/// The boundary bit is masked off first, which is why a field on a county
/// border is still a field; then two corrections fold the mountain and the
/// woodland into one bit and the reserved plot into nothing when the bank
/// disagrees. Only bit `0x20` is read by the one caller here, and the mask is
/// what makes `0x22` count.
fn neighbour_class(w: &MapWorld, tile: Option<usize>) -> u8 {
    let Some(t) = tile else { return 0 };
    let raw = w.tiles.flags[t];
    if raw == 0 {
        return 0;
    }
    let layer = w.tiles.bank_layer(t);
    let mut r = raw & !bit::BOUNDARY;
    if r == 0x08 && layer != BANK_MTNS {
        r = 0x10;
    }
    if r == 0x10 && layer != BANK_ROADS {
        r = 0;
    }
    r
}

/// `Dist_Chebyshev` (`0x00404F4C`).
fn chebyshev(a: (u8, u8), b: (u8, u8)) -> i32 {
    let dx = (a.0 as i32 - b.0 as i32).abs();
    let dy = (a.1 as i32 - b.1 as i32).abs();
    dx.max(dy)
}

/// `County_PlaceBlacksmith` (`0x0046902A`) — **the weapons site is derived, not
/// authored, which is why every county has one.**
///
/// The pick is the county's *plain* tile — flags exactly zero, so not a road,
/// not a field, not a boundary — that is 4-adjacent to one of the county's own
/// farm fields and nearest the town anchor. Ties go to the first in row-major
/// order, because the comparison is strict.
///
/// It **adds** [`bit::SITE`] to a tile the file had no flag on at all, so this
/// is one of the two places where the runtime flags plane is not the file's.
/// The other is a razed field. A reader diffing our flags against
/// `L2_maps.dat`'s should expect exactly `county_count` extra `0x80` tiles.
fn place_blacksmith(w: &mut MapWorld, county: usize) {
    let anchor = w.anchor[county];
    let mut best_tile = 0usize;
    let mut best = 0x40i32;
    for y in 0..PLANE_DIM {
        for x in 0..PLANE_DIM {
            let i = y * PLANE_DIM + x;
            if w.tiles.county[i] as usize != county || w.tiles.flags[i] != 0 {
                continue;
            }
            let n = four_neighbours(x, y);
            for slot in n {
                let same_county = slot.map(|t| w.tiles.county[t] as usize) == Some(county);
                if !same_county {
                    continue;
                }
                if neighbour_class(w, slot) & bit::FARM == 0 {
                    continue;
                }
                let d = chebyshev(anchor, (x as u8, y as u8));
                if d < best {
                    best = d;
                    best_tile = i;
                }
            }
        }
    }
    // `if ((local_14 != 0) && (flags[local_14] == 0))` — tile 0 is the "none"
    // encoding, so the north-west corner of the map can never be a blacksmith.
    if best_tile == 0 || w.tiles.flags[best_tile] != 0 {
        return;
    }
    w.has_resource[county][IND_WEAPONS] = true;
    w.industry_site[county][IND_WEAPONS] = best_tile;
    w.tiles.flags[best_tile] |= bit::SITE;
    w.tiles.content[best_tile] = TERRAIN_WEAPONS;
    w.tiles.bank[best_tile] = (w.tiles.bank[best_tile] & 0xE3) | BANK_TOWN | BANK_OVERLAY;
    w.tiles.frame[best_tile] = 10;
}

/// `County_FindCastleTile` (`0x00468121`) — the bare plot, and **only** the
/// bare plot.
///
/// Finds the county's 2×2 of `0x80` tiles that no resource site took, stamps
/// terrain `0x14` on all four and saves their terrain frames. Raising a castle
/// on the plot is `FUN_0046826C`, which is keyed on the castle's level and its
/// build percentage rather than on the map, and is not done here.
///
/// Returns the block's **north-west** tile, which is both the returned offset
/// and the stored `x`/`y` — the opposite of [`find_town_tile`], where they are
/// different tiles.
fn find_castle_tile(w: &mut MapWorld, county: usize) -> Option<(usize, (u8, u8))> {
    let mut found = 0;
    let mut first = None;
    for y in 0..PLANE_DIM {
        for x in 0..PLANE_DIM {
            let i = y * PLANE_DIM + x;
            if w.tiles.county[i] as usize != county
                || w.tiles.flags[i] & bit::SITE == 0
                || w.tiles.content[i] != 0
            {
                continue;
            }
            if found == 0 {
                first = Some((i, (x as u8, y as u8)));
                w.tiles.bank[i] |= BANK_OVERLAY;
            }
            w.tiles.saved_frame[i] = w.tiles.frame[i];
            w.tiles.content[i] = TERRAIN_CASTLE_PLOT;
            if found == 3 {
                return first;
            }
            found += 1;
        }
    }
    // Fewer than four: the original returns 0 and the county has no castle
    // tile, but the tiles it did find keep the stamp. Reproduced.
    None
}

/// `Map_PlaceStartingFields` (`0x00467A36`) — every farm tile's opening crop.
///
/// The ladder is *per county*, on the tile's ordinal within it, and the whole
/// of the difficulty setting's effect on the land is here:
///
/// | difficulty | pasture | fallow | wild |
/// |---|---|---|---|
/// | 0 easiest | first 8 | the rest | — |
/// | 1 | first 6 | 2 more | the rest |
/// | 2 | first 4 | 2 more | the rest |
/// | 3 hardest | first 4 | — | the rest |
///
/// The frame is `base + ((storedFrame + 0xB0) & 3)`, which keeps the tile's own
/// variant: every crop state is four consecutive frames and `& 3` picks the
/// same one of the four. `FUN_0046D7F4` then recomputes the identical number
/// from the other side (`docs/formats/maps-layers.md` §5.5), which is a
/// redundancy in the original and not a second rule.
fn place_starting_fields(w: &mut MapWorld, difficulty: u8) {
    let mut ordinal = [0i32; 0x20];
    for c in w.farm_tile_count.iter_mut() {
        *c = 0;
    }
    for y in 0..PLANE_DIM {
        for x in 0..PLANE_DIM {
            let i = y * PLANE_DIM + x;
            let county = w.tiles.county[i] as usize;
            if county == 0 || county >= 0x12 || w.tiles.flags[i] & bit::FARM == 0 {
                continue;
            }
            if let Some(c) = w.farm_tile_count.get_mut(county) {
                *c = c.wrapping_add(1);
            }
            let nth = ordinal[county];
            ordinal[county] += 1;
            let (content, base) = match difficulty {
                3 if nth < 4 => (0x14u8, 104u8),
                3 => (0, 80),
                2 if nth < 4 => (0x14, 104),
                2 if nth < 6 => (1, 84),
                2 => (0, 80),
                1 if nth < 6 => (0x14, 104),
                1 if nth < 8 => (1, 84),
                1 => (0, 80),
                _ if nth < 8 => (0x14, 104),
                _ => (1, 84),
            };
            let variant = w.tiles.frame[i].wrapping_add(0xB0) & 3;
            w.tiles.frame[i] = base + variant;
            w.tiles.content[i] = content;
            // `FUN_0046D7F4`'s bank half, which the frame half above already
            // agrees with: the roads layer, bit 0x01 set, bit 0x80 cleared and
            // then set again for a terrain in 0x0F..0x17 — which pasture is
            // and neither fallow nor wild is.
            let mut bank = ((w.tiles.bank[i] | 1) & 0xE3) | BANK_ROADS;
            bank &= 0x7F;
            if content > 0x0E && content < 0x17 {
                bank |= BANK_OVERLAY;
            }
            w.tiles.bank[i] = bank;
        }
    }
}

/// `County_CollectFieldTiles` (`0x0046DA4B`) — `g_countyFieldTiles`, and **the
/// twenty-slot table is a hard limit that razes what will not fit.**
///
/// This is the function that makes twenty fields per county a *fact about the
/// map* rather than a cap on a counter. A county with twenty-one farm tiles
/// loses the twenty-first outright: terrain 0, frame 6, **flags zeroed** and
/// the bank put back to base — it stops being farmland at all and becomes
/// plain grass, before the first season runs.
///
/// `[D]` on the consequence, `[V]` on the code: the razing branch is the `else`
/// of *"is there a free slot"*, and the four writes are literal.
fn collect_field_tiles(w: &mut MapWorld) {
    for row in w.field_tiles.iter_mut() {
        *row = [0; MAX_FIELDS];
    }
    for i in 0..MAP_TILES {
        let county = w.tiles.county[i] as usize;
        if county == 0 || county >= 0x12 || w.tiles.flags[i] & bit::FARM == 0 {
            continue;
        }
        let placed = w
            .field_tiles
            .get_mut(county)
            .and_then(|row| row.iter_mut().find(|slot| **slot == 0))
            .map(|slot| *slot = i as u16)
            .is_some();
        if !placed {
            w.tiles.content[i] = 0;
            w.tiles.frame[i] = FRAME_GRASS;
            w.tiles.flags[i] = 0;
            w.tiles.bank[i] &= 0xE3;
        }
    }
}

/// `Merchant_PickStartCounties` (`0x004291B3`) — one start county per trade
/// route, each different from the ones already handed out.
///
/// **The dedup walk is odd and it is reproduced exactly**, because a merchant
/// that starts in the wrong county walks the wrong route for the rest of the
/// game. Candidate 0 is the route's first town; after that it tries cells
/// 2, 4, 6 … and, on running off the end of the row, cells 3, 5, 7 …; **cell 1
/// is never tried**; and after five retries it stores whatever it has, taken or
/// not. `Merchant_StartCountyTaken` returns 1 for county 0 as well, because
/// unset entries are 0 — so a row that runs out of candidates stores 0, and
/// `Merchant_SpawnAll` stops dead at the first zero.
fn pick_merchant_starts(w: &mut MapWorld) {
    let taken = |starts: &[u8; ROUTES], c: u8| starts.iter().any(|&s| s == c);
    let mut starts = [0u8; ROUTES];
    for row in 0..ROUTES {
        let cells = *w.routes.row(row);
        let mut cursor = 0usize;
        let mut candidate = cells[0];
        let mut tries = 0;
        while taken(&starts, candidate) {
            tries += 1;
            if tries >= 6 {
                break;
            }
            candidate = cells.get(cursor + 2).copied().unwrap_or(0);
            cursor += 2;
            if candidate == 0 {
                cursor = 1;
            }
        }
        starts[row] = candidate;
    }
    w.merchant_start = starts;
}

// ---------------------------------------------------------------- the seating

/// `FUN_00497E65` (`0x00497E65`) — **the start table is shuffled, and that is
/// why which realm you play is different every game.**
///
/// `docs/environment.md` records the fact from the other end: the England
/// turn-one fixture's fingerprint deliberately excludes the realm→county
/// assignment, because two independently created saves of the same map
/// disagree about it. This is the code that disagrees. `Game_NewGame` calls it
/// between `Mercenary_Init` and `PlayerStart_Compact`, and it re-deals the
/// live entries of `g_playerStartTable` into each other's slots:
///
/// ```c
/// for (i = 1; i <= live; i++) {
///     pos = (rand & 3) + 1 + i;  if (pos > live) pos = 1;
///     for (tries = 0; tries < live; tries++) {
///         if (table[pos] == 0) { table[pos] = src[i]; break; }
///         if (++pos > live) pos = 1;
///     }
/// }
/// ```
///
/// **`[D]`, and the function had no name until now.** Its second half deals a
/// 48-entry table into `DAT_0057CAE0` and fills `DAT_00553080` with a value per
/// entry (`0`, `100`, or `6 + 5n`); neither destination has been traced and
/// neither is a player start, so neither is reproduced.
///
/// **The generator is ours and the shape is the original's**, for the reason
/// `l2_game::scenario::SEED` gives: the original draws from two 31-bit LFSRs
/// whose state no save records, and reproducing its *stream* is impossible from
/// a file. What is reproduced is the deal — a random offset, then a linear
/// probe forward for a free slot, wrapping at the live count.
fn shuffle_starts(w: &MapWorld, seed: u64) -> Vec<u8> {
    // `local_ec`: how many of entries 1..5 the map filled.
    let src: Vec<u8> = (1..w.player_start.len()).map(|m| w.player_start[m]).collect();
    let live = src.iter().filter(|&&c| c != 0).count();
    if live == 0 {
        return Vec::new();
    }
    let mut rng = l2_kingdom::Pcg32::from_seed(seed);
    let mut table = vec![0u8; live + 1];
    for i in 1..=live {
        let mut pos = (rng.next_u32() & 3) as usize + 1 + i;
        if pos > live {
            pos = 1;
        }
        for _ in 0..live {
            if table[pos] == 0 {
                table[pos] = src[i - 1];
                break;
            }
            pos += 1;
            if pos > live {
                pos = 1;
            }
        }
    }
    // The probe can leave an entry unplaced only if every slot was full, which
    // needs more sources than slots; there are exactly as many of each.
    debug_assert!(table[1..].iter().all(|&c| c != 0), "the deal placed every start");
    table[1..].to_vec()
}

/// `PlayerStart_Compact` (`0x0049BC5F`) — the start counties the live realms
/// get, realm 1 first.
///
/// The original bubbles entries whose *slot number* is above the live realm
/// count out of a six-entry table, repeatedly, until none is left. Every
/// populated entry's slot number is its own index (`PlayerStart_Record` writes
/// both), and the markers on every shipped map are contiguous from 1, so the
/// result is simply the first `lords` entries of the table as
/// [`shuffle_starts`] left it. Written that way rather than as the bubble, with
/// the equivalence stated here and checked over all 44 shipped maps in
/// `tests/newgame.rs`.
fn start_counties(w: &MapWorld, lords: usize, seed: u64) -> Result<Vec<u8>, MapError> {
    let seats = shuffle_starts(w, seed);
    if seats.is_empty() {
        return Err(MapError::NoPlayerStarts);
    }
    if lords > seats.len() {
        return Err(MapError::TooManyLords { lords, seats: seats.len() });
    }
    for (i, &c) in seats.iter().take(lords).enumerate() {
        if c as usize > w.county_count {
            return Err(MapError::StartCounty { marker: i + 1, county: c });
        }
    }
    Ok(seats)
}

/// What [`assign_lords`] decides for each realm: its shield, then its lord.
///
/// One struct rather than two arrays because **the second is a function of the
/// first** and a caller that could take one without the other would be able to
/// build a realm whose colour and lord disagree — which is exactly the state
/// `shield = realm` used to produce.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Assignment {
    /// Realm `+0x0A`, `shieldIndex`, 1 … 5. **Zero means the walk gave this
    /// realm nothing** — it is above the lord count — and the caller falls back
    /// to `FUN_0049C995`'s seed rather than inventing a colour.
    shield: [u8; MAX_REALMS],
    /// Realm `+0x28`, the lord id. Zero for a human and for a realm out of
    /// play, which is what the original writes at the top of every iteration.
    lord: [u8; MAX_REALMS],
}

/// `Realms_AssignLords` (`0x0049CAAA`) — **the shield first, by position, and
/// then the lord from the shield.**
///
/// # The arrow runs colour → lord, and no lord is ever consulted
///
/// 1. Mark every **human's** chosen shield taken. The original reads
///    `g_playerNames + realm * 0x2C + 0x25`, guarded by `+0x26 == 0` (a person
///    rather than a slot the AI fills), and page 4's `FUN_00432FAB` is what
///    wrote it.
/// 2. Walk realms **1 … 5 in realm order**, skipping humans and stopping when
///    `g_aiLordCount` lords have been handed out. Each AI takes **the lowest
///    shield nobody has taken**; a realm past the lord count gets no shield and
///    `strength = 0`.
/// 3. Then `lord = g_lordChoice[(g_scenarioIndex & 3) * 0x14 + shield * 4 + n]`
///    for `n` = 0 … 3, the first candidate no earlier realm has taken.
///
/// **This line used to read `let shield = realm`, under a doc comment that
/// stated that as the *mechanism*** — *"the colour slot is the realm id,
/// because `Game_SetupRealms` seeds `shieldIndex = i` and only a custom game's
/// colour picker permutes it."* The seed is real (`FUN_0049C995`) and the
/// conclusion drawn from it was not: the seed is what an *untouched* page 4
/// leaves, and `Realms_AssignLords` overwrites it for every AI on every run.
/// A sentence that explains a default as a rule is a sentence nobody re-reads,
/// which is why the line outlived four documents describing the real walk.
/// `docs/decisions.md` C130.
///
/// # The consequence a player will check
///
/// Take **yellow** and the Knight does not fall back to red: he becomes the
/// **black** lord and the **Baron** becomes the red one, because red's
/// candidate list names the Baron first and the walk reaches red before black.
/// `docs/rules.md` §7a has all five rows and
/// `crates/l2-game/tests/newgame.rs` drives the setup screen to each of them.
///
/// **`[D]` on the group.** The original picks the deterministic group 0 when
/// `DAT_0055302C == 1` and `(g_scenarioIndex & 3)` otherwise, and what
/// `DAT_0055302C` is has not been read. The scenario-varying path is taken here
/// because it is the one a single-player custom game reaches unless that flag
/// is set, and because it is the only reading under which the four groups exist
/// for a reason.
fn assign_lords(setup: &NewGame, lords: usize) -> Assignment {
    let group = setup.slot & 3;
    let human = setup.local_player as usize;
    // `g_aiLordCount` — `Setup_CommitOptions` keeps *Nobles* minus the people,
    // and this build has one person.
    let ai_lords = lords.saturating_sub(1);

    let mut a = Assignment { shield: [0; MAX_REALMS], lord: [0; MAX_REALMS] };
    // `acStack_14[6]` and `acStack_20[8]`, both zeroed at entry.
    let mut shield_taken = [false; 6];
    let mut lord_taken = [false; 8];

    // Step 1. The human's shield is taken before anybody walks.
    if human >= 1 && human < MAX_REALMS {
        a.shield[human] = setup.shield;
        shield_taken[setup.shield as usize] = true;
    }

    // Step 2. Realms 1 … 5 in realm order.
    let mut given = 0usize;
    for realm in 1..MAX_REALMS {
        if realm == human || given >= ai_lords {
            // A human keeps the shield he chose and takes no lord; a realm past
            // the lord count is the `strength = 0` limb and takes neither.
            continue;
        }
        given += 1;
        let Some(shield) = (1..=5u8).find(|s| !shield_taken[*s as usize]) else { continue };
        shield_taken[shield as usize] = true;
        a.shield[realm] = shield;
        // Step 3. And now the lord, out of that shield's four candidates.
        for n in 0..4usize {
            let at = group * 0x14 + shield as usize * 4 + n;
            let candidate = LORD_CHOICE.get(at).copied().unwrap_or(0);
            if candidate == 0 {
                continue;
            }
            if !lord_taken[candidate as usize & 7] {
                a.lord[realm] = candidate;
                lord_taken[candidate as usize & 7] = true;
                break;
            }
        }
    }
    a
}

// ------------------------------------------------------------- County_Reset

/// `County_Reset` (`0x00451150`) — the opening economy of **every** county,
/// owned or not.
///
/// It runs after `Map_InitScenario` and before the seating, and the option rows
/// then overwrite five of its numbers on every county
/// (`Game_SetupRealmsAndCounties`' first loop). What survives is the ration, the
/// split, the weather, the dryness, the labour shares and the industry share —
/// and `tax_rate`, which **nothing sets at all**: the county record was zeroed
/// wholesale by `FUN_0046EA28` in `Game_NewGame`'s preamble and no new-game
/// path writes a tax rate, so a fresh game opens at zero tax in every county.
/// `[D]`, and it is the answer to a question `docs/kingdom.md` does not ask.
fn county_reset(id: usize) -> CountyState {
    CountyState {
        owner: 0,
        population: reset::POPULATION,
        population_last: reset::POPULATION,
        happiness: reset::HAPPINESS,
        happiness_last: reset::HAPPINESS,
        shown_tax: 0,
        shown_ration: 0,
        shown_health: 0,
        shown_events: 0,
        d_hap_ration: 0,
        health_meter: reset::HEALTH_METER,
        health_band: reset::HEALTH_BAND,
        unrest: 0,
        births: 0,
        deaths: 0,
        emigrants: 0,
        immigrants: 0,
        // `popBand = (population - 1) / 25 + 1`, which the original computes
        // here rather than leaving to the population pass.
        pop_band: (reset::POPULATION - 1) / 25 + 1,
        anchor: (0, 0),
        neighbours: Vec::new(),
        tax_rate: 0,
        tax_collected: 0,
        ration_wanted: reset::RATION_WANTED,
        ration_achieved: 0,
        ration_split: reset::RATION_SPLIT,
        grain_eaten: 0,
        herd_eaten: 0,
        castle_type: 0,
        castle_building: 0,
        castle_switch: false,
        industry: [IndustryState::default(); 4],
        fields_fallow: 0,
        fields_cattle: 0,
        fields_grain: 0,
        fertility: 0,
        weather: Weather::from_index(reset::WEATHER).expect("3 is Cloudy"),
        dryness: reset::DRYNESS,
        grain: reset::GRAIN,
        herd: reset::HERD,
        labour: [0; JOB_COUNT],
        labour_wanted: [0; JOB_COUNT],
        labour_useful: [0; JOB_COUNT],
        labour_share: {
            let mut s = [0i32; JOB_COUNT - 1];
            let n = s.len().min(reset::LABOUR_SHARE.len());
            s[..n].copy_from_slice(&reset::LABOUR_SHARE[..n]);
            s
        },
        industry_share: reset::INDUSTRY_SHARE,
        field_tiles: [0; MAX_FIELDS],
        // `field_0x1FE = county & 1`. **The save path did not import this
        // until now** and every loaded county farmed as style 0; see
        // `Scenario::from_save`.
        farm_style: (id & 1) as u8,
    }
}

// ---------------------------------------------------------------- the merchants

/// `Merchant_SpawnAll` (`0x00427ED0`) — one type-3 merchant, owner 6, on a free
/// road tile near each start county, **stopping at the first zero**.
///
/// Owner 6 is nobody, which is what a merchant and a county's own levied
/// defence carry. The route cursor lives in the low byte of `yearFormed` and
/// opens at 1, not 0: the merchant's first destination is the *second* town on
/// its row, because it is standing in the first.
fn spawn_merchants(w: &MapWorld, map: &CampaignMap) -> Vec<(usize, Unit)> {
    let mut units = Units::new();
    let mut out = Vec::new();
    for (row, &county) in w.merchant_start.iter().enumerate() {
        if county == 0 {
            break;
        }
        let anchor = w.anchor.get(county as usize).copied().unwrap_or((0, 0));
        let Some((x, y)) = merchant::find_free_road_tile(map, &units, anchor)
            .or_else(|| merchant::find_free_open_tile(map, &units, anchor))
        else {
            continue;
        };
        let mut u = Unit::new(UnitKind::Merchant, 6, x, y);
        u.needs_destination = true;
        u.county = county;
        u.morale = 100;
        u.cargo_county = county;
        u.year_formed = 1;
        u.name_index = row as u8;
        if let Some(slot) = units.spawn(u.clone()) {
            out.push((slot, u));
        }
    }
    out
}

// ------------------------------------------------------------------ the whole

impl Scenario {
    /// **A world built from an `L2_maps.dat` slot.**
    ///
    /// The second constructor, and the one the *New Game* button needs.
    /// Everything it produces is the same plain data
    /// [`Scenario::from_save`] produces, so
    /// [`Scenario::starting_kingdom`] takes either without knowing which — and
    /// `crates/l2-scenario/tests/newgame.rs` builds England both ways and
    /// diffs them field by field.
    ///
    /// The clock is `Game_NewGame`'s: **Autumn 1267, with Winter next.**
    /// `Kingdom::start_new_game` then runs the one immediate `Season_Advance`
    /// that puts a new game in Winter 1268, so this is deliberately the
    /// position *before* it, exactly as [`Scenario::starting_kingdom`] is for a
    /// save.
    pub fn from_map(slot: &MapSlot<'_>, setup: &NewGame) -> Result<Scenario, MapError> {
        let world = build(slot, setup)?;
        Scenario::from_map_world(&world, setup)
    }

    /// The same, from an already-built [`MapWorld`] — for a caller that also
    /// wants the town tiles, the dwelling plots or the runtime frame plane,
    /// none of which fits on a [`Scenario`].
    pub fn from_map_world(w: &MapWorld, setup: &NewGame) -> Result<Scenario, MapError> {
        if setup.local_player < 1 || setup.local_player as usize >= MAX_REALMS {
            return Err(MapError::LocalPlayer(setup.local_player));
        }
        let lords = setup.lords.clamp(1, MAX_REALMS - 1);
        if setup.local_player as usize > lords {
            return Err(MapError::LocalPlayer(setup.local_player));
        }
        if !(1..=5).contains(&setup.shield) {
            return Err(MapError::Shield(setup.shield));
        }
        let seats = start_counties(w, lords, setup.seed)?;
        let assigned = assign_lords(setup, lords);

        let mut counties: Vec<Option<CountyState>> = vec![None; MAX_COUNTIES];
        for id in 1..=w.county_count {
            let mut c = county_reset(id);
            c.anchor = w.anchor[id];
            c.neighbours = w.neighbours[id].clone();
            c.field_tiles = w.field_tiles[id];
            for (record, slot) in c.industry.iter_mut().enumerate() {
                slot.has_resource = w.has_resource[id][record];
            }
            counties[id] = Some(c);
        }

        // `Game_SetupRealmsAndCounties`' seating half. The economy half — the
        // county-status row, the crowns, the armoury, the castle level — is
        // `l2_game::setup::Settings::apply_to`, which already runs on the save
        // path and now runs on this one too.
        let mut realms = vec![RealmState::default(); MAX_REALMS];
        for id in 1..MAX_REALMS {
            let realm = &mut realms[id];
            // **The walk's shield where it gave one, and `FUN_0049C995`'s seed
            // where it did not.** That seed — `shieldIndex = i` for realms
            // 1 … 5, run when the lobby is reset — is what a realm above the
            // lord count keeps, because `Realms_AssignLords` skips it. It is
            // the *default* and not the rule; reading it as the rule is what
            // this whole change is undoing, so it is written where it applies
            // and nowhere else.
            realm.shield_index =
                if assigned.shield[id] != 0 { assigned.shield[id] } else { id as u8 };
            if id > lords {
                // `Realms_AssignLords` writes `strength = 0` and hands out no
                // lord; `Game_SetupRealmsAndCounties` then skips the realm
                // entirely, so it gets no county and no gold.
                continue;
            }
            realm.in_play = true;
            realm.strength = 1;
            realm.is_human = id == setup.local_player as usize;
            // The walk already leaves a human at 0 — `g_realms[i].lord = 0` is
            // the first statement of every iteration — so this is the walk's
            // answer and not a second rule beside it.
            realm.lord = assigned.lord[id];
            realm.county_count = 1;
            let Some(&county) = seats.get(id - 1) else { continue };
            let Some(c) = counties.get_mut(county as usize).and_then(|c| c.as_mut()) else {
                continue;
            };
            c.owner = id as u8;
            // **The one industry a start county opens with switched on**, and
            // it is not the blacksmith: the loop skips record 2 and takes the
            // first of wood, iron, stone the county has a resource for.
            if let Some(slot) =
                c.industry.iter_mut().enumerate().find(|(r, s)| *r != IND_WEAPONS && s.has_resource)
            {
                slot.1.enabled = true;
            }
            // County `+0x1B0` — the castle-building switch, on from the first
            // turn in every start county.
            c.castle_switch = true;
        }

        let map = w.tiles.campaign_map();
        let units = spawn_merchants(w, &map);

        Ok(Scenario {
            county_count: w.county_count,
            local_player: setup.local_player,
            // `County_Reset` opens `g_weatherCounty` on 1.
            weather_county: 1,
            options: setup.options,
            clock: Clock { season: 3, season_next: 4, year: 1267, turn_count: 0 },
            counties,
            realms,
            map,
            units,
            routes: w.routes.clone(),
            merchant_start: w.merchant_start,
        })
    }
}

impl Default for RealmState {
    fn default() -> RealmState {
        RealmState {
            // A NEW game: `Game_NewGame` runs `Diplo_Init` after the realms
            // are set up, so the opening matrix is written there rather than
            // here. Default is the right thing to carry -- the values depend on
            // which realms are in play and which are people, and this
            // constructor knows neither yet.
            pairs: Default::default(),
            in_play: false,
            strength: 0,
            is_human: false,
            lord: 0,
            shield_index: 0,
            county_count: 0,
            rank: 0,
            score: 0,
            gold: 0,
            wages: 0,
            iron: 0,
            stone: 0,
            wood: 0,
            weapons: [0; l2_kingdom::tables::WEAPON_TYPE_COUNT],
        }
    }
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
        // Two farm fields, side by side, so a plain tile beside them can be a
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

    /// **A colour outside the five is refused rather than clamped**, for the
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
