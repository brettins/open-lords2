//! **Our own save format** — a whole campaign, as bytes, and back again.
//!
//! The original's `.sav` is a memory dump with no header and no version, whose
//! schema lives in `Lords2.exe` (`l2_formats::save` reads it, and
//! `l2-scenario` imports it). This is the other one: the format *we* write, so
//! that a player can quit and resume.
//!
//! # It reuses `l2-net`'s encoder, and that is the whole design
//!
//! `docs/netcode.md` §5 and §6 already demanded a byte-exact encoding of
//! simulation state — for the per-tick checksum, for the late-join snapshot and
//! for the desync dump — and `l2_net::Canonical` is that encoder. Writing a
//! second one here would mean a save whose bytes and a checksum whose bytes
//! could disagree, which is the exact failure `canonical.rs` was written to
//! prevent, and it would mean two places to get little-endian, fixed widths and
//! length prefixes right. So [`encode`] is a `Canonical`, hashing as it writes,
//! and the sections it opens are the same subsystem names a desync dump would
//! use.
//!
//! What that buys, concretely:
//!
//! * **No floats.** There are none in this crate to begin with, and `Canonical`
//!   has no method that could write one.
//! * **No `usize` on the wire.** A length written as `usize` is four bytes on a
//!   32-bit player and eight on a 64-bit one. Every count here goes out as
//!   `u32`.
//! * **No iteration in hash order.** Counties and realms are fixed arrays
//!   walked by ascending index; there is no `HashMap` in this crate to iterate.
//! * **Determinism is a property of the encoder**, not of a promise made here.
//!
//! # Versioned, and it refuses rather than guesses
//!
//! The header is a magic, a format version and a **ruleset fingerprint**.
//!
//! An unknown version is [`LoadError::UnsupportedVersion`] and nothing else. It
//! is never read on the assumption that the fields happen to line up: a save
//! written by a newer build is a save this build cannot honestly interpret, and
//! a plausible kingdom assembled from a misread one is worse than an error
//! message.
//!
//! # The ruleset is fingerprinted, not stored
//!
//! `Tables` is where every economic constant lives, and a mod replaces it
//! (`docs/modding.md`). A save could carry its own copy, but then loading one
//! would silently override whatever mod set the player has enabled, and a
//! ruleset would have two sources of truth. So the save stores a 64-bit hash of
//! the `Tables` it was written under, [`decode`] takes the ruleset from the
//! caller, and a mismatch is [`LoadError::RulesetMismatch`]. The rules come
//! from the mod layer; the save only checks that they are the same rules.
//!
//! # What is not here
//!
//! No `std::fs`. This module turns a [`Kingdom`] into a `Vec<u8>` and back;
//! *where* those bytes live is the application's business, and keeping the file
//! system out is what lets the round-trip tests run with no directory, no
//! permissions and no clean-up.

use crate::county::{ChangeReason, County, Industry, MAX_COUNTIES, MAX_INFLOW_SOURCES, MAX_NEIGHBOURS};
use crate::kingdom::{History, HistoryEntry, Kingdom, Options};
use crate::phase::{Phase, TurnMachine};
use crate::realm::{Realm, MAX_REALMS};
use crate::tables::{
    Tables, Weather, HISTORY_COUNTIES, HISTORY_SEASONS, JOB_COUNT, WEAPON_TYPE_COUNT,
};
use l2_net::canonical::{Canonical, CodecError, Decode, Encode, Reader};

/// Eight bytes, so a file command can name the format and a truncated file
/// fails on the magic rather than three fields later.
pub const MAGIC: [u8; 8] = *b"L2KSAVE\x01";

/// The format version. **Bump it whenever the byte layout below changes**, and
/// never reinterpret an unknown one.
pub const VERSION: u32 = 1;

