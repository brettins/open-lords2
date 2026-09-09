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
///
/// * 1 — the first layout.
/// * 2 — the county grew four herd fields (`docs/kingdom.md` §13), and the
///   ruleset fingerprint grew the tax-happiness table, the herd table and the
///   cattle-farming job slot. Both halves of the file moved, so a version 1
///   save is refused rather than misread.
/// * 3 — the **campaign layer** (`docs/armies.md`): the 151-slot unit array,
///   the map's three tile planes, the twelve mercenary bands and the realms'
///   army-name counters, plus three new county fields. A version 2 save has no
///   armies in it and no way to say so.
/// * 4 — **fields became tile-derived** (`crate::field`): the county carries
///   its twenty field tiles and the two counts `County_RecountFields` fills
///   besides the three we already had. A version 3 save records the counts but
///   not the tiles they were counted from, so its fields could never be
///   repainted; there is no way to recover the tiles from the counts, which is
///   why this is a refusal and not a default.
/// * 5 — **four county fields that were never written at all**:
///   `labour_wanted`, `labour_useful`, `labour_share` and `industry_share`.
///   Found by the *game* save's round trip over the England turn-one position
///   (`crates/l2-game/tests/save.rs`): a decoded kingdom compared equal on its
///   checksum and unequal on `PartialEq`, because the four were absent from the
///   `Encode` impl below and therefore absent from the hash as well. Two of
///   them — `labour_useful` and `labour_share` — are what the labour allocator
///   allocates *from*, so this was a hole in the lockstep checksum
///   (`docs/netcode.md` §5) and not only in the save. A version 4 save has the
///   four missing and no way to say what they held.
/// * 6 — `Options::fight_humans_only_byte`. It was a parameter threaded through
///   `l2-game`'s `engagement::resolve` while the save and the battle seam were
///   being written in parallel branches; now that both have landed it is where
///   it belongs, in the kingdom's own options, and therefore in the save and in
///   the lockstep checksum. A version 5 save does not carry it, and the option
///   changes whether a battle is fought or auto-resolved — so this is a refusal
///   rather than a default, on the same grounds as version 5.
/// * 7 — **the grain year got a memory**: `fields_grain_sown` (county `+0x202`)
///   and `sow_shortfall` (`+0x1A7`). `Grain_Grow` and `Grain_Harvest` scale the
///   standing crop by the grain fields still standing *against the fields that
///   were sown*, so a save that dropped the second number would resume a
///   half-grown crop with no basis to measure it against. A version 5 save has
///   both missing, and defaulting `fields_grain_sown` to 0 would silently mean
///   "no fields were sown" — which is not a default, it is a different game.
///
///   **This arrived as its own version 6.** Two branches bumped 5 → 6 in
///   parallel, each adding different fields, and the merged encoding is neither
///   of their version 6s — so it is 7. A version 6 save is a real thing that
///   exists (it carries `fight_humans_only_byte` and *not* these two), which is
///   why the number had to move rather than the two entries being folded
///   together.
/// * 8 — **the things that move now move** (`crate::units_tick`): the six
///   merchant trade routes `g_merchantRoutes`, the peasant mobs' shared
///   destination cursor, and a transport's cargo county on the unit record.
///   All three are state a turn *reads and writes* — a version 7 save would
///   load with six empty routes and every merchant would stand still for ever,
///   which is a silently different game rather than a missing feature.
///
///   **And it happened again, exactly as the entry above predicted.** This
///   arrived as its own version 7 from a third parallel branch, was read off
///   the changelog on merge, and moved to 8. That is now twice in one day, so
///   the note above should be taken as a standing hazard rather than an
///   anecdote: **the version number is the one field in this file that two
///   branches will always collide on**, because every branch that changes the
///   layout has to touch it and none of them can see the others. A check like
///   `tools/decisions/corrections.js` — which catches exactly this for
///   correction numbers — is the fix, and it does not exist for this constant.
/// * 9 — **the AI's farming style and its weapon rota**: county `farm_style`
///   (`+0x1FE`) and realm `weapon_rota` (`+0x6C`). Neither is derivable from the
///   rest of the file. An *unowned* county's style is whichever lord held it
///   last and `AI_ManageFields(0)` only reads it, so a save without it cannot
///   say how a county that has changed hands should be farmed; and AI step 12
///   advances the rota cursor once per county, so a game reloaded without it
///   restarts every AI's weapon programme from the top. See
///   [`crate::ai_farm`].
///
///   **Three times in one day, and this one is the third.** This arrived as its
///   own version 5, was moved to 7 on one merge and to 9 on the next, both
///   times by reading this changelog. The check the two entries above ask for
///   now exists: `the_version_is_ahead_of_its_own_changelog` in
///   `tests/save.rs` reads this file, scans the comment for its `* N —`
///   entries, and fails unless [`VERSION`] is greater than every one of them
///   and the numbering has no gap and no repeat.
///
///   It cannot stop two branches picking the same number, because neither can
///   see the other. What it *can* do is fail the instant they are merged —
///   which is where all three of these were caught by hand, twice by an
///   integrator reading a doc comment. A merge that takes one branch's `VERSION`
///   and both branches' changelog entries now goes red rather than shipping a
///   number that means two different layouts.
/// * 10 — **sieges** (`crate::siege`). The unit grew the three siege-engine
///   build records and the seasons countdown; `County::castle_degraded` grew
///   from a `bool` to the three-valued byte it always was, and the county grew
///   `castle_ruined` and `castle_level_left` beside it. `Unit::defence_mark`
///   joins them: it existed before and was in neither the save nor the
///   checksum, which is C30's shape exactly, and it is the byte that decides
///   whether winning a battle also wins the county. An older save has no siege
///   in it, but it also cannot say what its `castle_degraded` bytes meant — a
///   `true` could be either 1 or 2 and the two fight different castles — so
///   this is a refusal rather than a widening.
///
///   **Three branches found `defence_mark` missing, independently, within a
///   day.** The sharpest statement of it is the unit-import branch's: entry 8
///   added `cargo_county`, the *other* meaning of the same `+0x167` byte, and
///   left the field beside it unwritten. Two meanings sharing one offset, one
///   encoded and one not, added in the same neighbourhood by different hands.
///   It was invisible for version 5's reason: every unit that had ever
///   existed was one a test built by hand, and a hand-built unit carries 0 in
///   it. It surfaced the moment `l2-scenario` began importing `g_units` from
///   a save — `battle-during.sav` slot 6 carries a 1 — and it surfaced the
///   same way version 5's four did: a round trip equal on the checksum and
///   unequal on `PartialEq`.
///
///   **And a fourth collision, on the same day as the other three.** This
///   arrived as its own version 7, was rebased to 9, and is 10 here because the
///   entry above took 9 first. The standing hazard above is now the rule rather
///   than the exception: assume the number has moved under you. This is the
///   first collision the check above would have caught on its own — a merge
///   taking one branch's `VERSION` and both branches' entries leaves a repeat,
///   and the repeat is what it tests for.
/// * 11 — **the diplomacy record**: `Realm`'s
///   `offer_pending`, `ally_candidate`, `ally`, the six-slot `pairs` block,
///   `target_county`, `taunt_timer`, `taunt_stage`, `war_target`,
///   `offer_timer`, `crowned_once` and `voice_rotation`. (The twelfth field its
///   census found, `Unit::defence_mark`, went in with the sieges at entry 10 —
///   the two branches found it independently and within a day of each other.)
///   Every one of them is simulation state — `pairs` is what
///   `docs/diplomacy.md` §1 *is*: standing, alliance, grudge, at-war and the
///   gift history between every pair of realms — and none of them reached
///   these bytes, so none of them reached the lockstep digest either. Two peers
///   could diverge on the whole diplomatic state of a game and every checksum
///   they exchanged would agree.
///
///   **C30 for the third time, and the first time it was not luck that found
///   it**: `every_field_of_the_state_is_furnished` in `tests/save.rs` derives
///   the field list from the struct definitions, and these twelve were its
///   first run's output. `tests/save_gap.rs`, which pinned eleven of them, is
///   deleted with them — the gap it described is closed.
///
///   A version 10 save has all eleven missing. **Refusal rather than default**,
///   and `pairs` is why: a defaulted pair block is not "no diplomacy", it is
///   every alliance broken, every grudge forgotten and every standing reset to
///   the same number, which is a different game silently resumed.
///
///   **And this one is the fourth collision, caught by the check rather than by
///   an integrator.** It was written as version 8, then 9, and
///   `the_version_is_ahead_of_its_own_changelog` — added by the branch above,
///   independently and for the same reason — failed the merge both times and
///   named the duplicate. That is the entry above working exactly as it says.
/// * 12 — **`Options::exploration` and `Options::time_limit`**, the last two of
///   the six *rule* options the setup screen sets. Both are globals the
///   original's own `Save_Write` stores (`g_optExploration` `0x0053F264`,
///   `g_optTimeLimit` `0x0053F26C`) and both were being read out of a `.sav` by
///   `l2_formats::save::Globals` and then dropped on the floor, because
///   `Options` had nowhere to put them. Neither is read by a rule in this
///   crate — exploration's behaviour is unimplemented (`docs/mechanics.md`) and
///   a wall clock is not a rule — but a game that was started with a four
///   minute turn limit and no fog is a different game from one that was not,
///   and a save that cannot say which is a save that guesses.
///
///   **Refusal rather than default.** A version 11 save carries neither, and
///   defaulting both to off would silently claim a setting the file never made.
/// * 13 — **the merchant's books**: `County::purse` and the four `Realm` trade
///   accumulators (`trade_spent_a`/`_b`, `trade_received_a`/`_b`). All five are
///   written by [`crate::trade::trade`] and by nothing else, and until the
///   merchant screen existed no code path could make any of them non-zero — so
///   they were absent from the record, the save and the lockstep digest at once,
///   which is C30's shape for the fifth time. A trade produces them now, and an
///   unowned county's purse in particular is *simulation* state: the AI trades
///   out of it for every county nobody owns, so two peers that disagreed about a
///   purse would deal different goods and every checksum they exchanged would
///   still agree.
///
///   **Refusal rather than default**, on the same reasoning as entry 12: a
///   version 12 save was written by a build that could not trade, so all five
///   really are zero in it — but a save is not the place to be right by
///   accident, and the next version that widens this file would inherit a
///   default nobody checked.
///
///   *This entry was written as 13 with `VERSION` at 12 on `main`. Per the
///   standing hazard above, assume the number has moved: a merge that finds 13
///   taken renumbers this entry and the constant together.*
/// * 14 — **the castle's build record**: `County::castle_percent`,
///   `castle_work_left`, `castle_work_total`, `castle_stone_owed`,
///   `castle_stone_total`, `castle_wood_owed` and `castle_wood_total`, in place
///   of the single `castle_progress` counter this crate invented. County
///   `+0x1C4` and `+0x1CC … +0x1E0` in the original, and the reason the swap is
///   a version rather than a rename is that **the model changed with them**:
///   the materials are drawn down season by season rather than paid up front,
///   the work counts down rather than up, and `castle_building` holds the
///   castle you *had* rather than the one you are getting
///   (`County::castle_building`). A version 13 save's `castle_progress` cannot
///   be translated into any of them — the same number means "work done" there
///   and nothing here — and its `castle_building` byte means the opposite of
///   what this build would read.
///
///   **Refusal rather than a translation**, and this one is not conservatism:
///   `castle_degraded` had **no reachable writer** before this version, so
///   every version 13 save in existence has it zero in every county and a
///   castle nobody could have started. There is nothing to preserve.
///
///   **Two branches bumped to 14 on the same day with different contents, and
///   they are one version rather than 14 and 15.** Neither had shipped, so no
///   save exists that has one set of fields and not the other; numbering them
///   separately would invent a save nobody can hold and a translation nobody
///   can test. Both are refusals in any case.
///
///   **And, from a second branch that bumped to the same version on the same's war plan** ([`crate::ai_army`]): `Unit`'s `mission` and
///   day, the AI
///   `mission_county`, and `Realm`'s `muster_county`, `raid_county`,
///   `muster_timer`, `threat_realm`, `attack_county`, `raid_timer` and the
///   four-slot `want`.
///
///   Every one of them is a *standing order that persists between turns*, which
///   is exactly what a save is for. `mission` is the sharpest: it is the byte
///   AI step 11 dispatches on, so a save that dropped it would reload every AI
///   army as an attacker — **including the garrisons**, which would then
///   discover their county was still theirs, do nothing, and be marched out of
///   their castles the first time anything took one. The realm fields are
///   softer and no less real: a reloaded game would restart every lord's muster
///   and raid counters at zero, forget which county each realm musters from,
///   and forget the county its main army is currently marching on.
///
///   **Refusal rather than default**, on the same reasoning as entries 12 and
///   13: a version 13 save was written by a build whose AI raised no armies at
///   all, so the fields really are zero in it — but zero is a *meaningful*
///   value for `mission` (it is the one the dispatcher normalises), and a save
///   is not the place to be right by accident.
///
///   *Written as 14 with `VERSION` at 13 on `main`. Per the standing hazard
///   above, assume the number has moved.*
///
///   **And a third branch, same day, same version:** **`Options::quirks`**, the bitfield saying which of the original's
///   defects this game reproduces (`docs/bugs.md`, `docs/decisions.md` C62).
///   It changes what the simulation computes, so it is state: a save that did
///   not carry it would resume a fixed game as a faithful one, and a lockstep
///   peer that never exchanged it would desync.
///
///   **This is the only version bump the quirk set will ever cost**, which is
///   why the field is a `u64` bitfield rather than a run of `bool`s, and why a
///   set bit means *fixed* rather than *reproduced*: adding a quirk next month
///   sets a bit that is already being written as zero, and zero already means
///   the original's behaviour. `docs/bugs.md` §6.3 asked that the bump be paid
///   once rather than once per bug; this is how it is paid once.
///
///   **Refusal rather than default**, as for 12 and 13 — and here the default
///   would even have been right, since a version 13 save was written by a build
///   that had no quirks and so was faithful. It is refused anyway, because *"the
///   default happens to be correct this time"* is the reasoning that makes the
///   next widening wrong.
/// * 15 — **the diplomatic inbox** ([`crate::diplomacy`]): six realms of five
///   `g_diploInbox` slots, the outstanding pay-for-help price and county, and
///   the generator the three bargaining replies roll.
///
///   The eleven fields entry 11 added are a realm's *opinions*; these are the
///   letters in flight. `Diplo_Post` moves a gift's gold the moment it is
///   posted and the reply arrives a turn later, so a save taken between the two
///   that dropped the slot would take the money and never answer — and a
///   pay-for-help prompt reloaded without its price would ask for nothing.
///
///   **Refusal rather than default**, and here for a reason none of the earlier
///   entries had: the pair block *is* carried by a version 14 save, so a
///   defaulted inbox would produce a kingdom that looks entirely coherent — the
///   standings, alliances and grudges all present and correct — with the mail
///   silently thrown away. A wrong load that looks right is the one to refuse.
pub const VERSION: u32 = 15;

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
    /// The unit array is not [`crate::unit::MAX_UNITS`] slots.
    UnitCount(u32),
    /// The map's planes are not [`crate::map::MAP_TILES`] bytes each.
    MapSize(u32),
    /// More mercenary bands in play than there are bands.
    BandCount(u32),
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
            LoadError::UnitCount(n) => {
                write!(f, "{n} unit slots, and the array holds {}", crate::unit::MAX_UNITS)
            }
            LoadError::MapSize(n) => {
                write!(f, "a map plane of {n} tiles, and the map is {}", crate::map::MAP_TILES)
            }
            LoadError::BandCount(n) => write!(
                f,
                "{n} mercenary bands in play, and there are {}",
                crate::mercenary::MERCENARY_BANDS
            ),
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
        out.u8(self.options.fight_humans_only_byte);
        out.bool(self.options.exploration);
        out.i32(self.options.time_limit);
        // One `u64`, inside the `options` section, which is inside the per-tick
        // digest. That placement *is* the design: see `Options::quirks` and
        // entry 14 of `VERSION` above.
        out.encode(&self.options.quirks);

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

        out.section("campaign");
        encode_campaign(&self.campaign, out);

        out.section("diplomacy");
        encode_diplomacy(&self.diplomacy, out);
    }
}

