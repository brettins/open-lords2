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
//! handing it values, and the moment it can open a file it has a second way to
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
//! 1268, the counties. That is what "load a save"
//! means.
//!
//! [`Scenario::starting_kingdom`] is the position the save was taken **from**,
//! one season earlier, ready for [`l2_kingdom::Kingdom::start_new_game`]. That
//! is what "new game on the England map" means, and it is also what makes the
//! save an oracle: run the season forward and the result has to be the file.
//!
//! # What the save does not record
//!
//! One thing
//! burying it in a test. The file stores `popLast` and `happinessLast`, so the
//! previous season's population and happiness are recoverable exactly. It
//! stores **no previous herd and no previous grain** — the ration pass spent
//! some of both and only the result survives. [`Scenario::starting_kingdom`]
//! therefore carries the stored stores forward unchanged and says so, rather
//! than inverting the rules to manufacture a number that would make the
//! reproduction come out right. `crates/l2-kingdom/tests/reproduction.rs`
//! measures exactly what that costs: one county of fourteen.

//!
//! # Two constructors, not one
//!
//! [`Scenario::from_save`] reads a world the original had already built.
//! [`Scenario::from_map`] — [`newgame`] — **builds** one, out of an
//! `L2_maps.dat` slot and the twelve custom-game settings.
//! *New Game* button needs. Both produce the same plain data, so
//! [`Scenario::starting_kingdom`] cannot tell them apart and
//! `crates/l2-scenario/tests/newgame.rs` can diff England built both ways.

mod map;
pub use map::*;
mod save_helpers;
pub use save_helpers::*;
mod units;
pub use units::*;
mod scenario_impl;
pub use scenario_impl::*;

pub mod newgame;

use l2_formats::save::{Save, SaveError, COUNTY_BASE, COUNTY_STRIDE, REALM_BASE, REALM_STRIDE};
use l2_kingdom::county::{County, MAX_COUNTY_ID, MAX_FIELDS};
use l2_kingdom::explore::Explored;
use l2_kingdom::map::MAP_TILES;
use l2_kingdom::merchant::{MerchantRoutes, ROUTES, ROUTE_SLOTS};
use l2_kingdom::mercenary::{Band, MercenaryBands, MERCENARY_BANDS, ROSTER};
use l2_kingdom::realm::MAX_REALMS;
use l2_kingdom::tables::{health_band, Tables, Weather, JOB_COUNT};
use l2_kingdom::unit::{Mercenaries, TroopType, Unit, UnitKind, Units, MAX_UNITS, TROOP_TYPES};
use l2_kingdom::{field, land, CampaignMap, Kingdom, Options};

/// Where the nine labour records begin inside a county record, and how far
/// apart they are — `+0xC4`, stride `0x0C`, worker count at word 0.
///
/// **Read here** because that
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

/// **`+0x0F`, `+0x10` and `+0xC0` — the three numbers the county panels read
/// out of the county record and nothing here imported.** `docs/decisions.md`
/// C142.
///
/// `Tax_RecomputePreview` (`0x0044B80B`) writes `+0x0F` (`dHapTaxLocal`) and
/// `+0xC0` (`taxShown`); `Health_Apply`'s pass writes `+0x10`
/// (`dHapHealth`). `Panel_Tax` (`0x0041152F`) draws `realm+0x28 + county+0x0F`
/// on its *This county* line and `Ui_DrawCount(county+0xC0, …)` on *People
/// pay*; `Panel_Ration` (`0x00411B72`) draws `+0x10` beside the health band.
/// Every one of them was `County::new()`'s zero on a loaded game,
/// opened tax panel said *"People pay 0 crowns"* whatever the rate, and both
/// happiness lines read `( 0 ☺ )`.
///
/// **`[V]` against the original's own bytes, in twelve saves.** `+0x0F` is
/// `5 - taxRate` in every owned county of every save on this machine — 5 at
/// rate 0, 2 at rate 3, −1 at rate 6, −3 at rate 8 — which is
/// `Tax_RecomputePreview`'s second statement exactly. And `+0xC0` is
/// `Pct(Pct(population, castleBase), taxRate)` to the unit in eight of the
/// nine counties that carry a rate above zero: `sieging.sav` county 1 stores
/// **147** for 767 people at rate 6 with no castle, `safeturn.sav` county 2
/// stores **283** for 738 at rate 8 behind a wooden castle. The ninth is
/// `incombat.sav`, whose population fell while the battle was open and whose
/// stored 245 is the *pre-battle* population's answer — which is the point of
/// reading the byte.
///
/// **These are also the project's first oracle for a non-zero tax rate.**
/// `docs/plan.md` §2.5 says every county in every fixture sits at rate 0; that
/// is true of the England fixture and false of the turn pair and the six siege
/// saves, which carry rates 2, 3, 6 and 8. It is still true of anything above
/// 19, where `g_taxHappinessOther` starts to bite.
const D_HAP_TAX_LOCAL: u32 = 0x0F;
const D_HAP_HEALTH: u32 = 0x10;
const TAX_SHOWN: u32 = 0xC0;