/// The header: magic, version, ruleset fingerprint, and the body length.
pub const HEADER_LEN: usize = 8 + 4 + 8 + 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadError {
    /// The first eight bytes are not [`MAGIC`].
    NotASave,
    /// A version this build does not know. **A refusal, not a guess.**
    UnsupportedVersion { found: u32, supported: u32 },
    /// The save was written under a different ruleset. The rules are the mod
    /// layer's to supply; this only reports that they differ.
    RulesetMismatch { save: u64, supplied: u64 },
    /// The body's declared length does not match what followed it.
    TruncatedBody { declared: usize, actual: usize },
    /// The body decoded but its checksum does not match the header's.
    Corrupt { expected: u64, actual: u64 },
    /// The bytes ran out, or a tag byte named nothing.
    Malformed(CodecError),
    /// `head`, `tail` and `len` do not describe a ring of
    /// [`HISTORY_SEASONS`] seasons.
    CorruptHistory { head: usize, tail: usize, len: usize },
    /// A county count larger than the array.
    CountyCount(u32),
}

impl core::fmt::Display for LoadError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            LoadError::NotASave => write!(f, "not a lords2 kingdom save"),
            LoadError::UnsupportedVersion { found, supported } => write!(
                f,
                "save format version {found}; this build reads {supported}. \
                 Refusing rather than guessing at the layout"
            ),
            LoadError::RulesetMismatch { save, supplied } => write!(
                f,
                "the save was written under ruleset {save:#018x} and was handed \
                 {supplied:#018x}; load the mod set it was made with"
            ),
            LoadError::TruncatedBody { declared, actual } => {
                write!(f, "the body declares {declared} bytes and {actual} follow")
            }
            LoadError::Corrupt { expected, actual } => {
                write!(f, "checksum {actual:#018x}, expected {expected:#018x}")
            }
            LoadError::Malformed(e) => write!(f, "{e}"),
            LoadError::CorruptHistory { head, tail, len } => {
                write!(f, "history ring head {head}, tail {tail}, len {len}")
            }
            LoadError::CountyCount(n) => write!(f, "{n} counties, and the array holds 16"),
        }
    }
}

impl std::error::Error for LoadError {}

impl From<CodecError> for LoadError {
    fn from(e: CodecError) -> LoadError {
        LoadError::Malformed(e)
    }
}

/// The fingerprint of a ruleset: the canonical hash of every constant in it.
///
/// Two `Tables` that differ anywhere differ here — `tests/save.rs` asserts that
/// for each of the sub-tables in turn, which is the guard against a new field
/// being added and quietly left out of the encoding.
pub fn ruleset_fingerprint(tables: &Tables) -> u64 {
    Canonical::hash_of(tables)
}

/// A whole kingdom as bytes.
///
/// Deterministic by construction: the same kingdom encodes to the same bytes on
/// any machine, because every write is a fixed width in little-endian and every
/// collection is an array walked by index.
pub fn encode(kingdom: &Kingdom) -> Vec<u8> {
    let mut body = Canonical::recording();
    kingdom.encode(&mut body);
    let digest = body.finish();
    let bytes = digest.bytes.expect("a recording encoder keeps its bytes");

    let mut out = Vec::with_capacity(HEADER_LEN + bytes.len());
    out.extend_from_slice(&MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.extend_from_slice(&ruleset_fingerprint(&kingdom.tables).to_le_bytes());
    out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(&bytes);
    out.extend_from_slice(&digest.hash.to_le_bytes());
    out
}

/// The checksum of a kingdom without writing it out — the same number
/// [`encode`] puts in the trailer, and the same one a lockstep tick would
/// exchange.
pub fn checksum(kingdom: &Kingdom) -> u64 {
    Canonical::hash_of(kingdom)
}

/// Read a save back, on a supplied ruleset.
///
/// `tables` is the ruleset the game is currently running — from `l2-mods`, or
/// [`Tables::DEFAULT`]. It must be the one the save was written under.
pub fn decode(bytes: &[u8], tables: Tables) -> Result<Kingdom, LoadError> {
    if bytes.len() < HEADER_LEN + 8 || bytes[..8] != MAGIC {
        return Err(LoadError::NotASave);
    }
    let version = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
    if version != VERSION {
        return Err(LoadError::UnsupportedVersion { found: version, supported: VERSION });
    }
    let mut fingerprint = [0u8; 8];
    fingerprint.copy_from_slice(&bytes[12..20]);
    let fingerprint = u64::from_le_bytes(fingerprint);
    let supplied = ruleset_fingerprint(&tables);
    if fingerprint != supplied {
        return Err(LoadError::RulesetMismatch { save: fingerprint, supplied });
    }
    let declared =
        u32::from_le_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]) as usize;
    let actual = bytes.len() - HEADER_LEN - 8;
    if declared != actual {
        return Err(LoadError::TruncatedBody { declared, actual });
    }
    let body = &bytes[HEADER_LEN..HEADER_LEN + declared];
    let mut trailer = [0u8; 8];
    trailer.copy_from_slice(&bytes[HEADER_LEN + declared..]);
    let expected = u64::from_le_bytes(trailer);

    let mut c = Canonical::hashing();
    c.raw(body);
    let actual_hash = c.finish().hash;
    if actual_hash != expected {
        return Err(LoadError::Corrupt { expected, actual: actual_hash });
    }

    let mut reader = Reader::new(body);
    let mut kingdom = decode_kingdom(&mut reader, tables)?;
    reader.finish()?;
    // The tables are the caller's, not the file's; the fingerprint above is
    // what guarantees they are the right ones.
    kingdom.tables = tables;
    Ok(kingdom)
}