/// The diplomatic state that is not inside a realm record — `VERSION` 15.
///
/// Fixed-width and index-ordered like everything else: six realms of five
/// slots, written whether or not they hold a letter, because *"realm 3's inbox
/// is empty"* is state a lockstep peer has to agree about.
///
/// The dice go out as their two `u64` parts for the reason
/// [`crate::Kingdom::rng`] does: a generator that reloaded at its seed would
/// give a reloaded game different answers to the same alliance offer, which is
/// the divergence `docs/netcode.md` D-3 is about.
fn encode_diplomacy(d: &crate::diplomacy::Diplomacy, out: &mut Canonical) {
    out.u32(MAX_REALMS as u32);
    out.u32(crate::diplomacy::INBOX_SLOTS as u32);
    for realm in &d.inbox {
        for slot in realm {
            out.u8(slot.from);
            out.u8(slot.kind);
            out.u8(slot.county);
            out.i32(slot.gold);
        }
    }
    out.i32(d.help_price);
    out.u8(d.help_county);
    let (state, increment) = d.dice.parts();
    out.u64(state);
    out.u64(increment);
}

fn decode_diplomacy(input: &mut Reader<'_>) -> Result<crate::diplomacy::Diplomacy, LoadError> {
    let realms = input.u32()? as usize;
    let slots = input.u32()? as usize;
    if realms != MAX_REALMS || slots != crate::diplomacy::INBOX_SLOTS {
        return Err(LoadError::Malformed(CodecError::BadTag {
            tag: realms.min(255) as u8,
            expected: "diplomacy inbox shape",
            at: input.position(),
        }));
    }
    let mut d = crate::diplomacy::Diplomacy::new(0);
    for realm in 0..MAX_REALMS {
        for slot in 0..crate::diplomacy::INBOX_SLOTS {
            d.inbox[realm][slot] = crate::diplomacy::InboxSlot {
                from: input.u8()?,
                kind: input.u8()?,
                county: input.u8()?,
                gold: input.i32()?,
            };
        }
    }
    d.help_price = input.i32()?;
    d.help_county = input.u8()?;
    let state = input.u64()?;
    let increment = input.u64()?;
    d.dice = crate::diplomacy::Dice::from_parts(state, increment);
    Ok(d)
}

