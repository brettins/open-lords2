

mod loader;

pub mod newgame;

use l2_formats::save::SaveError;
use l2_kingdom::county::{MAX_COUNTY_ID, MAX_FIELDS};
use l2_kingdom::merchant::{MerchantRoutes, ROUTES};
use l2_kingdom::mercenary::MercenaryBands;
use l2_kingdom::tables::{Weather, JOB_COUNT};
use l2_kingdom::unit::Unit;
use l2_kingdom::{CampaignMap, Options};

const TILE_BANK: usize = 2;
const BANK_SEEN: u8 = 0x20;

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
/// fourteen and county 5 having neither. `crates/l2-scenario/tests/import/main.rs`
/// asserts it.
///
/// Until this was read, every county arrived with [`Industry::has_resource`] and
/// [`Industry::enabled`] defaulted to `true`, so **no county ever
/// showed a mine**: the village picks the mine for cluster 0 only when the
/// county has a mine *and no quarry*, and a county that claims both is a county
/// with a quarry. `docs/decisions.md` C57.
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
    pub efficiency: i32,
    pub capacity: i32,
    pub total: i32,
    pub total_snapshot: i32,
}


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
/// table; `crates/l2-kingdom/tests/reproduction/main.rs` checks the save against it.
pub const STARTING_HEALTH_METER: i32 = 65;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportError {
    Save(SaveError),
    CountyCount(i32),
    Owner { county: usize, owner: u8 },
    Weather { county: usize, byte: u8 },
    LocalPlayer(i32),
    Clock { season: i32, season_next: i32 },
    Neighbour { county: usize, id: u8 },
    UnitKind { unit: usize, byte: u8 },
    /// A unit standing on no tile, or whose `+0x0C` tile offset disagrees with
    /// its `x`/`y`. **The offset is redundant on purpose** — it is
    /// `(y * 64 + x) * 8` —
    /// every unit above this one is being read from the middle of its
    /// neighbour.
    UnitTile { unit: usize, x: u8, y: u8, offset: i32 },
    UnitOwner { unit: usize, owner: u8 },
    MercenaryBandCount(i32),
    MercenaryRoster { band: usize, field: &'static str, file: i32, roster: i32 },
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Clock {
    pub season: u8,
    pub season_next: u8,
    pub year: i32,
    pub turn_count: u32,
}

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
    pub castle_switch: bool,
    /// `+0x295`, `+0x296`, `+0x297` and `+0x2A8` of each of the four industry
    /// records (`+1`, `+2`, `+3` and `+0x14` of a record based at `+0x294`), in
    /// commodity order **wood, iron, weapons, stone**: whether the county has
    /// the resource, how many seasons the industry is out of action, whether its
    /// switch is on, and next season's forecast. See [`IndustryState`].
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
    pub labour: [i32; JOB_COUNT],
    /// `+0xC4 + job * 0x0C + 0x04` and `+ 0x08` — the wanted floor and the
    /// useful ceiling of each record.
    pub labour_wanted: [i32; JOB_COUNT],
    pub labour_useful: [i32; JOB_COUNT],
    /// `+0x130 + job*4` — the eight percentages the allocator splits each half
    /// of the county by, and `+0x08`, the percentage of the county that is
/// industry.
    pub labour_share: [i32; JOB_COUNT - 1],
    pub industry_share: i32,
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
    //
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
    pub mercenary_offer: u8,
}

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
    pub army_names: [u8; l2_kingdom::unit::ARMY_NAME_SLOTS],
    /// Realm `+0xF4` and `+0xF8` — `Tax_CollectAll`'s accumulator pair,
    /// [`l2_kingdom::realm::Realm::tax_ledger`].
    pub tax_ledger: [i32; 2],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scenario {
    pub county_count: usize,
    pub local_player: u8,
    pub weather_county: usize,
    pub options: Options,
    pub clock: Clock,
    pub counties: Vec<Option<CountyState>>,
    pub realms: Vec<RealmState>,
    pub map: CampaignMap,
    pub mercenaries: MercenaryBands,
    pub explored: l2_kingdom::explore::Explored,
    pub units: Vec<(usize, Unit)>,
    /// `g_merchantRoutes` (`0x00567970`) — the six trade itineraries, which are
    /// what makes a merchant walk anywhere at all.
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