// ---------------------------------------------------------------------------
// The state
// ---------------------------------------------------------------------------

impl Encode for Kingdom {
    fn encode(&self, out: &mut Canonical) {
        out.section("clock");
        out.u32(self.county_count as u32);
        out.u8(self.season);
        out.u8(self.season_next);
        out.u8(self.season_prev);
        out.i32(self.year);
        out.i32(self.year_next);
        out.u32(self.turn_count);
        out.u8(self.turn.phase.index());
        out.u32(self.turn.step);
        out.u32(self.weather_county as u32);

        out.section("options");
        out.u8(self.options.difficulty);
        out.bool(self.options.advanced_farming);
        out.bool(self.options.armies_eat);

        // The generator is part of the state (docs/netcode.md D-3), so it is
        // part of the save and part of the checksum.
        out.section("rng");
        out.encode(&self.rng);

        // Fixed arrays, walked by ascending index. The lengths are part of the
        // schema, so they are written once rather than per record.
        out.section("counties");
        out.u32(MAX_COUNTIES as u32);
        for county in &self.counties {
            county.encode(out);
        }

        out.section("realms");
        out.u32(MAX_REALMS as u32);
        for realm in &self.realms {
            realm.encode(out);
        }

        out.section("history");
        self.history.encode(out);
    }
}

fn decode_kingdom(input: &mut Reader<'_>, tables: Tables) -> Result<Kingdom, LoadError> {
    let county_count = input.u32()?;
    if county_count as usize >= MAX_COUNTIES {
        return Err(LoadError::CountyCount(county_count));
    }
    let mut k = Kingdom::with_tables(0, tables);
    k.county_count = county_count as usize;
    k.season = input.u8()?;
    k.season_next = input.u8()?;
    k.season_prev = input.u8()?;
    k.year = input.i32()?;
    k.year_next = input.i32()?;
    k.turn_count = input.u32()?;
    let phase = input.u8()?;
    k.turn = TurnMachine {
        phase: Phase::from_index(phase).ok_or(CodecError::BadTag {
            tag: phase,
            expected: "turn phase",
            at: input.position() - 1,
        })?,
        step: input.u32()?,
    };
    k.weather_county = input.u32()? as usize;

    k.options = Options {
        difficulty: input.u8()?,
        advanced_farming: input.bool()?,
        armies_eat: input.bool()?,
    };
    k.rng = input.decode()?;

    let counties = input.u32()? as usize;
    if counties != MAX_COUNTIES {
        return Err(LoadError::CountyCount(counties as u32));
    }
    for slot in 0..MAX_COUNTIES {
        k.counties[slot] = County::decode(input)?;
    }

    let realms = input.u32()? as usize;
    if realms != MAX_REALMS {
        return Err(LoadError::Malformed(CodecError::BadTag {
            tag: realms.min(255) as u8,
            expected: "realm count",
            at: input.position(),
        }));
    }
    for slot in 0..MAX_REALMS {
        k.realms[slot] = Realm::decode(input)?;
    }

    k.history = decode_history(input)?;
    Ok(k)
}