/// The campaign layer — `docs/armies.md`.
///
/// Written last so that a reader of a hex dump meets the economy in the order
/// `docs/kingdom.md` describes it and the war after. Everything is
/// fixed-width and index-ordered like the rest of the file: the unit array is
/// 151 slots with a presence byte apiece rather than a count and a list,
/// because "slot 7 is empty" is state a lockstep peer has to agree about and a
/// compacted list would renumber every unit above a casualty.
fn encode_campaign(campaign: &crate::kingdom::Campaign, out: &mut Canonical) {
    out.section("units");
    out.u32(crate::unit::MAX_UNITS as u32);
    for slot in 0..crate::unit::MAX_UNITS {
        match campaign.units.get(slot) {
            None => out.bool(false),
            Some(u) => {
                out.bool(true);
                u.encode(out);
            }
        }
    }

    out.section("map");
    out.u32(crate::map::MAP_TILES as u32);
    out.raw(&campaign.map.terrain);
    out.raw(&campaign.map.flags);
    out.raw(&campaign.map.county);

    out.section("mercenaries");
    out.u32(campaign.mercenaries.in_play() as u32);
    for band in 1..crate::mercenary::BAND_SLOTS {
        let b = campaign.mercenaries.band_raw(band);
        out.u16(b.hired_by);
        out.u8(b.offered_in);
        out.u8(b.next_county);
        out.i8(b.countdown);
        out.i8(b.reload);
    }

    out.section("army_names");
    for realm in 0..MAX_REALMS {
        out.raw(campaign.names.counters(realm as u8));
    }

    out.section("routes");
    for route in 0..crate::merchant::ROUTES {
        out.raw(campaign.routes.row(route));
    }
    out.u32(campaign.mob_cursor as u32);
}

