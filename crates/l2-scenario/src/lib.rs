//! **The seam.** A shipped Lords of the Realm II save on one side, a live
//! [`l2_kingdom::Kingdom`] on the other, and the only place in the workspace
//! that is allowed to know both.
//!
//! # Why this is a crate and not a module
//!
//! The two crates it joins are each dependency-shaped in a way that forbids the
//! conversion living inside them.
//!
//! * `l2-kingdom` has **no loader, no I/O and no knowledge of any file format.**
//!   It takes plain data. That is not tidiness: it is the property that lets a
//!   lockstep peer, a replay, a mod and a test all build the same simulation by
//!   handing it values, and the moment it can open a file it has a second way to
//!   reach a state that the checksum never saw.
//! * `l2-formats` is **dependency-free on purpose** and parses bytes. Teaching
//!   it about counties and realms would make a format decoder depend on a
//!   simulation, which is the arrow pointing the wrong way.
//!
//! So the conversion belongs to neither, and it is small enough that a crate
//! costs almost nothing. `docs/plan.md`'s dependency diagram gains one leaf:
//! `l2-game ──► l2-scenario ──► {l2-formats, l2-kingdom}`, and nothing below
//! `l2-scenario` learns that the other side exists.
//!
//! # Two kingdoms, and they are different games
//!
//! [`Scenario::kingdom`] is the state **as the save holds it** — turn 1, Winter
//! 1268, the counties exactly as they were written. That is what "load a save"
//! means.
//!
//! [`Scenario::starting_kingdom`] is the position the save was taken **from**,
//! one season earlier, ready for [`l2_kingdom::Kingdom::start_new_game`]. That
//! is what "new game on the England map" means, and it is also what makes the
//! save an oracle: run the season forward and the result has to be the file.
//!
//! # What the save does not record
//!
//! One thing, and it is worth stating where a reader will find it rather than
//! burying it in a test. The file stores `popLast` and `happinessLast`, so the
//! previous season's population and happiness are recoverable exactly. It
//! stores **no previous herd and no previous grain** — the ration pass spent
//! some of both and only the result survives. [`Scenario::starting_kingdom`]
//! therefore carries the stored stores forward unchanged and says so, rather
//! than inverting the rules to manufacture a number that would make the
//! reproduction come out right. `crates/l2-kingdom/tests/reproduction.rs`
//! measures exactly what that costs: one county of fourteen.

use l2_formats::save::{Save, SaveError, COUNTY_BASE, COUNTY_STRIDE};
use l2_kingdom::county::{County, MAX_COUNTY_ID, MAX_FIELDS};
use l2_kingdom::map::MAP_TILES;
use l2_kingdom::merchant::{MerchantRoutes, ROUTES, ROUTE_SLOTS};
use l2_kingdom::realm::MAX_REALMS;
use l2_kingdom::tables::{health_band, Tables, Weather, JOB_COUNT};
use l2_kingdom::unit::{Mercenaries, TroopType, Unit, UnitKind, Units, MAX_UNITS, TROOP_TYPES};
use l2_kingdom::{field, land, CampaignMap, Kingdom, Options};

/// Where the nine labour records begin inside a county record, and how far
/// apart they are — `+0xC4`, stride `0x0C`, worker count at word 0.
///
/// **Read here rather than through [`l2_formats::save::County`]** because that
/// struct belongs to `l2-formats`, which is not this crate's to change; `Save`
/// exposes the addressed reads it is itself built from, so the seam can take
/// the word it needs without either crate learning about the other.
///
/// **`[V]`.** The layout is fixed by the labour allocator (`FUN_0044F6E7`),
/// which clears the block with
/// `for (c = 0; c < 9; c++) *(int *)(county + 0xC4 + c * 0x0C) = 0;` — nine
/// records, twelve bytes each, count first. The England turn-one fixture then checks
/// itself: county 1 holds 218 cattle farmers and 217 wood cutters against a
/// population of 435, county 2 holds 323 and 133 against 456, and **every one
/// of the fourteen sums to its population exactly**, which no wrong stride
/// would do fourteen times running. `docs/kingdom.md` §13.
const LABOUR_BASE: u32 = 0xC4;
const LABOUR_STRIDE: u32 = 0x0C;

/// `+0x130 + job*4` — eight `i32` percentages, and `+0x08` — one signed byte.
///
/// **`[V]`.** `FUN_00450000` recomputes them and indexes them as
/// `(&DAT_0053FAE0)[i * 4]` across two ranges, `0..3` and `3..8`; and both of
/// the binary's default setters (`FUN_004514F8`, `FUN_0045158B`) leave each
/// range summing to exactly 100.
const LABOUR_SHARE_BASE: u32 = 0x130;
const INDUSTRY_SHARE: u32 = 0x08;

/// `+0x1B0` — the castle-building switch `Industry_ToggleFromMap` flips and
/// `Labour_Allocate` gates castle building on.
const CASTLE_SWITCH: u32 = 0x1B0;