impl Encode for County {
    fn encode(&self, out: &mut Canonical) {
        out.bool(self.event_fired);
        out.u16(self.event_id);
        out.u8(self.owner);
        out.u8(self.health_band);
        out.i32(self.health_meter);
        out.i32(self.happiness);
        out.i32(self.happiness_last);
        out.i32(self.d_hap_tax);
        out.i32(self.d_hap_tax_local);
        out.i32(self.d_hap_health);
        out.i32(self.d_hap_ration);
        out.i32(self.shown_tax);
        out.i32(self.shown_ration);
        out.i32(self.shown_health);
        out.i32(self.shown_army);
        out.i32(self.tax_hap_other);
        out.i32(self.shown_events);
        out.i32(self.happiness_avg);
        out.i32(self.happiness_sum);
        out.i32(self.shown_ale);
        out.i32(self.ale_happiness_given);
        out.u8(self.unrest);
        out.bool(self.unrest_warned);

        out.i32(self.population);
        out.i32(self.pop_last);
        out.i32(self.pop_change_pct);
        out.i32(self.births);
        out.i32(self.deaths);
        out.i32(self.army);
        out.i32(self.emigrants);
        out.i32(self.immigrants);
        out.i32(self.largest_inflow);
        out.u8(self.emigrant_destination);
        out.u8(self.largest_inflow_source);
        out.raw(&self.inflow_sources);
        out.u8(self.neighbour_count);
        out.raw(&self.neighbours);
        out.u8(self.change_reason as u8);
        out.i32(self.pop_band);
        out.u8(self.anchor_x);
        out.u8(self.anchor_y);

        out.i32(self.tax_rate);
        out.i32(self.tax_collected);
        out.i32(self.tax_shown);
        for job in &self.labour {
            out.i32(*job);
        }
        for field in &self.field_progress {
            out.u16(*field);
        }
        out.i32(self.ration_achieved);
        out.i32(self.ration_wanted);
        out.i32(self.ration_split);
        out.i32(self.grain_eaten);
        out.i32(self.herd_eaten);
        out.i32(self.grain_available);
        out.i32(self.herd_available);
        out.i32(self.friendly_troops);
        out.i32(self.enemy_troops);
        out.u8(self.castle_type);
        out.u8(self.castle_building);
        out.bool(self.castle_degraded);
        out.i32(self.castle_progress);
        out.i32(self.event_population_pct);
        out.i32(self.event_grain_pct);
        out.i32(self.event_herd_pct);
        out.i32(self.fields_fallow);
        out.i32(self.fields_cattle);
        out.i32(self.fields_grain);
        out.i32(self.fertility);
        out.u8(self.weather.index());
        out.i32(self.dryness);
        out.i32(self.grain);
        for stage in &self.crop {
            out.i32(*stage);
        }
        out.i32(self.herd);
        for industry in &self.industry {
            industry.encode(out);
        }
        out.u32(self.weapon_type as u32);
        out.bool(self.tax_suppressed);
    }
}