/// `+0x1FE` — the county's farming style, which is
/// [`l2_kingdom::county::County::farm_style`].
///
/// **This was not imported at all until the map-file constructor needed it,
/// and that is a defect of its own.** `AI_ManageFields(0)` dispatches the
/// *unowned* counties on it and `Ai_ManageCountyFarms` overwrites it with the
/// owning lord's style every pass —
/// arrived at style 0 and every neutral county farmed as a style-0 lord would,
/// whatever the file said. `County_Reset` (`0x00451150`) seeds it to
/// `countyId & 1` at new game, and `docs/symbols.md` records that the England
/// turn-one fixture holds only 0, 1 and 9 here — the exact value set
/// `AI_PERSONALITY_FARM_STYLE` produces, which is what says the offset is
/// right.
const FARM_STYLE: u32 = 0x1FE;

/// `+0x1F4` — **an unowned county's own treasury**, and `+0x1A4` / `+0x1A5` /
/// `+0x1A0` — its merchant stall: how many merchants are standing in it, the
/// slot of the last one counted, and the lifetime visit total.
///
/// **`[V]`, and read straight out of the fixtures.** `Tax_CollectAll`
/// (`0x0044B59B`) banks a lordless county's tax into `+0x1F4`, and `Ai_BuyGood`
/// (`0x004A4B12`) refuses every purchase unless `+0x1A4` is non-zero and then
/// prices the goods by `g_units[+0x1A5].morale`. Those four bytes are therefore
/// the whole of whether a county nobody owns can feed itself.
///
/// None of them was imported,
/// **fifty sacks of grain a season, per unowned county, in perpetuity.** The
/// six one-turn-apart saves carry `+0x1F4` at 186, 297, 260 (county 1), 195,
/// 316, 294 (county 3) and 436 (siege county 3), and `+0x1A4` non-zero on every
/// county holding a merchant. `l2_kingdom::county::County::purse`'s own comment
/// asserted *"it is 0 in every fixture"*, which was true of `england-turn1.sav`
/// and of nothing else. `docs/decisions.md` C149.
///
/// **The stall is imported**, even though
/// `l2_kingdom::merchant::recount_all` could derive it, because the first thing
/// a loaded game runs is turn phase 1 — the neutral farming pass —
/// recount is season pass 22, at the *end* of that same turn. The original
/// reads these bytes back out of the file; so does this.
const PURSE: u32 = 0x1F4;
const MERCHANT_VISITS: u32 = 0x1A0;
const MERCHANT_COUNT: u32 = 0x1A4;
const MERCHANT_UNIT: u32 = 0x1A5;

/// `g_mercBands` (`0x00568DC0`) — **the twelve mercenary bands' live table**,
/// stride `0x14`, slot 0 never a band — and `g_mercBandsInPlay`
/// (`0x00554030`), how many of them this map uses.
///
/// **Neither was read, ** The
/// kingdom arrived with `MercenaryBands::none()`: no band in play, nothing to
/// walk, and `Mercenary_AdvanceAll` a no-op for the rest of the game — so the
/// raise-army screen and the town square never showed one,
/// on any save, for ever.
///
/// **`[V]`, three ways, none of them resemblance to the roster.**
///
/// * **The block closes.** `Save_Write`'s table saves `0x00568DC0` as a block of
///   exactly **260** bytes, which is `13 × 0x14`: slot 0 and twelve bands, and
///   not a byte either side.
/// * **The six constant fields are the roster, in every band of every save.**
///   `Mercenary_Init` (`0x004AC904`) copies `+0x02` start county, `+0x05` troop,
///   `+0x07` reload, `+0x08` men, `+0x0C` price and `+0x10` the unread wage out
///   of six static arrays; all eighteen saves on this machine hold exactly
///   [`l2_kingdom::mercenary::ROSTER`] there, and every slot past the in-play
///   count is zero. [`read_mercenaries`] refuses a save where they disagree,
///   because the kingdom keeps those six in the roster and
///   would otherwise replace the file's values in silence.
/// * **County `+0x1AD` is `Mercenary_OfferInCounty` over this table**, in every
///   county of every save — including `siege-old_turn.sav`, where bands 2 and 3
/// both stand in county 1 and the cache holds **2**, the lower-numbered one.
///   And **one season of `Mercenary_AdvanceAll` over each of the four one-turn
///   pairs on disk lands on the next save's table exactly.**
///   `crates/l2-scenario/tests/import.rs` asserts all of it.
///
/// What no save settles: `+0x00`, the hiring unit, is **zero in every band of
/// every save** — nobody on this machine ever hired one — so the importer's
/// check that a hirer carries its band has only been exercised by a test that
/// writes the byte.
const MERCENARY_BANDS_VA: u32 = 0x0056_8DC0;
const MERCENARY_BAND_STRIDE: u32 = 0x14;
const MERCENARY_BANDS_IN_PLAY: u32 = 0x0055_4030;