fn decode_campaign(input: &mut Reader<'_>) -> Result<crate::kingdom::Campaign, LoadError> {
    let mut campaign = crate::kingdom::Campaign::new();

    let slots = input.u32()? as usize;
    if slots != crate::unit::MAX_UNITS {
        return Err(LoadError::UnitCount(slots as u32));
    }
    for slot in 0..crate::unit::MAX_UNITS {
        if input.bool()? {
            campaign.units.put(slot, crate::unit::Unit::decode(input)?);
        }
    }

    let tiles = input.u32()? as usize;
    if tiles != crate::map::MAP_TILES {
        return Err(LoadError::MapSize(tiles as u32));
    }
    for plane in [0usize, 1, 2] {
        let bytes = input.raw(crate::map::MAP_TILES)?;
        let target = match plane {
            0 => &mut campaign.map.terrain,
            1 => &mut campaign.map.flags,
            _ => &mut campaign.map.county,
        };
        target.copy_from_slice(bytes);
    }

    let in_play = input.u32()? as usize;
    if in_play > crate::mercenary::MERCENARY_BANDS {
        return Err(LoadError::BandCount(in_play as u32));
    }
    campaign.mercenaries.set_in_play(in_play);
    for band in 1..crate::mercenary::BAND_SLOTS {
        let b = crate::mercenary::Band {
            hired_by: input.u16()?,
            offered_in: input.u8()?,
            next_county: input.u8()?,
            countdown: input.i8()?,
            reload: input.i8()?,
        };
        campaign.mercenaries.set_band_raw(band, b);
    }

    for realm in 0..MAX_REALMS {
        let bytes = input.raw(crate::unit::ARMY_NAME_SLOTS)?;
        let mut row = [0u8; crate::unit::ARMY_NAME_SLOTS];
        row.copy_from_slice(bytes);
        campaign.names.set_counters(realm as u8, row);
    }

    for route in 0..crate::merchant::ROUTES {
        let bytes = input.raw(crate::merchant::ROUTE_SLOTS)?;
        let mut row = [0u8; crate::merchant::ROUTE_SLOTS];
        row.copy_from_slice(bytes);
        campaign.routes.set_row(route, row);
    }
    campaign.mob_cursor = input.u32()? as usize;
    Ok(campaign)
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
        fight_humans_only_byte: input.u8()?,
        exploration: input.bool()?,
        time_limit: input.i32()?,
        quirks: input.decode()?,
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
    k.campaign = decode_campaign(input)?;
    k.diplomacy = decode_diplomacy(input)?;
    Ok(k)
}