impl Decode for County {
    fn decode(input: &mut Reader<'_>) -> Result<County, CodecError> {
        let mut c = County::new();
        c.event_fired = input.bool()?;
        c.event_id = input.u16()?;
        c.owner = input.u8()?;
        c.health_band = input.u8()?;
        c.health_meter = input.i32()?;
        c.happiness = input.i32()?;
        c.happiness_last = input.i32()?;
        c.d_hap_tax = input.i32()?;
        c.d_hap_tax_local = input.i32()?;
        c.d_hap_health = input.i32()?;
        c.d_hap_ration = input.i32()?;
        c.shown_tax = input.i32()?;
        c.shown_ration = input.i32()?;
        c.shown_health = input.i32()?;
        c.shown_army = input.i32()?;
        c.tax_hap_other = input.i32()?;
        c.shown_events = input.i32()?;
        c.happiness_avg = input.i32()?;
        c.happiness_sum = input.i32()?;
        c.shown_ale = input.i32()?;
        c.ale_happiness_given = input.i32()?;
        c.unrest = input.u8()?;
        c.unrest_warned = input.bool()?;

        c.population = input.i32()?;
        c.pop_last = input.i32()?;
        c.pop_change_pct = input.i32()?;
        c.births = input.i32()?;
        c.deaths = input.i32()?;
        c.army = input.i32()?;
        c.emigrants = input.i32()?;
        c.immigrants = input.i32()?;
        c.largest_inflow = input.i32()?;
        c.emigrant_destination = input.u8()?;
        c.largest_inflow_source = input.u8()?;
        c.inflow_sources.copy_from_slice(input.raw(MAX_INFLOW_SOURCES)?);
        c.neighbour_count = input.u8()?;
        c.neighbours.copy_from_slice(input.raw(MAX_NEIGHBOURS)?);
        let at = input.position();
        c.change_reason = match input.u8()? {
            0 => ChangeReason::None,
            1 => ChangeReason::Births,
            2 => ChangeReason::Deaths,
            3 => ChangeReason::Emigration,
            4 => ChangeReason::Immigration,
            tag => return Err(CodecError::BadTag { tag, expected: "change reason", at }),
        };
        c.pop_band = input.i32()?;
        c.anchor_x = input.u8()?;
        c.anchor_y = input.u8()?;

        c.tax_rate = input.i32()?;
        c.tax_collected = input.i32()?;
        c.tax_shown = input.i32()?;
        for job in 0..JOB_COUNT {
            c.labour[job] = input.i32()?;
        }
        for field in 0..c.field_progress.len() {
            c.field_progress[field] = input.u16()?;
        }
        c.ration_achieved = input.i32()?;
        c.ration_wanted = input.i32()?;
        c.ration_split = input.i32()?;
        c.grain_eaten = input.i32()?;
        c.herd_eaten = input.i32()?;
        c.grain_available = input.i32()?;
        c.herd_available = input.i32()?;
        c.friendly_troops = input.i32()?;
        c.enemy_troops = input.i32()?;
        c.castle_type = input.u8()?;
        c.castle_building = input.u8()?;
        c.castle_degraded = input.bool()?;
        c.castle_progress = input.i32()?;
        c.event_population_pct = input.i32()?;
        c.event_grain_pct = input.i32()?;
        c.event_herd_pct = input.i32()?;
        c.fields_fallow = input.i32()?;
        c.fields_cattle = input.i32()?;
        c.fields_grain = input.i32()?;
        c.fertility = input.i32()?;
        let at = input.position();
        let byte = input.u8()?;
        c.weather = Weather::from_index(byte)
            .ok_or(CodecError::BadTag { tag: byte, expected: "weather", at })?;
        c.dryness = input.i32()?;
        c.grain = input.i32()?;
        for stage in 0..c.crop.len() {
            c.crop[stage] = input.i32()?;
        }
        c.herd = input.i32()?;
        for slot in 0..c.industry.len() {
            c.industry[slot] = Industry::decode(input)?;
        }
        c.weapon_type = input.u32()? as usize;
        c.tax_suppressed = input.bool()?;
        Ok(c)
    }
}

impl Encode for Industry {
    fn encode(&self, out: &mut Canonical) {
        out.i32(self.output);
        out.i32(self.efficiency);
        out.i32(self.capacity);
        out.bool(self.has_resource);
        out.bool(self.enabled);
        out.i32(self.disabled_seasons);
        out.i32(self.total);
    }
}

impl Decode for Industry {
    fn decode(input: &mut Reader<'_>) -> Result<Industry, CodecError> {
        Ok(Industry {
            output: input.i32()?,
            efficiency: input.i32()?,
            capacity: input.i32()?,
            has_resource: input.bool()?,
            enabled: input.bool()?,
            disabled_seasons: input.i32()?,
            total: input.i32()?,
        })
    }
}

impl Encode for Realm {
    fn encode(&self, out: &mut Canonical) {
        out.i32(self.ai_step);
        out.bool(self.in_play);
        out.u8(self.strength);
        out.bool(self.is_human);
        out.u8(self.lord);
        out.i8(self.tax_hap_empire);
        out.u8(self.county_count);
        out.u8(self.rank);
        out.i32(self.score);
        out.i32(self.wages);
        out.i32(self.gold);
        out.i32(self.iron);
        out.i32(self.stone);
        out.i32(self.wood);
        for weapon in &self.weapons {
            out.i32(*weapon);
        }
        out.u8(self.bankrupt_stage);
        out.i32(self.population_total);
        out.i32(self.population_last);
        out.i32(self.population_mean);
        out.i32(self.mean_happiness);
        out.i32(self.mean_health);
        out.i32(self.share_of_map_pct);
        out.u8(self.army_count);
        out.i32(self.total_men);
        for input in &self.score_inputs {
            out.i32(*input);
        }
    }
}