/// `+0x294 + c*0x18` — the four industry records, the three bytes of each that
/// say whether it can run — `+1` the resource, `+2` the countdown, `+3` the
/// switch — and `+0x14`, the forecast the sidebar's row draws. Commodity order
/// **wood, iron, weapons, stone**, which is `Industry_ToggleFromMap`'s own
/// numbering.
///
/// **The base was `+0x290`, with every offset four higher.** That read the same
/// three bytes, and could not have named the fourth field, because under it
/// `+0x2A8 + c*0x18` is the head of the *next* record. `+0x290` is the county's
/// `weaponType` byte; the array runs `+0x294 … +0x2F3` and closes on
/// `levySurcharge`. `docs/decisions.md` C153.
///
/// **`[V]`, and it checks itself against the map.** `County_PlaceResourceSites`
/// (`0x00468E61`) sets `+0x295` from the county's `Town`-bank tiles, so the byte
/// and the terrain have to agree: an industry's `hasResource` is 1
/// the county owns a settlement tile whose terrain is in that industry's rung of
/// `Map_Click`'s ladder (0…3 iron, 4…6 stone, 7…9 weapons, 10…12 wood). Over the
/// England turn-one fixture that is **56 of 56** — fourteen counties, four
/// industries each — with iron and stone complementary in thirteen of the
/// fourteen and county 5 having neither. `crates/l2-scenario/tests/import.rs`
/// asserts it.
///
/// Until this was read, every county arrived with [`Industry::has_resource`] and
/// [`Industry::enabled`] defaulted to `true`, so **no county ever
/// showed a mine**: the village picks the mine for cluster 0 only when the
/// county has a mine *and no quarry*, and a county that claims both is a county
/// with a quarry. `docs/decisions.md` C57.
const INDUSTRY_BASE: u32 = 0x294;
const INDUSTRY_STRIDE: u32 = 0x18;
const INDUSTRY_HAS_RESOURCE: u32 = 1;
const INDUSTRY_DISABLED_SEASONS: u32 = 2;
const INDUSTRY_ENABLED: u32 = 3;
const INDUSTRY_NEXT_SEASON: u32 = 0x14;

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

/// `g_tiles[t].bank` (record `+2`) bit `0x20` — **the fog of war's seen bit**,
/// and whose it is.
///
/// Every one of the bit's writers tests `g_localPlayer` (`l2_kingdom::explore`
/// has the table), so the file says what the local player has seen and nothing
/// about anybody else. It is kept whatever `g_optExploration` says, which the
/// England fixture shows: saved with the option off, and the local player's
/// county carries the bit — `crates/l2-scenario/tests/explored.rs`.
const TILE_BANK: u32 = 2;
const BANK_SEEN: u8 = 0x20;