impl Encode for crate::unit::Unit {
    fn encode(&self, out: &mut Canonical) {
        out.u8(self.owner);
        out.bool(self.owner_is_human);
        out.u8(self.shield);
        out.bool(self.player_driven);
        out.u8(self.kind.byte());
        out.u8(self.facing);
        out.u8(self.x);
        out.u8(self.y);
        out.u8(self.county);
        out.u8(self.home_county);
        match self.dest {
            None => out.bool(false),
            Some((x, y)) => {
                out.bool(true);
                out.u8(x);
                out.u8(y);
            }
        }
        out.u32(self.path.len() as u32);
        for (x, y) in &self.path {
            out.u8(*x);
            out.u8(*y);
        }
        out.bool(self.moving);
        out.bool(self.on_road);
        out.u8(self.name_index);
        out.bool(self.needs_destination);
        out.u8(self.dest_county);
        out.i32(self.moves_used);
        out.i32(self.move_allowance);
        out.i32(self.starvation);
        out.i32(self.wages);
        out.i32(self.year_formed);
        out.i32(self.morale);
        out.i32(self.men);
        for count in &self.troops {
            out.i32(*count);
        }
        match self.mercenaries {
            None => out.bool(false),
            Some(m) => {
                out.bool(true);
                out.u8(m.band);
                out.u8(m.troop as u8);
                out.u8(m.men);
            }
        }
        out.u8(self.garrison_county);
        out.u8(self.besieging_county);
        out.u8(self.besieged_by);
        out.u8(self.cargo_county);
        // The mission byte and its county (`VERSION` 14). `+0x1A` is what AI
        // step 11 dispatches on, so a save that dropped it would reload every
        // army as an attacker � including the garrisons, which would then
        // walk out of their castles.
        out.u8(self.mission);
        out.u8(self.mission_county);
        // The siege state. `defence_mark` joins it here for the reason C30
        // gives: it was in no save and in no checksum, and it is the byte that
        // decides whether winning a battle also wins the county. It is
        // short-lived — written when a defence is found, read when the battle
        // returns — but a save taken between those two points loses the county.
        out.u8(self.defence_mark);
        for record in &self.engines {
            out.i16(record.ordered);
            out.i16(record.percent);
            out.i16(record.work_done);
        }
        out.u8(self.siege_seasons_left);
    }
}