/// `g_countyFieldTiles` (`0x0053EA00`) — 17 × 20 × `u32`, **the map tiles that
/// are each county's fields**, stored as byte offsets into [`TILES`].
///
/// **`[V]`.** Save block 12 is 1,360 bytes = 17 × 80 exactly, and applying
/// `County_RecountFields`' terrain ladder to the tiles this names reproduces
/// all three stored field counts for all fourteen counties of the England
/// turn-one fixture — see `crates/l2-kingdom/tests/fields.rs`.
const COUNTY_FIELD_TILES: u32 = 0x0053_EA00;
const COUNTY_FIELD_STRIDE: u32 = 0x50;

/// `g_tiles` (`0x00522F90`) — 4,096 eight-byte tile records, `y * 64 + x`.
///
/// The three bytes `l2-kingdom` reads are `+0` terrain, `+1` flags and `+7`
/// county (`docs/symbols.md`, `docs/formats/maps-layers.md` §5.3). This is the
/// **first block in the save**, at file offset 0.
const TILES: u32 = 0x0052_2F90;
const TILE_STRIDE: u32 = 8;

/// One county's twenty field tiles, converted from byte offsets to tile
/// indices.
///
/// An offset that is not a multiple of eight, or that lands outside the
/// 64 × 64 plane, is a misread and not a field: it becomes an **empty slot**
/// rather than an out-of-bounds index. Nothing in the fixture takes that path
/// — `crates/l2-scenario/tests/import.rs` asserts every populated slot is a
/// real tile — and it is here so that a corrupt save loses a field instead of
/// panicking somewhere else later.
fn read_field_tiles(save: &Save, county: usize) -> Result<[u16; MAX_FIELDS], SaveError> {
    let base = COUNTY_FIELD_TILES + county as u32 * COUNTY_FIELD_STRIDE;
    let mut tiles = [0u16; MAX_FIELDS];
    for (slot, out) in tiles.iter_mut().enumerate() {
        let offset = save.i32_at(base + slot as u32 * 4)?;
        if offset <= 0 || offset % TILE_STRIDE as i32 != 0 {
            continue;
        }
        let index = offset / TILE_STRIDE as i32;
        if (index as usize) < MAP_TILES {
            *out = index as u16;
        }
    }
    Ok(tiles)
}

/// `g_tiles`' terrain, flags and county planes, de-interleaved out of the
/// eight-byte records.
fn read_map(save: &Save) -> Result<CampaignMap, SaveError> {
    let mut terrain = vec![0u8; MAP_TILES];
    let mut flags = vec![0u8; MAP_TILES];
    let mut county = vec![0u8; MAP_TILES];
    for tile in 0..MAP_TILES {
        let base = TILES + tile as u32 * TILE_STRIDE;
        terrain[tile] = save.u8_at(base)?;
        flags[tile] = save.u8_at(base + 1)?;
        county[tile] = save.u8_at(base + 7)?;
    }
    Ok(CampaignMap::from_planes(&terrain, &flags, &county)
        .expect("three planes of MAP_TILES bytes each"))
}

/// One word out of each of a county's nine labour records.
///
/// `word` is the byte offset inside the twelve-byte record: 0 is the workers
/// actually assigned, 4 the wanted floor, 8 the useful ceiling. All three are
/// read the same way because the record really is three plain `i32`s — which
/// is the whole reason the stride is twelve and not four.
fn read_labour(save: &Save, county: usize, word: u32) -> Result<[i32; JOB_COUNT], SaveError> {
    let base = COUNTY_BASE + (county * COUNTY_STRIDE) as u32 + LABOUR_BASE + word;
    let mut jobs = [0i32; JOB_COUNT];
    for (job, slot) in jobs.iter_mut().enumerate() {
        *slot = save.i32_at(base + job as u32 * LABOUR_STRIDE)?;
    }
    Ok(jobs)
}