/// One county's four industry records, reduced to the three bytes that decide
/// whether the industry can run at all and the forecast the sidebar draws. See
/// [`INDUSTRY_BASE`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct IndustryState {
    pub has_resource: bool,
    pub disabled_seasons: i32,
    pub enabled: bool,
    /// Record `+0x14`, county `+0x2A8 + c*0x18` — `Industry_LabourEstimate`'s
    /// (`0x0044F318`) tail, and the word `Ui_DrawDelta` draws on the row.
    ///
    /// **Read, not recomputed on load**, for C142's reason: the original
    /// restores a memory image and runs `County_RefreshEstimates` at the end of
    /// a season or on a control, not on a load. Left out, a loaded game drew
    /// **no** industry forecast in any county until its first season ended —
    /// every one of the four at `Industry::new()`'s zero, which `Ui_DrawDelta`
    /// with `mode == 0` draws as nothing at all.
    pub next_season: i32,
    /// Record `+0x00`, `+0x0A`, `+0x0C` and `+0x10` — the efficiency the ramp
    /// has reached, the workers it absorbs at full value, and the running total
    /// with its last snapshot, whose difference is the season's output.
    ///
    /// **Dropped until C161**, and not quietly: every owned county
    /// in every save stores an efficiency of **80**, and a loaded game arrived at
    /// `Industry::new()`'s 20 for wood and 15 for the rest — so the production
    /// pass ran a mid-game county's mines as if they had just opened.
    /// `docs/stored-fields.json`.
    pub efficiency: i32,
    pub capacity: i32,
    pub total: i32,
    pub total_snapshot: i32,
}

/// **The offsets C161 imports**, one constant each, and nothing
/// else. Every one is a row of `docs/stored-fields.json` with status
/// `imported`, and `crates/l2-scenario/tests/stored_fields.rs` holds each to the
/// file's own bytes after [`Scenario::kingdom`] in every save on the machine —
/// so the prose for each lives in that inventory and in the kingdom field's own
/// doc comment, not a third time here.
mod stored {
    pub const EVENT_FIRED: u32 = 0x000;
    pub const D_HAP_TAX: u32 = 0x00E;
    pub const SHOWN_ARMY: u32 = 0x015;
    pub const TAX_HAP_OTHER: u32 = 0x016;
    pub const HAPPINESS_AVG: u32 = 0x018;
    pub const HAPPINESS_SUM: u32 = 0x01C;
    pub const UNREST_WARNED: u32 = 0x021;
    pub const POP_CHANGE_PCT: u32 = 0x02C;
    pub const POP_ARMY: u32 = 0x038;
    pub const LARGEST_INFLOW: u32 = 0x044;
    pub const INFLOW_SOURCES: u32 = 0x048;
    pub const EMIGRANT_DESTINATION: u32 = 0x058;
    pub const LARGEST_INFLOW_SOURCE: u32 = 0x059;
    pub const CHANGE_REASON: u32 = 0x05B;
    pub const FIELD_PROGRESS: u32 = 0x090;
    pub const SHOWN_ALE: u32 = 0x194;
    pub const FRIENDLY_TROOPS: u32 = 0x198;
    pub const ENEMY_TROOPS: u32 = 0x19C;
    pub const SOW_SHORTFALL: u32 = 0x1A7;
    pub const TAX_SUPPRESSED: u32 = 0x1A8;
    pub const EVENT_ID: u32 = 0x1AA;
    pub const CASTLE_RUINED: u32 = 0x1C2;
    pub const CASTLE_DEGRADED: u32 = 0x1C3;
    pub const CASTLE_PERCENT: u32 = 0x1C4;
    pub const CASTLE_WORK_LEFT: u32 = 0x1CC;
    pub const CASTLE_STONE_OWED: u32 = 0x1D0;
    pub const CASTLE_WOOD_OWED: u32 = 0x1D4;
    pub const CASTLE_WORK_TOTAL: u32 = 0x1D8;
    pub const CASTLE_STONE_TOTAL: u32 = 0x1DC;
    pub const CASTLE_WOOD_TOTAL: u32 = 0x1E0;
    pub const SIEGE_MOAT_FILLED: u32 = 0x1E4;
    pub const SIEGE_WALL_DAMAGE: u32 = 0x1E6;
    pub const SIEGE_BREACH_SCORE: u32 = 0x1E8;
    pub const SIEGE_APPROACH_SCORE: u32 = 0x1EC;
    pub const SIEGE_RAMPARTS_BREACHED: u32 = 0x1F0;
    pub const SIEGE_GATE_OPEN: u32 = 0x1F1;
    pub const CASTLE_LEVEL_LEFT: u32 = 0x1F9;
    /// Signed: `Event_RollAll`'s blights write `0xD8` and `0xE2`, −40 and −30.
    pub const EVENT_POPULATION_PCT: u32 = 0x1FB;
    pub const EVENT_GRAIN_PCT: u32 = 0x1FC;
    pub const EVENT_HERD_PCT: u32 = 0x1FD;
    /// `County_EnsurePasture`'s round-robin field cursor…
    pub const PASTURE_CURSOR: u32 = 0x15A;
    /// …and `Weather_UpdateAll`'s, for the field it floods or parches.
    pub const BLIGHT_CURSOR: u32 = 0x15B;
    pub const FIELDS_GRAIN_SOWN: u32 = 0x202;
    /// The sown fields still standing — `County_DestroyField` steps it down
    /// and `Grain_SeasonTick`'s wheat picture divides by it.
    pub const FIELDS_GRAIN_STANDING: u32 = 0x206;
    pub const RECLAIM_FIELDS_FINISHING: u32 = 0x20C;
    pub const RECLAIM_SEASONS_TO_NEXT: u32 = 0x214;
    pub const ALE_HAPPINESS_GIVEN: u32 = 0x219;
    pub const GRAIN_CHANGE_EXPECTED: u32 = 0x22C;
    pub const GRAIN_SOWN_EXPECTED: u32 = 0x230;
    pub const CROP: u32 = 0x240;
    pub const HERD_CHANGE_EXPECTED: u32 = 0x258;
    pub const HERD_BIRTHS_EXPECTED: u32 = 0x268;
    pub const HERD_DEATHS_EXPECTED: u32 = 0x26C;
    pub const GRAIN_WEATHER_CHANGE: u32 = 0x24C;
    pub const HERD_WEATHER_CHANGE: u32 = 0x270;
    pub const HERD_EVENT_CHANGE: u32 = 0x274;
    pub const GRAIN_EVENT_CHANGE: u32 = 0x278;
    pub const WEAPON_TYPE: u32 = 0x290;
    pub const MERCENARY_OFFER: u32 = 0x1AD;
    /// Realm `+0x2D`, twenty-four bytes: `Army_PickName`'s per-name counters.
    pub const REALM_ARMY_NAMES: u32 = 0x02D;
    pub const LEVY_SURCHARGE: u32 = 0x2F4;
    pub const EVENT_POPULATION_SWING: u32 = 0x2F8;
    pub const GRAIN_GROWN_EXPECTED: u32 = 0x2FC;
}