impl Decode for Realm {
    fn decode(input: &mut Reader<'_>) -> Result<Realm, CodecError> {
        let mut r = Realm::new();
        r.ai_step = input.i32()?;
        r.in_play = input.bool()?;
        r.strength = input.u8()?;
        r.is_human = input.bool()?;
        r.lord = input.u8()?;
        r.tax_hap_empire = input.i8()?;
        r.county_count = input.u8()?;
        r.rank = input.u8()?;
        r.score = input.i32()?;
        r.wages = input.i32()?;
        r.gold = input.i32()?;
        r.iron = input.i32()?;
        r.stone = input.i32()?;
        r.wood = input.i32()?;
        for slot in 0..WEAPON_TYPE_COUNT {
            r.weapons[slot] = input.i32()?;
        }
        r.bankrupt_stage = input.u8()?;
        r.population_total = input.i32()?;
        r.population_last = input.i32()?;
        r.population_mean = input.i32()?;
        r.mean_happiness = input.i32()?;
        r.mean_health = input.i32()?;
        r.share_of_map_pct = input.i32()?;
        r.army_count = input.u8()?;
        r.total_men = input.i32()?;
        for slot in 0..r.score_inputs.len() {
            r.score_inputs[slot] = input.i32()?;
        }
        Ok(r)
    }
}

impl Encode for History {
    /// All four hundred slots, not only the live ones.
    ///
    /// The ring is 400 × 16 × `{i32, i8}` and writing it whole costs 32,000
    /// bytes. Writing only the live window would be smaller and would need the
    /// reader to reconstruct which slots the writer considered empty — an
    /// invariant restated in a second place, which is how the two drift apart.
    fn encode(&self, out: &mut Canonical) {
        out.u32(self.head as u32);
        out.u32(self.tail as u32);
        out.u32(self.len as u32);
        out.u32(HISTORY_SEASONS as u32);
        out.u32(HISTORY_COUNTIES as u32);
        for season in &self.entries {
            for entry in season {
                out.i32(entry.population);
                out.i8(entry.happiness);
            }
        }
    }
}

fn decode_history(input: &mut Reader<'_>) -> Result<History, LoadError> {
    let head = input.u32()? as usize;
    let tail = input.u32()? as usize;
    let len = input.u32()? as usize;
    let seasons = input.u32()? as usize;
    let counties = input.u32()? as usize;
    if seasons != HISTORY_SEASONS || counties != HISTORY_COUNTIES {
        return Err(LoadError::CorruptHistory { head, tail, len });
    }
    if head >= HISTORY_SEASONS || tail >= HISTORY_SEASONS || len > HISTORY_SEASONS {
        return Err(LoadError::CorruptHistory { head, tail, len });
    }
    let mut history = History::new();
    history.head = head;
    history.tail = tail;
    history.len = len;
    for season in 0..HISTORY_SEASONS {
        for county in 0..HISTORY_COUNTIES {
            history.entries[season][county] =
                HistoryEntry { population: input.i32()?, happiness: input.i8()? };
        }
    }
    Ok(history)
}

// ---------------------------------------------------------------------------
// The ruleset fingerprint
// ---------------------------------------------------------------------------