/// The health meter every county starts a new game on.
///
/// **`[V]`, and it used to be `[I]`.** It was the one number in the whole
/// reproduction that came from prior art rather than from the binary or the
/// file, held in place only by the chain around it: 65 bands as 2, the Normal
/// ration delta for band 2 is +2, and the save stores a meter of 67 in band 3.
///
/// It is in the binary. `FUN_0049BD99` sets up a new game from a five-column
/// table at `0x004DC0D0 + startingWealth * 0x14`, and row 1 is
/// `{grain 0, herd 95, population 417, health 65, health 65}`. Two more of
/// those five turn up in `lastturn.sav` unchanged — `popLast` is 417 in all
/// fourteen counties and `+0x254` is 95 in all fourteen — so the row the
/// England turn-one scenario used is not in doubt. `tools/oracle/kingdom.ps1` checks the
/// table; `crates/l2-kingdom/tests/reproduction.rs` checks the save against it.
pub const STARTING_HEALTH_METER: i32 = 65;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportError {
    /// The save could not be read at all.
    Save(SaveError),
    /// More counties than `g_counties` can hold, or fewer than one.
    CountyCount(i32),
    /// An owner byte naming a realm that does not exist.
    Owner { county: usize, owner: u8 },
    /// A weather byte outside 0..=5. Refused rather than defaulted to Cloudy:
    /// a scenario we cannot read is not a scenario we should half-load.
    Weather { county: usize, byte: u8 },
    /// `g_localPlayer` outside 1..=5.
    LocalPlayer(i32),
    /// A season or year the clock cannot represent.
    Clock { season: i32, season_next: i32 },
    /// A neighbour id that is not a county on this map.
    Neighbour { county: usize, id: u8 },
    /// A unit whose type byte names none of the four handlers in
    /// `g_unitTickTable`. Slot 5 of that table is NULL and nothing spawns a
    /// type-5 unit, so a fifth value is a misread rather than a unit this code
    /// does not model yet.
    UnitKind { unit: usize, byte: u8 },
    /// A unit standing on no tile, or whose `+0x0C` tile offset disagrees with
    /// its `x`/`y`. **The offset is redundant on purpose** — it is
    /// `(y * 64 + x) * 8` — so a disagreement means the stride is wrong and
    /// every unit above this one is being read from the middle of its
    /// neighbour.
    UnitTile { unit: usize, x: u8, y: u8, offset: i32 },
    /// A unit owned by a realm that does not exist. Owner 6 is legal — it is
    /// what merchants and a county's own levied defence carry.
    UnitOwner { unit: usize, owner: u8 },
}

impl core::fmt::Display for ImportError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ImportError::Save(e) => write!(f, "{e}"),
            ImportError::CountyCount(n) => {
                write!(f, "{n} counties, and the array holds 1..={MAX_COUNTY_ID}")
            }
            ImportError::Owner { county, owner } => {
                write!(f, "county {county} is owned by realm {owner}, which is not a realm")
            }
            ImportError::Weather { county, byte } => {
                write!(f, "county {county} has weather byte {byte}, which names nothing")
            }
            ImportError::LocalPlayer(p) => write!(f, "g_localPlayer is {p}"),
            ImportError::Clock { season, season_next } => {
                write!(f, "season {season}, next {season_next}")
            }
            ImportError::Neighbour { county, id } => {
                write!(f, "county {county} borders {id}, which is not on this map")
            }
            ImportError::UnitKind { unit, byte } => {
                write!(f, "unit {unit} has type byte {byte}, which names no unit handler")
            }
            ImportError::UnitTile { unit, x, y, offset } => write!(
                f,
                "unit {unit} stands at ({x}, {y}) but its tile offset is {offset:#x}, \
                 and ({x}, {y}) is {:#x}",
                (*y as i32 * 64 + *x as i32) * 8
            ),
            ImportError::UnitOwner { unit, owner } => {
                write!(f, "unit {unit} is owned by realm {owner}, which is not a realm")
            }
        }
    }
}

impl std::error::Error for ImportError {}

impl From<SaveError> for ImportError {
    fn from(e: SaveError) -> ImportError {
        ImportError::Save(e)
    }
}

/// The clock, as plain numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Clock {
    pub season: u8,
    pub season_next: u8,
    pub year: i32,
    pub turn_count: u32,
}