impl Decode for crate::unit::Unit {
    fn decode(input: &mut Reader<'_>) -> Result<crate::unit::Unit, CodecError> {
        let owner = input.u8()?;
        let owner_is_human = input.bool()?;
        let shield = input.u8()?;
        let player_driven = input.bool()?;
        let tag = input.u8()?;
        let kind = crate::unit::UnitKind::from_byte(tag).ok_or(CodecError::BadTag {
            tag,
            expected: "unit type 1..=4",
            at: input.position() - 1,
        })?;
        let mut u = crate::unit::Unit::new(kind, owner, 0, 0);
        u.owner_is_human = owner_is_human;
        u.shield = shield;
        u.player_driven = player_driven;
        u.facing = input.u8()?;
        u.x = input.u8()?;
        u.y = input.u8()?;
        u.county = input.u8()?;
        u.home_county = input.u8()?;
        u.dest = if input.bool()? { Some((input.u8()?, input.u8()?)) } else { None };
        let steps = input.u32()? as usize;
        if steps > crate::unit::MAX_PATH {
            return Err(CodecError::BadTag {
                tag: steps.min(255) as u8,
                expected: "a path of at most 150 steps",
                at: input.position(),
            });
        }
        u.path = Vec::with_capacity(steps);
        for _ in 0..steps {
            u.path.push((input.u8()?, input.u8()?));
        }
        u.moving = input.bool()?;
        u.on_road = input.bool()?;
        u.name_index = input.u8()?;
        u.needs_destination = input.bool()?;
        u.dest_county = input.u8()?;
        u.moves_used = input.i32()?;
        u.move_allowance = input.i32()?;
        u.starvation = input.i32()?;
        u.wages = input.i32()?;
        u.year_formed = input.i32()?;
        u.morale = input.i32()?;
        u.men = input.i32()?;
        for slot in 0..crate::unit::TROOP_TYPES {
            u.troops[slot] = input.i32()?;
        }
        u.mercenaries = if input.bool()? {
            let band = input.u8()?;
            let tag = input.u8()?;
            let troop = crate::unit::TroopType::from_index(tag as usize).ok_or(CodecError::BadTag {
                tag,
                expected: "troop type 0..=6",
                at: input.position() - 1,
            })?;
            Some(crate::unit::Mercenaries { band, troop, men: input.u8()? })
        } else {
            None
        };
        u.garrison_county = input.u8()?;
        u.besieging_county = input.u8()?;
        u.besieged_by = input.u8()?;
        u.cargo_county = input.u8()?;
        u.mission = input.u8()?;
        u.mission_county = input.u8()?;
        u.defence_mark = input.u8()?;
        for record in u.engines.iter_mut() {
            record.ordered = input.i16()?;
            record.percent = input.i16()?;
            record.work_done = input.i16()?;
        }
        u.siege_seasons_left = input.u8()?;
        Ok(u)
    }
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
        out.i32(self.purse);
        for job in &self.labour {
            out.i32(*job);
        }
        // **The other three labour arrays and the industry split.** They were
        // missing until a game save round-tripped the England position and came
        // back with `County::new`'s defaults in all four; see [`VERSION`] 5.
        // `labour_useful` and `labour_share` are what `FUN_0044F6E7` allocates
        // *from*, so they are simulation state and not a display hint, and
        // leaving them out of the encoding left them out of the lockstep
        // checksum too.
        for job in &self.labour_wanted {
            out.i32(*job);
        }
        for job in &self.labour_useful {
            out.i32(*job);
        }
        for job in &self.labour_share {
            out.i32(*job);
        }
        out.i32(self.industry_share);
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
        out.u8(self.mercenary_offer);
        out.u32(self.garrison_unit as u32);
        out.i32(self.levy_surcharge);
        out.u8(self.castle_type);
        out.u8(self.castle_building);
        out.u8(self.castle_degraded);
        out.bool(self.castle_ruined);
        out.u8(self.castle_level_left);
        out.bool(self.castle_switch);
        out.u8(self.castle_percent);
        out.i32(self.castle_work_left);
        out.i32(self.castle_work_total);
        out.i32(self.castle_stone_owed);
        out.i32(self.castle_stone_total);
        out.i32(self.castle_wood_owed);
        out.i32(self.castle_wood_total);
        out.i32(self.event_population_pct);
        out.i32(self.event_grain_pct);
        out.i32(self.event_herd_pct);
        for tile in &self.field_tiles {
            out.u16(*tile);
        }
        out.i32(self.fields_fallow);
        out.i32(self.fields_cattle);
        out.i32(self.fields_grain);
        out.i32(self.fields_waste);
        out.i32(self.fields_reclaiming);
        out.i32(self.fertility);
        out.u8(self.weather.index());
        out.i32(self.dryness);
        out.i32(self.grain);
        for stage in &self.crop {
            out.i32(*stage);
        }
        out.i32(self.fields_grain_sown);
        out.bool(self.sow_shortfall);
        out.i32(self.herd);
        out.i32(self.herd_crowding);
        out.i32(self.herd_births_expected);
        out.i32(self.herd_deaths_expected);
        out.i32(self.herd_change_expected);
        for industry in &self.industry {
            industry.encode(out);
        }
        out.u32(self.weapon_type as u32);
        out.u8(self.farm_style);
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
        c.purse = input.i32()?;
        for job in 0..JOB_COUNT {
            c.labour[job] = input.i32()?;
        }
        for job in 0..JOB_COUNT {
            c.labour_wanted[job] = input.i32()?;
        }
        for job in 0..JOB_COUNT {
            c.labour_useful[job] = input.i32()?;
        }
        for job in 0..JOB_COUNT - 1 {
            c.labour_share[job] = input.i32()?;
        }
        c.industry_share = input.i32()?;
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
        c.mercenary_offer = input.u8()?;
        c.garrison_unit = input.u32()? as usize;
        c.levy_surcharge = input.i32()?;
        c.castle_type = input.u8()?;
        c.castle_building = input.u8()?;
        c.castle_degraded = input.u8()?;
        c.castle_ruined = input.bool()?;
        c.castle_level_left = input.u8()?;
        c.castle_switch = input.bool()?;
        c.castle_percent = input.u8()?;
        c.castle_work_left = input.i32()?;
        c.castle_work_total = input.i32()?;
        c.castle_stone_owed = input.i32()?;
        c.castle_stone_total = input.i32()?;
        c.castle_wood_owed = input.i32()?;
        c.castle_wood_total = input.i32()?;
        c.event_population_pct = input.i32()?;
        c.event_grain_pct = input.i32()?;
        c.event_herd_pct = input.i32()?;
        for slot in 0..c.field_tiles.len() {
            c.field_tiles[slot] = input.u16()?;
        }
        c.fields_fallow = input.i32()?;
        c.fields_cattle = input.i32()?;
        c.fields_grain = input.i32()?;
        c.fields_waste = input.i32()?;
        c.fields_reclaiming = input.i32()?;
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
        c.fields_grain_sown = input.i32()?;
        c.sow_shortfall = input.bool()?;
        c.herd = input.i32()?;
        c.herd_crowding = input.i32()?;
        c.herd_births_expected = input.i32()?;
        c.herd_deaths_expected = input.i32()?;
        c.herd_change_expected = input.i32()?;
        for slot in 0..c.industry.len() {
            c.industry[slot] = Industry::decode(input)?;
        }
        c.weapon_type = input.u32()? as usize;
        c.farm_style = input.u8()?;
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
        // Realm `+0x0A`, the banner. It was missing from this codec until the
        // campaign layer arrived and `tests/campaign.rs` caught it: no rule in
        // the economy reads it, so nothing noticed, and `County_ChangeOwner`
        // reads it now — a captured county draws its new owner's shield.
        out.u8(self.shield_index);
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
        out.i32(self.trade_spent_a);
        out.i32(self.trade_spent_b);
        out.i32(self.trade_received_a);
        out.i32(self.trade_received_b);
        out.i32(self.weapon_rota);
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