/// The whole of [`Tables`], written out so it can be hashed.
///
/// Encode only: the save does not carry a ruleset, it carries this hash. Every
/// field is here, and `tests/save.rs` mutates each sub-table in turn to check
/// that the hash notices — which is the guard against a constant being added to
/// `Tables` and quietly left out of the fingerprint.
impl Encode for Tables {
    fn encode(&self, out: &mut Canonical) {
        out.section("food");
        out.i32(self.food.dairy_per_head);
        out.i32(self.food.food_per_head);
        out.i32(self.food.food_per_sack);

        out.section("grain");
        out.i32(self.grain.yield_per_sack);
        out.i32(self.grain.max_sacks_per_field);
        out.i32(self.grain.labour_divisor_advanced);
        out.i32(self.grain.labour_divisor_basic);

        out.section("field");
        out.i32(self.field.progress_max);
        out.i32(self.field.reclaim_per_season);

        out.section("event");
        out.i32(self.event.population_cap_pct);
        out.i32(self.event.first_year);

        out.section("season");
        for row in &self.season {
            out.i32(row.death_rate);
            out.i32(row.dryness);
        }

        out.section("ration");
        for row in &self.ration {
            out.i32(row.divisor);
            out.i32(row.multiplier);
            for delta in &row.health_delta {
                out.i32(*delta);
            }
        }
        out.i32(self.ration_happiness_slope);
        out.i32(self.ration_happiness_offset);

        out.section("health");
        for row in &self.health {
            out.i32(row.happiness);
            out.i32(row.death_rate);
        }
        for (threshold, band) in &self.health_band_ladder {
            out.i32(*threshold);
            out.i32(*band);
        }

        out.section("population");
        for (up_to, percent) in &self.population.birth_rate_ladder {
            out.i32(*up_to);
            out.i32(*percent);
        }
        for (below, percent) in &self.population.happiness_factor_ladder {
            out.i32(*below);
            out.i32(*percent);
        }

        out.section("weather");
        for row in &self.weather {
            out.i32(row.herd_pct);
        }

        out.section("castle");
        out.u8(self.castle.starting_type);
        for v in &self.castle.tax_base {
            out.i32(*v);
        }
        for v in &self.castle.tax_bonus_pct {
            out.i32(*v);
        }
        for (wood, stone) in &self.castle.cost {
            out.i32(*wood);
            out.i32(*stone);
        }
        for (a, b) in &self.castle.workforce {
            out.i32(*a);
            out.i32(*b);
        }
        for v in &self.castle.garrison_cap {
            out.i32(*v);
        }
        for v in &self.castle.free_archers {
            out.i32(*v);
        }

        out.section("commodity");
        for row in &self.commodity {
            out.u32(row.job as u32);
            out.i32(row.divisor);
            out.i32(row.base_efficiency);
        }

        out.section("job");
        out.u32(self.job.count as u32);
        out.u32(self.job.iron_mining as u32);
        out.u32(self.job.stone_quarrying as u32);
        out.u32(self.job.wood_cutting as u32);
        out.u32(self.job.blacksmith as u32);
        out.u32(self.job.grain_farming as u32);
        out.u32(self.job.castle_building as u32);

        out.section("weapon");
        for row in &self.weapon {
            out.i32(row.wood);
            out.i32(row.iron);
        }

        out.section("good");
        for row in &self.good {
            out.i32(row.sell_price);
        }

        out.section("wages");
        out.i32(self.wages.divisor_human);
        for v in &self.wages.divisor_ai {
            out.i32(*v);
        }
        out.u8(self.wages.bankrupt_stage_max);

        out.section("efficiency");
        out.i32(self.efficiency.max);
        out.i32(self.efficiency.without_advanced_farming);

        out.section("ale");
        out.i32(self.ale.step_pct);
        out.i32(self.ale.max);

        out.section("army_happiness");
        for v in &self.army_happiness_cost {
            out.i32(*v);
        }

        out.section("ai");
        for row in &self.ai.gold_grant {
            for v in row {
                out.i32(*v);
            }
        }
        out.i32(self.ai.grant_population_per_difficulty);
        out.i32(self.ai.grant_herd_per_difficulty);
        out.i32(self.ai.grant_grain_per_difficulty);
        out.i32(self.ai.grant_min_population);
        out.i32(self.ai.grant_min_herd);
        out.i32(self.ai.grant_min_grain);
        for (threshold, rate) in &self.ai.tax_ladder_neutral {
            out.i32(*threshold);
            out.i32(*rate);
        }
        for ladder in &self.ai.tax_ladders {
            for (threshold, rate) in ladder {
                out.i32(*threshold);
                out.i32(*rate);
            }
        }
        for row in &self.ai.personality {
            out.u8(row.farm_style);
            out.u32(row.tax_ladder as u32);
        }

        out.section("score");
        for (at_least, points) in &self.score.gold_brackets {
            out.i32(*at_least);
            out.i32(*points);
        }
        for (numerator, denominator) in &self.score.weights {
            out.i32(*numerator);
            out.i32(*denominator);
        }
        for offset in &self.score.input_offsets {
            out.u16(*offset);
        }
    }
}