/// One county's imported state. Plain values: no `Save`, no offsets, nothing
/// that remembers where it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CountyState {
    pub owner: u8,
    pub population: i32,
    /// `+0x28` — **the previous season's population**, and the reason the save
    /// can be rewound at all.
    pub population_last: i32,
    pub happiness: i32,
    /// `+0x0D` — the previous season's happiness.
    pub happiness_last: i32,
    pub shown_tax: i32,
    pub shown_ration: i32,
    pub shown_health: i32,
    pub shown_events: i32,
    pub d_hap_ration: i32,
    pub health_meter: i32,
    pub health_band: u8,
    pub unrest: u8,
    pub births: i32,
    pub deaths: i32,
    pub emigrants: i32,
    pub immigrants: i32,
    pub pop_band: i32,
    pub anchor: (u8, u8),
    pub neighbours: Vec<u8>,
    pub tax_rate: i32,
    pub tax_collected: i32,
    pub ration_wanted: i32,
    pub ration_achieved: i32,
    pub ration_split: i32,
    pub grain_eaten: i32,
    pub herd_eaten: i32,
    pub castle_type: u8,
    pub castle_building: u8,
    /// `+0x1B0` — the castle-building switch a click on the castle throws.
    /// See [`l2_kingdom::county::County::castle_switch`].
    pub castle_switch: bool,
    pub fields_fallow: i32,
    pub fields_cattle: i32,
    pub fields_grain: i32,
    pub fertility: i32,
    pub weather: Weather,
    pub dryness: i32,
    pub grain: i32,
    pub herd: i32,
    /// `+0xC4 + job * 0x0C` — the nine job records' worker counts.
    ///
    /// They were not imported at all until the herd needed them, and the herd
    /// needs them badly: `l2_kingdom::land::herd_growth` staffs a herd at three
    /// labourers a head and kills the shortfall, so a county imported with a
    /// labour of zero would lose cattle every season for want of a field this
    /// crate had simply not read. `docs/kingdom.md` §13.
    pub labour: [i32; JOB_COUNT],
    /// `+0xC4 + job * 0x0C + 0x04` and `+ 0x08` — the wanted floor and the
    /// useful ceiling of each record.
    ///
    /// The two words that were read as nothing until now. They are what the
    /// village screen draws its shortfall and surplus icons from, what the job
    /// popup colours its worker count by, and — for the ceiling — what the
    /// original's own labour allocator fills each job up to. See
    /// [`l2_kingdom::county::County::labour_wanted`].
    pub labour_wanted: [i32; JOB_COUNT],
    pub labour_useful: [i32; JOB_COUNT],
    /// `+0x130 + job*4` — the eight percentages the allocator splits each half
    /// of the county by, and `+0x08`, the percentage of the county that is
    /// industry rather than farm.
    ///
    /// Read here for the same reason the labour records are: they are the
    /// allocator's only inputs besides the ceilings, and with them the shipped
    /// save's own labour split can be recomputed from scratch and checked
    /// against what the file says. See [`l2_kingdom::labour`].
    pub labour_share: [i32; JOB_COUNT - 1],
    pub industry_share: i32,
    /// `g_countyFieldTiles + county * 0x50` — the twenty field tiles, as tile
    /// **indices** (the file's byte offsets divided by eight), 0 for an empty
    /// slot.
    ///
    /// Without these a county's fields cannot be repainted, because the three
    /// counts are a cache and there is nothing to recount from. See
    /// [`l2_kingdom::field`].
    pub field_tiles: [u16; MAX_FIELDS],
}

/// One realm's imported state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RealmState {
    pub in_play: bool,
    pub strength: u8,
    pub is_human: bool,
    pub lord: u8,
    pub county_count: u8,
    pub rank: u8,
    pub score: i32,
    pub gold: i32,
    pub wages: i32,
    pub iron: i32,
    pub stone: i32,
    pub wood: i32,
    pub weapons: [i32; l2_kingdom::tables::WEAPON_TYPE_COUNT],
}

/// A whole starting position, as plain data.
///
/// Nothing here is a `Save` and nothing here is a `Kingdom`. That is what makes
/// this the seam rather than a shortcut through it: a hand-written scenario, a
/// scenario editor or a future `.toml` can produce one of these without a game
/// install, and [`Scenario::kingdom`] is the only code that has to change if the
/// simulation's own shape does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scenario {
    pub county_count: usize,
    /// `g_localPlayer` — the realm a person drives.
    pub local_player: u8,
    /// `g_weatherCounty`, the county that had last season's local swing.
    pub weather_county: usize,
    pub options: Options,
    pub clock: Clock,
    /// Index 0 is never a county; the array is `1 ..= MAX_COUNTY_ID`.
    pub counties: Vec<Option<CountyState>>,
    /// Index 0 is never a realm.
    pub realms: Vec<RealmState>,
    /// `g_tiles`' three simulation planes — terrain, flags and county.
    ///
    /// **The map used to be left empty.** `Kingdom::new` builds
    /// [`l2_kingdom::CampaignMap::empty`] and nothing overwrote it, so every
    /// imported game ran its pathfinding, its field-crossing and its trampling
    /// against 4,096 blank tiles. The planes are in the save — `g_tiles` is
    /// block 0 — and they are read here.
    pub map: CampaignMap,
    /// `g_units` — **armies, revolting peasants, merchants and transports**, as
    /// `(slot, unit)` pairs in ascending slot order.
    ///
    /// **The whole block used to be dropped on the floor.** Every other layer
    /// was ready for it — `l2_kingdom::unit` models the record, `movement` walks
    /// it, `conquest` fights with it — and the only units that had ever existed
    /// were the ones tests built by hand. So a loaded England position had a
    /// working economy and an empty map.
    ///
    /// Slots are carried rather than compacted because they are *referenced*:
    /// county `garrison_unit`, a garrison's `besieged_by` and a mercenary band's
    /// `hired_by` all name a slot, and `Merchant_AdvanceAll` indexes the route
    /// table by slot. Renumber on import and the six merchants walk each other's
    /// routes.
    ///
    /// Stored as [`l2_kingdom::unit::Unit`] rather than as a plain mirror of it,
    /// on the same grounds as [`Scenario::map`]: the record has forty fields and
    /// a second copy of it here would be forty more places to drop one.
    pub units: Vec<(usize, Unit)>,
    /// `g_merchantRoutes` (`0x00567970`) — the six trade itineraries, which are
    /// what makes a merchant walk anywhere at all.
    ///
    /// `l2-kingdom` has had [`MerchantRoutes`] and `Merchant_AdvanceAll` since
    /// the turn movers landed, and nothing but a test had ever filled the table:
    /// a game loaded from a save arrived with six empty rows and six merchants
    /// that could never work out where to go. It is in the save, inside a block,
    /// and it is read here.
    pub routes: MerchantRoutes,
    /// `g_merchantStartCounty` (`0x00569518`) — the county each route's merchant
    /// was **spawned** in, which is not where it is now.
    ///
    /// Carried on the scenario rather than in [`MerchantRoutes`] because nothing
    /// in the simulation reads it: `Merchant_PickStartCounties` and
    /// `Merchant_SpawnAll` both ran before the save was written, and their
    /// output is already in the unit array. What it is good for is *checking*
    /// that reading — every merchant's `+0x167` is its own entry here, on every
    /// save the fixture set holds — which is a test's business and not a rule's.
    pub merchant_start: [u8; ROUTES],
}