        // The diplomacy record (`docs/diplomacy.md` §1). It landed on `main`
        // outside this file and was absent from the encoding — and therefore
        // from the lockstep digest — until the census below found it. See
        // [`VERSION`] 8.
        out.bool(self.offer_pending);
        out.u8(self.ally_candidate);
        out.u8(self.ally);
        for pair in &self.pairs {
            pair.encode(out);
        }
        out.u8(self.target_county);
        out.u8(self.taunt_timer);
        out.u8(self.taunt_stage);
        out.u8(self.war_target);
        out.i8(self.offer_timer);
        out.bool(self.crowned_once);
        out.u8(self.voice_rotation);

        // The war plan (`VERSION` 14). Seven fields the AI writes in steps 7,
        // 9 and 10 and reads again next turn, plus the four resource wants
        // step 4 fills. A realm that reloaded without them would forget which
        // county it musters from, restart every lord's muster and raid
        // counters at zero, and lose the county its main army is marching on.
        out.u8(self.muster_county);
        out.u8(self.raid_county);
        out.u8(self.muster_timer);
        out.u8(self.threat_realm);
        out.u8(self.attack_county);
        out.u8(self.raid_timer);
        for want in &self.want {
            out.i32(*want);
        }
    }
}

impl Encode for crate::realm::Pair {
    fn encode(&self, out: &mut Canonical) {
        out.i8(self.standing);
        out.bool(self.allied);
        out.u8(self.grudge);
        out.u8(self.warnings_sent);
        out.bool(self.at_war);
        out.u8(self.compliments_from);
        out.i32(self.best_gift);
        out.bool(self.has_mail);
        out.u8(self.help_price_multiple);
    }
}