/// The health meter every county starts a new game on.
///
/// **`[V]`, and it used to be `[I]`.** It was the one number in the whole
/// reproduction that came from prior art
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
/// A weather byte outside 0..=5. Refused:
    Weather { county: usize, byte: u8 },
    /// `g_localPlayer` outside 1..=5.
    LocalPlayer(i32),
    /// A season or year the clock cannot represent.
    Clock { season: i32, season_next: i32 },
    /// A neighbour id that is not a county on this map.
    Neighbour { county: usize, id: u8 },
    /// A unit whose type byte names none of the four handlers in
    /// `g_unitTickTable`. Slot 5 of that table is NULL and nothing spawns a
/// type-5 unit,
    /// does not model yet.
    UnitKind { unit: usize, byte: u8 },
    /// A unit standing on no tile, or whose `+0x0C` tile offset disagrees with
    /// its `x`/`y`. **The offset is redundant on purpose** — it is
    /// `(y * 64 + x) * 8` —
    /// every unit above this one is being read from the middle of its
    /// neighbour.
    UnitTile { unit: usize, x: u8, y: u8, offset: i32 },
    /// A unit owned by a realm that does not exist. Owner 6 is legal — it is
    /// what merchants and a county's own levied defence carry.
    UnitOwner { unit: usize, owner: u8 },
    /// `g_mercBandsInPlay` outside `0..=12`. See [`MERCENARY_BANDS_VA`].
    MercenaryBandCount(i32),
    /// A band whose constant field disagrees with the roster `Mercenary_Init`
    /// copied it from — which means the table is being read at the wrong
    /// place, not that the band is unusual. See [`MERCENARY_BANDS_VA`].
    MercenaryRoster { band: usize, field: &'static str, file: i32, roster: i32 },
    /// A band offered in a county off the map, or hired by a slot that is not
    /// an army carrying it.
    MercenaryState { band: usize, field: &'static str, value: i32 },
    /// County `+0x1AD` naming a band that is not in play. The raise-army
    /// screen indexes the roster with this byte.
    MercenaryOffer { county: usize, band: u8 },
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
            ImportError::MercenaryBandCount(n) => {
                write!(f, "g_mercBandsInPlay is {n}, and there are twelve bands")
            }
            ImportError::MercenaryRoster { band, field, file, roster } => write!(
                f,
                "mercenary band {band}'s {field} is {file} in the file and {roster} in the roster \
                 Mercenary_Init copies it from"
            ),
            ImportError::MercenaryState { band, field, value } => {
                write!(f, "mercenary band {band}'s {field} is {value}, which names nothing it can")
            }
            ImportError::MercenaryOffer { county, band } => {
                write!(f, "county {county} offers mercenary band {band}, which is not in play")
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
    /// `+0x28` — **the previous season's population**,
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
    /// `+0x10` — the health term of this season's happiness, which the ration
    /// panel draws beside the band name. See [`D_HAP_TAX_LOCAL`].
    pub d_hap_health: i32,
    /// `+0x0F` — `5 - taxRate`, the *local* half of the tax panel's
    /// *This county* line. **Not `+0x0E`**, which is that plus the realm's
    /// empire term and is what the happiness pass banks. See
    /// [`D_HAP_TAX_LOCAL`].
    pub d_hap_tax_local: i32,
    /// `+0xC0` — what the tax panel's *People pay* line says, which is a
    /// preview and not [`tax_collected`](Self::tax_collected): it is
    /// recomputed at the current population with no suppression test,
    /// suppressed county goes on saying what its people *would* pay while the
    /// treasury banks nothing. See [`D_HAP_TAX_LOCAL`].
    pub tax_shown: i32,
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
    /// `+0x295`, `+0x296`, `+0x297` and `+0x2A8` of each of the four industry
    /// records (`+1`, `+2`, `+3` and `+0x14` of a record based at `+0x294`), in
    /// commodity order **wood, iron, weapons, stone**: whether the county has
    /// the resource, how many seasons the industry is out of action, whether its
    /// switch is on, and next season's forecast. See [`INDUSTRY_BASE`].
    pub industry: [IndustryState; 4],
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
    /// They were not imported at all until the herd needed them,
    /// needs them badly: `l2_kingdom::land::herd_growth` staffs a herd at three
    /// labourers a head and kills the shortfall,
    /// labour of zero would lose cattle every season for want of a field this
/// crate had not read. `docs/kingdom.md` §13.
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
/// industry.
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
    /// `+0x1FE` — how this county farms. See [`FARM_STYLE`]: it is read by the
    /// neutral counties' own AI pass and was left at zero on every load until
    /// the map constructor made the omission visible.
    pub farm_style: u8,
    /// `+0x1F4` — an unowned county's own treasury. See [`PURSE`].
    pub purse: i32,
    /// `+0x1A4`, `+0x1A5`, `+0x1A0` — the county's merchant stall. See
    /// [`MERCHANT_COUNT`].
    pub merchant_count: i32,
    pub merchant_unit: u8,
    pub merchant_visits: i32,

    // --- C161 ------------------------------------------------
    // Every field below was stored by the original, modelled by
    // `l2_kingdom::county::County`, carried by our own save format — and read
    // out of a `.sav` by nothing,
    // value until a season rewrote it. `docs/stored-fields.json` is the
    // inventory that found them and the offsets are in [`stored` text mapping key: number to string map.]; each
    // kingdom field's doc comment says what it is.
    /// `+0x258`, `+0x268`, `+0x26C` — the cattle row's forecast: overall
    /// change, calf births, cow deaths.
    pub herd_change_expected: i32,
    pub herd_births_expected: i32,
    pub herd_deaths_expected: i32,
    /// `+0x24C`, `+0x278`, `+0x270`, `+0x274` — what last season's weather and
    /// random event did to the grain and the herd, which the grain and cattle
    /// panels print. See [`l2_kingdom::county::County::grain_weather_change`].
    pub grain_weather_change: i32,
    pub grain_event_change: i32,
    pub herd_weather_change: i32,
    pub herd_event_change: i32,
    /// `+0x22C`, `+0x230`, `+0x2FC` — the grain row's forecast: the change the
    /// sidebar draws, the sowing, the growth.
    pub grain_change_expected: i32,
    pub grain_sown_expected: i32,
    pub grain_grown_expected: i32,
    /// `+0x20C`, `+0x214` — the reclamation row's two figures.
    pub reclaim_fields_finishing: i32,
    pub reclaim_seasons_to_next: i32,
    /// `+0x18`, `+0x1C` — *"Average happiness"* and the running sum it is
/// taken from. **These were not dropped, they were invented:** the
    /// importer set both to this season's happiness, which is wrong in every
    /// save past turn one — `siege-aftersie.sav` county 2 draws 95 where the
    /// original draws 54.
    pub happiness_avg: i32,
    pub happiness_sum: i32,
    pub d_hap_tax: i32,
    pub shown_army: i32,
    pub tax_hap_other: i32,
    pub shown_ale: i32,
    pub ale_happiness_given: i32,
    /// `+0x21` — `Unrest_UpdateAll`'s once-only warning latch, which
    /// `County::unrest_warned` said had no known offset. Set only in counties
    /// below 30 happiness, in every save.
    pub unrest_warned: bool,
    pub pop_change_pct: i32,
    /// `+0x38` — the *Army* line of the population panel.
    pub army: i32,
    pub largest_inflow: i32,
    pub inflow_sources: [u8; l2_kingdom::county::MAX_INFLOW_SOURCES],
    pub emigrant_destination: u8,
    pub largest_inflow_source: u8,
    pub change_reason: u8,
    pub event_fired: bool,
    pub event_id: u16,
    pub event_population_pct: i32,
    /// `+0x2F8` — the figure *Plague* and *Wedding fever*'s letters print,
    /// written by `Population_UpdateAll` every season. Excluded until
    /// C169 because our population rule could not have produced
    /// it; `l2_kingdom::county::County::event_population_swing`.
    pub event_population_swing: i32,
    pub event_grain_pct: i32,
    pub event_herd_pct: i32,
    pub tax_suppressed: bool,
    pub field_progress: [u16; MAX_FIELDS],
    /// `+0x198`, `+0x19C` — the troops standing in the county, which the ration
    /// panel adds to the food bill.
    pub friendly_troops: i32,
    pub enemy_troops: i32,
    pub levy_surcharge: i32,
    pub castle_degraded: u8,
    pub castle_ruined: bool,
    pub castle_level_left: u8,
    pub castle_percent: u8,
    pub castle_work_left: i32,
    pub castle_work_total: i32,
    pub castle_stone_owed: i32,
    pub castle_stone_total: i32,
    pub castle_wood_owed: i32,
    pub castle_wood_total: i32,
    pub siege_scars: l2_kingdom::siege::SiegeScars,
    pub crop: [i32; 3],
    /// `+0x15A` and `+0x15B` — the two round-robin field cursors.
    pub pasture_cursor: u8,
    pub blight_cursor: u8,
    pub fields_grain_sown: i32,
    pub fields_grain_standing: i32,
    pub sow_shortfall: bool,
    /// `+0x290` — which weapon the blacksmith makes, and the frame the weapons
    /// row draws.
    pub weapon_type: usize,
    /// `+0x1AD` — the mercenary band standing in this county, 0 for none:
    /// `Mercenary_AdvanceAll`'s cache of `Mercenary_OfferInCounty`, which the
    /// raise-army screen offers and the town square's marker draws. Carried
    /// with [`Scenario::mercenaries`] and never without it — alone it would
    /// advertise a band the kingdom could not hire. See [`MERCENARY_BANDS_VA`].
    pub mercenary_offer: u8,
}

/// One realm's imported state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RealmState {
    pub in_play: bool,
    pub strength: u8,
    pub is_human: bool,
    pub lord: u8,
    /// Realm `+0x0A` — the shield and flag colour. A default game sets it to
    /// the realm id; a custom game's colour picker permutes it.
    pub shield_index: u8,
    pub county_count: u8,
    /// Realm `+0x2A` — the most counties ever held,
    /// [`l2_kingdom::realm::Realm::peak_counties`].
    pub peak_counties: u8,
    pub rank: u8,
    pub score: i32,
    pub gold: i32,
    pub wages: i32,
    pub iron: i32,
    pub stone: i32,
    pub wood: i32,
    pub weapons: [i32; l2_kingdom::tables::WEAPON_TYPE_COUNT],
    /// `+0x84 + other * 0x10` — this realm's view of each other realm.
    ///
    /// **Carried at last.** `l2_formats::save::Realm` did not read these 96
    /// bytes at all, so every load ran `Diplo_Init` and a mid-game save came
    /// back with the diplomatic matrix reset — every alliance and every grudge
    /// gone. `docs/decisions.md` C83.
    pub pairs: [l2_kingdom::realm::Pair; l2_kingdom::realm::MAX_REALMS],

    // --- C161 ------------------------------------------------
    // Modelled by `l2_kingdom::realm::Realm`, stored by the original, read by
    // nobody. The ally byte is the one a player sees soonest: the diplomacy
    // screen draws `Realm::ally`, and an allied realm loaded from a save showed
    // no alliance. The rest are the score screen's totals and the AI's
    // standing orders. `docs/stored-fields.json`.
    pub ai_step: i32,
    pub tax_hap_empire: i8,
    pub population_total: i32,
    pub population_mean: i32,
    pub population_last: i32,
    pub mean_happiness: i32,
    pub mean_health: i32,
    pub share_of_map_pct: i32,
    pub army_count: u8,
    pub total_men: i32,
    /// `+0x4C` — the castle count, [`l2_kingdom::tables::SCORE_INPUT_CASTLES`].
    pub castle_count: i32,
    pub offer_pending: bool,
    pub ally_candidate: u8,
    pub ally: u8,
    pub target_county: u8,
    pub taunt_timer: u8,
    pub taunt_stage: u8,
    pub war_target: u8,
    pub offer_timer: i8,
    pub crowned_once: bool,
    pub weapon_rota: i32,
    pub voice_rotation: u8,
    pub muster_county: u8,
    pub raid_county: u8,
    pub muster_timer: u8,
    pub threat_realm: u8,
    pub attack_county: u8,
    pub raid_timer: u8,
    pub want: [i32; 4],
    pub bankrupt_stage: u8,
    pub trade_spent_a: i32,
    pub trade_spent_b: i32,
    pub trade_received_a: i32,
    pub trade_received_b: i32,
    /// Realm `+0x2D` — `Army_PickName`'s twenty-four per-name counters, which
    /// decide the name the next army this realm raises is given: the least-used
    /// of the lord's twenty-four, `+2` a pick and `-1` a destroyed army.
    ///
    /// **Modelled, encoded, **, so a loaded game handed out
    /// names from a clean slate — a second *"The Black Company"* beside the
    /// first. `docs/stored-fields.json` had it excluded as *"not a field of
    /// Realm"*, which was true: it is [`l2_kingdom::unit::ArmyNames`], a field
    /// of the campaign.
    pub army_names: [u8; l2_kingdom::unit::ARMY_NAME_SLOTS],
    /// Realm `+0xF4` and `+0xF8` — `Tax_CollectAll`'s accumulator pair,
    /// [`l2_kingdom::realm::Realm::tax_ledger`].
    pub tax_ledger: [i32; 2],
}

/// A whole starting position, as plain data.
///
/// Nothing here is a `Save` and nothing here is a `Kingdom`. That is what makes
/// this the seam: a hand-written scenario, a
/// scenario editor or a future `.toml` can produce one of these
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
    /// `g_mercBands` and `g_mercBandsInPlay` — the twelve mercenary bands and
    /// where each is in its walk. From a save, the file's table; from a map,
    /// `Mercenary_Init`'s, which is what `Game_NewGame` runs after
    /// `Merchant_SpawnAll`. See [`MERCENARY_BANDS_VA`].
    pub mercenaries: MercenaryBands,
    /// **What the local player has seen** — `g_tiles[t].bank & 0x20`, the fog
    /// of war's seen bit, read out of the same block as [`Scenario::map`].
    ///
    /// Every writer of the bit is guarded on `g_localPlayer`,
    /// answers the question for that realm alone and every other realm's bits
    /// arrive clear. Nothing reads another realm's bits; `l2_kingdom::explore`
    /// has the whole table. A new game fills it from the seating instead.
    pub explored: l2_kingdom::explore::Explored,
    /// `g_units` — **armies, revolting peasants, merchants and transports**, as
    /// `(slot, unit)` pairs in ascending slot order.
    ///
    /// **The whole block used to be dropped on the floor.** Every other layer
    /// was ready for it — `l2_kingdom::unit` models the record, `movement` walks
    /// it, `conquest` fights with it —
    /// were the ones tests built by hand.
    /// working economy and an empty map.
    ///
/// Slots are carried because they are *referenced*:
    /// county `garrison_unit`, a garrison's `besieged_by` and a mercenary band's
    /// `hired_by` all name a slot, and `Merchant_AdvanceAll` indexes the route
    /// table by slot. Renumber on import and the six merchants walk each other's
    /// routes.
    ///
/// Stored as [`l2_kingdom::unit::Unit`],
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
/// Carried on the scenario because nothing
    /// in the simulation reads it: `Merchant_PickStartCounties` and
    /// `Merchant_SpawnAll` both ran before the save was written, and their
    /// output is already in the unit array. What it is good for is *checking*
    /// that reading — every merchant's `+0x167` is its own entry here, on every
    /// save the fixture set holds — which is a test's business and not a rule's.
    pub merchant_start: [u8; ROUTES],
}