/// One `g_units` record, checked and converted.
///
/// Three refusals, and each of them means "the array is being read wrong"
/// rather than "this save is unusual":
///
/// * a type byte outside 1…4 — `g_unitTickTable`'s fifth slot is NULL and
///   nothing spawns a type-5 unit;
/// * an owner above 6 — 1…5 are realms and **6 is nobody**, which is what a
///   merchant and a county's own levied defence carry;
/// * a `+0x0C` that disagrees with `x`/`y`. That one is the **self-checking
///   invariant**: the field is `(y * 64 + x) * 8`, and nothing but the right
///   stride makes it agree on every occupied slot of every save.
fn read_unit(u: &l2_formats::save::Unit) -> Result<Unit, ImportError> {
    let kind =
        UnitKind::from_byte(u.kind).ok_or(ImportError::UnitKind { unit: u.index, byte: u.kind })?;
    if u.owner as usize > MAX_REALMS {
        return Err(ImportError::UnitOwner { unit: u.index, owner: u.owner });
    }
    if !u.tile_offset_agrees() {
        return Err(ImportError::UnitTile { unit: u.index, x: u.x, y: u.y, offset: u.tile_offset });
    }
    let mut troops = [0i32; TROOP_TYPES];
    for (t, slot) in troops.iter_mut().enumerate() {
        *slot = u.troops[t] as i32;
    }
    Ok(Unit {
        owner: u.owner,
        owner_is_human: u.owner_is_human,
        shield: u.shield,
        player_driven: u.player_driven,
        kind,
        facing: u.facing,
        x: u.x,
        y: u.y,
        county: u.county,
        home_county: u.home_county,
        // The original has no "no destination" encoding for `+0x16`/`+0x17` —
        // the pair is always a tile, and `needs_destination` is the bit that
        // says whether it means anything. Carried as `Some` for that reason:
        // dropping it while the unit is idle would lose the tile the info panel
        // still draws.
        dest: Some((u.dest_x, u.dest_y)),
        path: u.path(),
        moving: u.move_state != 0,
        on_road: u.on_road,
        name_index: u.name_index,
        needs_destination: u.needs_destination,
        dest_county: u.dest_county,
        moves_used: u.moves_used as i32,
        // **Taken from the file, not from the type.** Each type's tick handler
        // rewrites this unconditionally, so the six shipped merchants carry 0
        // and a defence raised mid-turn carries 0; an importer that wrote 15 or
        // 10 here would be inventing a number the game had not got round to.
        // `docs/armies.md` §8b.4.
        move_allowance: u.move_allowance as i32,
        starvation: u.starvation as i32,
        wages: u.wages,
        // `+0x164`. A merchant keeps its **route cursor** in the low byte of the
        // same field; `l2_kingdom::merchant::cursor_of` is the reading that
        // knows which is which.
        year_formed: u.year_formed as i32,
        morale: u.morale as i32,
        men: u.men,
        troops,
        // `+0x195…+0x197`. A band id of 0 is no band, and the men and the troop
        // type mean nothing without it.
        mercenaries: (u.merc_band != 0)
            .then(|| {
                TroopType::from_index(u.merc_troop as usize)
                    .map(|troop| Mercenaries { band: u.merc_band, troop, men: u.merc_men })
            })
            .flatten(),
        garrison_county: u.garrison_county,
        besieging_county: u.besieging_county,
        besieged_by: u.besieged_by,
        // **`+0x167` is one byte and `Unit` models it as two fields**, because
        // the meanings have nothing to do with each other: the county-defence
        // mark on an army or a mob, the cargo county on a transport, and — this
        // one is neither — the county a *merchant* was spawned in, which
        // `Merchant_SpawnAll` writes once and nothing ever updates.
        //
        // So the byte goes to the field its type gives it, and the merchant's
        // birthplace lands in `cargo_county` beside the transport's destination
        // because that is the non-army half of the alias. Nothing reads it back
        // for a merchant; `Scenario::merchant_start` is what checks it.
        defence_mark: if kind.is_combatant() { u.role } else { 0 },
        cargo_county: if kind.is_combatant() { 0 } else { u.role },
        // The siege build records and the countdown are not read out of the save
        // yet - the unit block holds them and `l2-formats` does not surface them -
        // so an imported army starts with no engines ordered. A besieging army
        // imported mid-build therefore resumes at zero work, which is wrong and
        // is recorded here rather than hidden: it needs the three `+0x16C` words
        // and the countdown adding to `l2_formats::save`.
        engines: Default::default(),
        siege_seasons_left: 0,
    })
}

