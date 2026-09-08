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
use l2_kingdom::county::{County, MAX_COUNTY_ID};
use l2_kingdom::realm::MAX_REALMS;
use l2_kingdom::tables::{health_band, Tables, Weather, JOB_COUNT};
use l2_kingdom::{land, Kingdom, Options};

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
/// records, twelve bytes each, count first. The shipped save then checks
/// itself: county 1 holds 218 cattle farmers and 217 wood cutters against a
/// population of 435, county 2 holds 323 and 133 against 456, and **every one
/// of the fourteen sums to its population exactly**, which no wrong stride
/// would do fourteen times running. `docs/kingdom.md` §13.
const LABOUR_BASE: u32 = 0xC4;
const LABOUR_STRIDE: u32 = 0x0C;

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
/// shipped scenario used is not in doubt. `tools/oracle/kingdom.ps1` checks the
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
}

impl Scenario {
    /// Read a shipped save into plain data.
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
                fields_fallow: c.fields_fallow as i32,
                fields_cattle: c.fields_cattle as i32,
                fields_grain: c.fields_grain as i32,
                fertility: c.fertility,
                weather,
                dryness: c.dryness as i32,
                grain: c.grain,
                herd: c.herd,
                labour: read_labour(&save, c.index, 0)?,
                labour_wanted: read_labour(&save, c.index, 4)?,
                labour_useful: read_labour(&save, c.index, 8)?,
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
            },
            clock: Clock {
                season: g.season as u8,
                season_next: g.season_next as u8,
                year: g.year,
                turn_count: g.turn_count.max(0) as u32,
            },
            counties,
            realms,
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
            c.fields_fallow = s.fields_fallow;
            c.fields_cattle = s.fields_cattle;
            c.fields_grain = s.fields_grain;
            c.fertility = s.fertility;
            c.weather = s.weather;
            c.dryness = s.dryness;
            c.grain = s.grain;
            c.herd = s.herd;
            c.labour = s.labour;
            c.labour_wanted = s.labour_wanted;
            c.labour_useful = s.labour_useful;
            // `FUN_0044D913` is called from everywhere a county's herd or
            // pasture can change, county setup included, so a county always
            // arrives with its crowding already computed. Deriving it here
            // rather than reading `+0x25C` keeps the two consistent — and the
            // reproduction test checks the derived value against the byte.
            c.herd_crowding = land::herd_crowding(&tables, c.herd, c.fields_cattle);
        }
        k
    }
}