impl Decode for crate::realm::Pair {
    fn decode(input: &mut Reader<'_>) -> Result<crate::realm::Pair, CodecError> {
        Ok(crate::realm::Pair {
            standing: input.i8()?,
            allied: input.bool()?,
            grudge: input.u8()?,
            warnings_sent: input.u8()?,
            at_war: input.bool()?,
            compliments_from: input.u8()?,
            best_gift: input.i32()?,
            has_mail: input.bool()?,
            help_price_multiple: input.u8()?,
        })
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
        r.shield_index = input.u8()?;
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
        r.trade_spent_a = input.i32()?;
        r.trade_spent_b = input.i32()?;
        r.trade_received_a = input.i32()?;
        r.trade_received_b = input.i32()?;
        r.weapon_rota = input.i32()?;
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
        r.offer_pending = input.bool()?;
        r.ally_candidate = input.u8()?;
        r.ally = input.u8()?;
        for slot in 0..MAX_REALMS {
            r.pairs[slot] = crate::realm::Pair::decode(input)?;
        }
        r.target_county = input.u8()?;
        r.taunt_timer = input.u8()?;
        r.taunt_stage = input.u8()?;
        r.war_target = input.u8()?;
        r.offer_timer = input.i8()?;
        r.crowned_once = input.bool()?;
        r.voice_rotation = input.u8()?;
        r.muster_county = input.u8()?;
        r.raid_county = input.u8()?;
        r.muster_timer = input.u8()?;
        r.threat_realm = input.u8()?;
        r.attack_county = input.u8()?;
        r.raid_timer = input.u8()?;
        for slot in 0..r.want.len() {
            r.want[slot] = input.i32()?;
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

        out.section("tax");
        for v in &self.tax_happiness_other {
            out.i32(*v);
        }

        out.section("weather");
        for row in &self.weather {
            out.i32(row.herd_pct);
        }

        out.section("herd");
        out.i32(self.herd.labour_per_head);
        out.i32(self.herd.staffing_max);
        out.i32(self.herd.understaffing_divisor);
        for row in &self.herd.crowding {
            out.i32(row.density_max);
            out.i32(row.level);
            out.i32(row.death_rate);
            out.i32(row.birth_rate);
        }
        for (below, bonus) in &self.herd.small_bonus {
            out.i32(*below);
            out.i32(*bonus);
        }
        out.i32(self.herd.no_pasture_density);
        out.i32(self.herd.no_pasture_kill_all_below);
        out.i32(self.herd.no_pasture_divisor);
        out.u8(self.herd.calving_season);
        out.u8(self.herd.culling_season);
        out.i32(self.herd.season_bonus.0);
        out.i32(self.herd.season_bonus.1);

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
        out.u32(self.job.cattle_farming as u32);
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