impl Scenario {
    /// Read a England turn-one save into plain data.
    ///
    /// Every refusal here is a refusal rather than a default. A save whose owner
    /// byte names realm 9 is not a save with a small problem; it is a save this
    /// code has misread, and the arithmetic in [`Save::open`] having closed is
    /// exactly why a surprise at this level should stop rather than be papered
    /// over.
    pub fn from_save(save: &Save) -> Result<Scenario, ImportError> {
        let g = save.globals()?;

        let county_count = g.county_count;
        if county_count < 1 || county_count > MAX_COUNTY_ID as i32 {
            return Err(ImportError::CountyCount(county_count));
        }
        let county_count = county_count as usize;

        if g.local_player < 1 || g.local_player >= MAX_REALMS as i32 {
            return Err(ImportError::LocalPlayer(g.local_player));
        }
        let local_player = g.local_player as u8;

        if !(1..=4).contains(&g.season) || !(1..=4).contains(&g.season_next) {
            return Err(ImportError::Clock { season: g.season, season_next: g.season_next });
        }

        let stored = save.counties()?;
        let mut counties: Vec<Option<CountyState>> = vec![None; MAX_COUNTY_ID as usize + 1];
        for c in stored.iter().take(county_count + 1).skip(1) {
            if c.owner as usize >= MAX_REALMS {
                return Err(ImportError::Owner { county: c.index, owner: c.owner });
            }
            let weather = Weather::from_index(c.weather)
                .ok_or(ImportError::Weather { county: c.index, byte: c.weather })?;
            let mut neighbours = Vec::with_capacity(c.neighbours().len());
            for &id in c.neighbours() {
                if id as usize == c.index || id < 1 || id as usize > county_count {
                    return Err(ImportError::Neighbour { county: c.index, id });
                }
                neighbours.push(id);
            }
            counties[c.index] = Some(CountyState {
                owner: c.owner,
                population: c.population,
                population_last: c.pop_last,
                happiness: c.happiness as i32,
                happiness_last: c.happiness_last as i32,
                shown_tax: c.shown_tax as i32,
                shown_ration: c.shown_ration as i32,
                shown_health: c.shown_health as i32,
                shown_events: c.shown_events as i32,
                d_hap_ration: c.d_hap_ration as i32,
                health_meter: c.health_meter as i32,
                health_band: c.health_band.max(0) as u8,
                unrest: c.unrest,
                births: c.births,
                deaths: c.deaths,
                emigrants: c.emigrants,
                immigrants: c.immigrants,
                pop_band: c.pop_band as i32,
                anchor: (c.anchor_x, c.anchor_y),
                neighbours,
                tax_rate: c.tax_rate as i32,
                tax_collected: c.tax_collected,
                ration_wanted: c.ration_wanted as i32,
                ration_achieved: c.ration_achieved as i32,
                ration_split: c.ration_split as i32,
                grain_eaten: c.grain_eaten,
                herd_eaten: c.herd_eaten,
                castle_type: c.castle_type,
                castle_building: c.castle_building,
                castle_switch: save
                    .u8_at(COUNTY_BASE + (c.index * COUNTY_STRIDE) as u32 + CASTLE_SWITCH)?
                    != 0,
                fields_fallow: c.fields_fallow as i32,
                fields_cattle: c.fields_cattle as i32,
                fields_grain: c.fields_grain as i32,
                fertility: c.fertility,
                weather,
                dryness: c.dryness as i32,
                grain: c.grain,
                herd: c.herd,
                labour: read_labour(save, c.index, 0)?,
                labour_wanted: read_labour(save, c.index, 4)?,
                labour_useful: read_labour(save, c.index, 8)?,
                labour_share: {
                    let base = COUNTY_BASE + (c.index * COUNTY_STRIDE) as u32 + LABOUR_SHARE_BASE;
                    let mut shares = [0i32; JOB_COUNT - 1];
                    for (job, share) in shares.iter_mut().enumerate() {
                        *share = save.i32_at(base + job as u32 * 4)?;
                    }
                    shares
                },
                industry_share: save
                    .i8_at(COUNTY_BASE + (c.index * COUNTY_STRIDE) as u32 + INDUSTRY_SHARE)?
                    as i32,
                field_tiles: read_field_tiles(save, c.index)?,
            });
        }

        let realms = save
            .realms()?
            .iter()
            .map(|r| RealmState {
                in_play: r.in_play(),
                strength: r.strength,
                is_human: r.is_human,
                lord: r.lord,
                county_count: r.county_count,
                rank: r.rank,
                score: r.score,
                gold: r.gold,
                wages: r.wages,
                iron: r.iron,
                stone: r.stone,
                wood: r.wood,
                weapons: {
                    let mut w = [0i32; l2_kingdom::tables::WEAPON_TYPE_COUNT];
                    let n = w.len().min(r.weapons.len());
                    w[..n].copy_from_slice(&r.weapons[..n]);
                    w
                },
            })
            .collect();

        Ok(Scenario {
            county_count,
            local_player,
            weather_county: g.weather_county.clamp(1, county_count as i32) as usize,
            options: Options {
                difficulty: g.opt_difficulty.clamp(0, 3) as u8,
                advanced_farming: g.opt_advanced_farming != 0,
                armies_eat: g.opt_armies_eat != 0,
                // **NOT read from the save, and now we know why.** This used to
                // say `l2-formats` merely did not expose `g_optFightHumansOnly`
                // (0x0053F284). Adding it to that list turned the battle
                // fixtures red with `NotSaved`, and the block table says the
                // reason: **the original does not save this option.** Seven
                // four-byte entries cover 0x0053F23C, F258, F25C, F260, F264,
                // F268 and F26C, and 0x0053F284 is in none of them.
                //
                // So an imported game *cannot* know what it was played with,
                // and neither can the original — reloading takes whatever is in
                // memory. Taking the game's default is the honest answer to a
                // question the file does not answer. `docs/bugs.md`.
                fight_humans_only_byte: l2_kingdom::battle::FIGHT_HUMANS_ONLY_DEFAULT,
                exploration: g.opt_exploration != 0,
                time_limit: g.opt_time_limit,
            },
            clock: Clock {
                season: g.season as u8,
                season_next: g.season_next as u8,
                year: g.year,
                turn_count: g.turn_count.max(0) as u32,
            },
            counties,
            realms,
            map: read_map(save)?,
            units: {
                let mut units = Vec::new();
                for u in save.units()?.iter().filter(|u| u.is_live()) {
                    units.push((u.index, read_unit(u)?));
                }
                units
            },
            routes: {
                let rows = save.merchant_routes()?;
                let mut routes = MerchantRoutes::none();
                for (row, cells) in rows.iter().enumerate().take(ROUTES) {
                    let mut slots = [0u8; ROUTE_SLOTS];
                    slots.copy_from_slice(&cells[..ROUTE_SLOTS]);
                    routes.set_row(row, slots);
                }
                routes
            },
            merchant_start: save.merchant_start_counties()?,
        })
    }

    /// The counties that exist, ascending. Never a hash, never sorted — the ids
    /// are `1 ..= county_count` and that is the iteration order everywhere in
    /// this project (`docs/netcode.md` D-4).
    pub fn county_ids(&self) -> core::ops::RangeInclusive<usize> {
        1..=self.county_count
    }

    /// The state **as the save holds it**: load-a-save.
    pub fn kingdom(&self, seed: u64) -> Kingdom {
        self.kingdom_with_tables(seed, Tables::DEFAULT)
    }

    /// The same, on a supplied ruleset — how a mod reaches an imported
    /// scenario. The kingdom never learns which of the two it got.
    pub fn kingdom_with_tables(&self, seed: u64, tables: Tables) -> Kingdom {
        let mut k = self.skeleton(seed, tables);
        k.season = self.clock.season;
        k.season_next = self.clock.season_next;
        k.season_prev = if self.clock.season == 1 { 4 } else { self.clock.season - 1 };
        k.year = self.clock.year;
        k.year_next = self.clock.year + 1;
        k.turn_count = self.clock.turn_count;

        for id in self.county_ids() {
            let Some(s) = &self.counties[id] else { continue };
            let c = &mut k.counties[id];
            c.population = s.population;
            c.pop_last = s.population_last;
            c.happiness = s.happiness;
            c.happiness_last = s.happiness_last;
            c.happiness_sum = s.happiness;
            c.happiness_avg = s.happiness;
            c.shown_tax = s.shown_tax;
            c.shown_ration = s.shown_ration;
            c.shown_health = s.shown_health;
            c.shown_events = s.shown_events;
            c.d_hap_ration = s.d_hap_ration;
            c.health_meter = s.health_meter;
            c.health_band = s.health_band;
            c.unrest = s.unrest;
            c.births = s.births;
            c.deaths = s.deaths;
            c.emigrants = s.emigrants;
            c.immigrants = s.immigrants;
            c.pop_band = s.pop_band;
            c.tax_collected = s.tax_collected;
            c.ration_achieved = s.ration_achieved;
            c.grain_eaten = s.grain_eaten;
            c.herd_eaten = s.herd_eaten;
            c.grain_available = s.grain;
            c.herd_available = s.herd;
        }
        k
    }

    /// The position the save was taken **from**: one season earlier, ready for
    /// [`Kingdom::start_new_game`].
    ///
    /// Population and happiness are rewound to the values the file records for
    /// the previous season, and the health meter to
    /// [`STARTING_HEALTH_METER`]. Everything the season *computes* — births,
    /// deaths, the display copies, the bands — is left at zero, because a
    /// starting position that already carried last season's outputs would let a
    /// broken pass pass by leaving them alone.
    ///
    /// **The food stores are the ones the save holds**, because the save holds
    /// no earlier ones. See the module documentation.
    pub fn starting_kingdom(&self, seed: u64) -> Kingdom {
        self.starting_kingdom_with_tables(seed, Tables::DEFAULT)
    }

    pub fn starting_kingdom_with_tables(&self, seed: u64, tables: Tables) -> Kingdom {
        let mut k = self.skeleton(seed, tables);
        for id in self.county_ids() {
            let Some(s) = &self.counties[id] else { continue };
            let c = &mut k.counties[id];
            c.population = s.population_last;
            c.happiness = s.happiness_last;
            c.health_meter = STARTING_HEALTH_METER;
            c.health_band = health_band(STARTING_HEALTH_METER) as u8;
            c.ration_achieved = s.ration_wanted;
        }
        k
    }

    /// Everything both kingdoms share: the map, who owns what, the options and
    /// the realms. Split out so the two constructors cannot drift apart.
    fn skeleton(&self, seed: u64, tables: Tables) -> Kingdom {
        let mut k = Kingdom::with_tables(seed, tables);
        k.campaign.map = self.map.clone();
        k.campaign.routes = self.routes.clone();
        // **Slots, not order.** `Units::put` writes the slot the save recorded;
        // `Units::spawn` would take the lowest free one and quietly renumber
        // everything the moment a save had a hole in its array.
        let mut units = Units::new();
        for (slot, unit) in &self.units {
            if *slot < MAX_UNITS {
                units.put(*slot, unit.clone());
            }
        }
        k.campaign.units = units;
        k.options = self.options;
        k.weather_county = self.weather_county;
        assert!(
            k.set_county_count(self.county_count),
            "from_save bounds the county count before it is stored"
        );

        for (id, r) in self.realms.iter().enumerate().take(MAX_REALMS).skip(1) {
            let realm = &mut k.realms[id];
            realm.in_play = r.in_play;
            realm.strength = r.strength;
            realm.is_human = r.is_human || id == self.local_player as usize;
            realm.lord = r.lord;
            realm.county_count = r.county_count;
            realm.rank = r.rank;
            realm.score = r.score;
            realm.gold = r.gold;
            realm.wages = r.wages;
            realm.iron = r.iron;
            realm.stone = r.stone;
            realm.wood = r.wood;
            realm.weapons = r.weapons;
        }

        for id in self.county_ids() {
            let Some(s) = &self.counties[id] else { continue };
            let c = &mut k.counties[id];
            *c = County::new();
            c.owner = s.owner;
            c.anchor_x = s.anchor.0;
            c.anchor_y = s.anchor.1;
            for &n in &s.neighbours {
                c.add_neighbour(n);
            }
            c.tax_rate = s.tax_rate;
            c.ration_wanted = s.ration_wanted;
            c.ration_split = s.ration_split;
            c.castle_type = s.castle_type;
            c.castle_building = s.castle_building;
            c.castle_switch = s.castle_switch;
            c.field_tiles = s.field_tiles;
            c.fertility = s.fertility;
            c.weather = s.weather;
            c.dryness = s.dryness;
            c.grain = s.grain;
            c.herd = s.herd;
            c.labour = s.labour;
            c.labour_wanted = s.labour_wanted;
            c.labour_useful = s.labour_useful;
            c.labour_share = s.labour_share;
            c.industry_share = s.industry_share;
            // `FUN_0044D913` is called from everywhere a county's herd or
            // pasture can change, county setup included, so a county always
            // arrives with its crowding already computed. Deriving it here
            // rather than reading `+0x25C` keeps the two consistent — and the
            // reproduction test checks the derived value against the byte.
            // **The five field counts are derived, not imported.** The file
            // stores them, `CountyState` carries what it stored, and the
            // kingdom gets what `County_RecountFields` makes of the twenty
            // field tiles — two independent readings that
            // `crates/l2-kingdom/tests/fields.rs` diffs against each other on
            // all fourteen counties. Doing it the other way round would leave
            // the counts and the tiles free to disagree the first time a field
            // was repainted. Recounted here rather than after the loop because
            // the herd's crowding on the next line reads `fields_cattle`.
            field::recount(c, &self.map);
            c.herd_crowding = land::herd_crowding(&tables, c.herd, c.fields_cattle);
        }
        k
    }
}
