//! **A saved game** — the whole [`Game`], as bytes, and back again.
//!
//! # It is `l2_kingdom::save` plus ten fields, and that is the whole design
//!
//! `crates/l2-kingdom/src/save.rs` already encodes a [`Kingdom`] through
//! `l2_net::Canonical`, the deterministic encoder `docs/netcode.md` §5 demands
//! for the per-tick checksum and the late-join snapshot. It refuses an unknown
//! version, it fingerprints the ruleset rather than storing it, and it has no
//! floats, no `usize` on the wire and no hash-ordered iteration.
//!
//! A saved *game* is that file with a short prefix: the ten plain fields
//! [`Game`] adds on top of the world — who the player is, which map slot the
//! scenario runs on, what colour each realm flies, which county is selected,
//! the county anchors, last turn's treasuries, the last season's report, and
//! how many turns have been ended.
//!
//! So this module **does not encode a kingdom**. It calls
//! [`l2_kingdom::save::encode`] and puts the result in the file whole, and on
//! the way back it hands those bytes straight to [`l2_kingdom::save::decode`]
//! and reports whatever that says. Writing a second kingdom encoder here would
//! mean a save whose bytes and a lockstep checksum whose bytes could disagree,
//! which is exactly the failure `l2-net`'s `canonical.rs` exists to prevent.
//!
//! # Two versions, and both refuse rather than guess
//!
//! The file has **two** version numbers and they are independent:
//!
//! * [`VERSION`], this module's, which changes when the ten-field prefix
//!   changes;
//! * `l2_kingdom::save::VERSION`, which changes when the world does — it is at
//!   4 today, and has been bumped three times already for the herd fields, the
//!   campaign layer and the tile-derived fields.
//!
//! Either one being unfamiliar is a refusal that names itself:
//! [`LoadError::UnsupportedVersion`] for ours,
//! [`LoadError::Kingdom`] wrapping `l2_kingdom::save::LoadError::UnsupportedVersion`
//! for the world's. Neither is ever read on the assumption that the fields
//! happen to line up — a plausible kingdom assembled out of a misread save is
//! worse than an error message, and a *half*-loaded one is worse still, which
//! is why [`decode`] builds a whole [`Game`] or returns `Err` and never touches
//! the caller's.
//!
//! # No `std::fs`
//!
//! Same rule as `l2_kingdom::save`, for the same reason. This module turns a
//! [`Game`] into a `Vec<u8>` and back; *where* those bytes live is
//! [`crate::saves`]'s business, and keeping the file system out is what lets
//! the round-trip test run with no directory, no permissions and no clean-up.

use l2_kingdom::county::MAX_COUNTIES;
use l2_kingdom::industry::BankruptcyAction;
use l2_kingdom::realm::MAX_REALMS;
use l2_kingdom::report::{Message, SeasonReport};
use l2_kingdom::tables::Tables;
use l2_kingdom::{EventKind, Kingdom, Pass, SEASON_PIPELINE};
use l2_net::canonical::{Canonical, CodecError, Reader};

use crate::game::Game;
use crate::screens::setup::MAP_COUNT;

/// Eight bytes, so a file command can name the format and a truncated file
/// fails on the magic rather than three fields later.
///
/// Deliberately one letter from `l2_kingdom::save::MAGIC` (`L2KSAVE\x01`): a
/// kingdom save and a game save are different files and neither should ever be
/// read as the other.
pub const MAGIC: [u8; 8] = *b"L2GSAVE\x01";

/// The version of **the prefix**, not of the world. Bump it whenever a field
/// below is added, removed or changes width, and never reinterpret an unknown
/// one.
///
/// * 1 — the first layout: the ten fields `Game` carries on top of `Kingdom`.
/// * 2 — the campaign section: which of the two campaigns, how many of its maps
///   have been won, whether this one is over, and the ending messages still
///   queued. Without it a saved campaign always resumed at map one.
/// * 3 — `g_playerNames`: six 31-byte lord names, in the realms section beside
///   the colours. Without it the name a person typed on setup page 4 lasted
///   until they saved.
/// * 4 — **the message ring**, in place of version 2's ending-message list. The
///   endings are no longer a queue of their own: they go into `g_messageQueue`
///   with every other message and are settled by being displayed and dismissed,
///   so what has to survive a save is the whole ring and the record on screen.
///   See [`crate::message`].
pub const VERSION: u32 = 4;

/// Magic, version, the prefix's length and the kingdom blob's length.
pub const HEADER_LEN: usize = 8 + 4 + 4 + 4;

/// The file extension we write. **Not `.sav`.** The original's `.sav` is a
/// memory dump with no header and no version which we read as an oracle
/// (`l2_formats::save`); this is our own format and confusing the two — in a
/// directory listing, in a bug report, or in `.gitignore` — costs more than the
/// four characters saves.
pub const EXTENSION: &str = "l2sav";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadError {
    /// The first eight bytes are not [`MAGIC`].
    NotASave,
    /// A prefix version this build does not know. **A refusal, not a guess.**
    UnsupportedVersion { found: u32, supported: u32 },
    /// The two declared lengths do not match what followed them.
    TruncatedBody { declared: usize, actual: usize },
    /// The body decoded but its checksum does not match the trailer's.
    Corrupt { expected: u64, actual: u64 },
    /// The bytes ran out, or a tag byte named nothing.
    Malformed(CodecError),
    /// The world would not load. The kingdom's own error, unchanged — an
    /// unknown `l2_kingdom::save::VERSION` and a ruleset mismatch both arrive
    /// here saying what they are.
    Kingdom(l2_kingdom::save::LoadError),
    /// `g_localPlayer` naming a realm that does not exist.
    Player(u8),
    /// A map slot outside the sixty `L2.eng` group 101 names.
    MapSlot(u32),
    /// A selected county that is not a county on this map. 0 — nothing
    /// selected — is legal; county ids are 1-based.
    Selected(u8),
    /// A pass index that is not a position in `SEASON_PIPELINE`.
    BadPass(u32),
    /// A season report tag that names no [`Message`] variant.
    BadMessage(u8),
    /// An event id the deck does not have.
    BadEvent(u16),
    /// A bankruptcy action tag that names no variant.
    BadBankruptcy(u8),
}

impl core::fmt::Display for LoadError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            LoadError::NotASave => write!(f, "not an open-lords2 saved game"),
            LoadError::UnsupportedVersion { found, supported } => write!(
                f,
                "saved game format version {found}; this build reads {supported}. \
                 Refusing rather than guessing at the layout"
            ),
            LoadError::TruncatedBody { declared, actual } => {
                write!(f, "the body declares {declared} bytes and {actual} follow")
            }
            LoadError::Corrupt { expected, actual } => {
                write!(f, "checksum {actual:#018x}, expected {expected:#018x}")
            }
            LoadError::Malformed(e) => write!(f, "{e}"),
            LoadError::Kingdom(e) => write!(f, "the world in this save: {e}"),
            LoadError::Player(p) => {
                write!(f, "realm {p} drives this game, and there are {MAX_REALMS} realm slots")
            }
            LoadError::MapSlot(s) => write!(f, "map slot {s}, and there are {MAP_COUNT}"),
            LoadError::Selected(c) => {
                write!(f, "county {c} is selected and it is not a county on this map")
            }
            LoadError::BadPass(p) => {
                write!(f, "season pass {p}, and the pipeline has {}", SEASON_PIPELINE.len())
            }
            LoadError::BadMessage(t) => write!(f, "season message tag {t} names nothing"),
            LoadError::BadEvent(id) => write!(f, "event {id:#x} is not in the deck"),
            LoadError::BadBankruptcy(t) => write!(f, "bankruptcy action tag {t} names nothing"),
        }
    }
}

impl std::error::Error for LoadError {}

impl From<CodecError> for LoadError {
    fn from(e: CodecError) -> LoadError {
        LoadError::Malformed(e)
    }
}

impl From<l2_kingdom::save::LoadError> for LoadError {
    fn from(e: l2_kingdom::save::LoadError) -> LoadError {
        LoadError::Kingdom(e)
    }
}

/// What the header says, without decoding anything behind it.
///
/// The load screen reads this for every file it lists, so a save it cannot read
/// can be *named* as unreadable in the list rather than only when somebody
/// clicks it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header {
    pub version: u32,
    pub prefix_len: usize,
    pub kingdom_len: usize,
}

/// Read the header and nothing else. Cheap, and it is the only thing the file
/// list needs.
pub fn peek(bytes: &[u8]) -> Result<Header, LoadError> {
    if bytes.len() < HEADER_LEN + 8 || bytes[..8] != MAGIC {
        return Err(LoadError::NotASave);
    }
    let word = |at: usize| {
        u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]) as usize
    };
    let version = word(8) as u32;
    if version != VERSION {
        return Err(LoadError::UnsupportedVersion { found: version, supported: VERSION });
    }
    let prefix_len = word(12);
    let kingdom_len = word(16);
    let declared = prefix_len.saturating_add(kingdom_len);
    let actual = bytes.len() - HEADER_LEN - 8;
    if declared != actual {
        return Err(LoadError::TruncatedBody { declared, actual });
    }
    Ok(Header { version, prefix_len, kingdom_len })
}

/// A whole game as bytes.
///
/// Deterministic by construction: every write is a fixed width in
/// little-endian, every collection is an array walked by ascending index, and
/// the world's half is `l2_kingdom::save::encode` unchanged. The same game
/// encodes to the same bytes on any machine.
pub fn encode(game: &Game) -> Vec<u8> {
    let mut prefix = Canonical::recording();
    encode_prefix(game, &mut prefix);
    let prefix = prefix.finish().bytes.expect("a recording encoder keeps its bytes");
    let kingdom = l2_kingdom::save::encode(&game.kingdom);

    let mut out = Vec::with_capacity(HEADER_LEN + prefix.len() + kingdom.len() + 8);
    out.extend_from_slice(&MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.extend_from_slice(&(prefix.len() as u32).to_le_bytes());
    out.extend_from_slice(&(kingdom.len() as u32).to_le_bytes());
    out.extend_from_slice(&prefix);
    out.extend_from_slice(&kingdom);

    let mut c = Canonical::hashing();
    c.raw(&out[HEADER_LEN..]);
    out.extend_from_slice(&c.finish().hash.to_le_bytes());
    out
}

/// Read a saved game back, on a supplied ruleset.
///
/// `tables` is the ruleset the game is currently running — from `l2-mods`, or
/// `Tables::DEFAULT`. It must be the one the save was written under;
/// `l2_kingdom::save` fingerprints it and says so if it is not.
pub fn decode(bytes: &[u8], tables: Tables) -> Result<Game, LoadError> {
    let header = peek(bytes)?;

    let body = &bytes[HEADER_LEN..bytes.len() - 8];
    let mut trailer = [0u8; 8];
    trailer.copy_from_slice(&bytes[bytes.len() - 8..]);
    let expected = u64::from_le_bytes(trailer);
    let mut c = Canonical::hashing();
    c.raw(body);
    let actual = c.finish().hash;
    if actual != expected {
        return Err(LoadError::Corrupt { expected, actual });
    }

    // The world first, so the prefix's county checks have something to check
    // against. The file is laid out prefix-then-kingdom because the prefix is
    // small and fixed; the *reading* order is the other way round and nothing
    // requires them to agree.
    let kingdom = l2_kingdom::save::decode(&body[header.prefix_len..], tables)?;

    let mut reader = Reader::new(&body[..header.prefix_len]);
    let game = decode_prefix(&mut reader, kingdom)?;
    reader.finish()?;
    Ok(game)
}

// ---------------------------------------------------------------------------
// The prefix: what `Game` adds to `Kingdom`
// ---------------------------------------------------------------------------

fn encode_prefix(game: &Game, out: &mut Canonical) {
    out.section("player");
    out.u8(game.player);
    out.u32(game.map_slot as u32);
    out.u8(game.selected);
    out.u32(game.turns_played);

    out.section("realms");
    out.u32(MAX_REALMS as u32);
    out.raw(&game.realm_colour);
    // `g_playerNames`, fixed width and no length prefix, because that is what
    // the original's own save block is: six 44-byte records of which 31 bytes
    // are the name. See `Game::player_names`.
    for name in &game.player_names {
        out.raw(name.bytes());
    }
    for gold in &game.gold_last {
        out.i32(*gold);
    }

    out.section("anchors");
    out.u32(MAX_COUNTIES as u32);
    out.raw(&game.anchor_x);
    out.raw(&game.anchor_y);

    out.section("report");
    out.option(game.last_report.as_ref(), encode_report);

    // The campaign: `DAT_0053F640`, `DAT_0053F258` and `DAT_0053F0C4`.
    //
    // **The ending messages are no longer here; the whole ring is, below.** They
    // used to be a `Vec<Ending>` of their own with the note that the queue is
    // empty at every point a person can save. That stopped being true the moment
    // the messages were *shown* rather than settled headlessly: a person can now
    // save on the campaign map with three obituaries still queued behind the one
    // on screen, and a save that dropped them would be a save that can never be
    // won. `docs/decisions.md` CNEW-msg-ring-in-the-save.
    out.section("campaign");
    let c = &game.campaign;
    out.u8(match c.track {
        crate::victory::Track::First => 0,
        crate::victory::Track::Second => 1,
    });
    out.u32(c.map as u32);
    out.u8(c.outcome.value());
    out.u8(c.ranking.leader);
    out.u8(c.ranking.trailer);
    out.u8(c.ranking.opponents_remaining);
    out.u8(c.ranking.realms_in_play);

    encode_messages(out, game);
}

/// **The message ring**, `g_messageQueue` and the window over it.
///
/// Written as a flat list of the records still waiting plus the one on screen,
/// rather than as fifty slots and two cursors: the cursors are an implementation
/// of a queue and the queue is what has to survive. Reloading rebuilds the ring
/// from index 0, which is where `Msg_Reset` puts it.
fn encode_messages(out: &mut Canonical, game: &Game) {
    out.section("messages");
    let q = &game.messages;
    let open = q.open().copied();
    out.u8(u8::from(open.is_some()));
    if let Some(r) = open {
        encode_record(out, &r);
        out.u32(q.timer() as u32);
    }
    let waiting = q.waiting();
    out.u32(waiting.len() as u32);
    for r in &waiting {
        encode_record(out, r);
    }
}

fn encode_record(out: &mut Canonical, r: &crate::message::Record) {
    out.u8(r.to);
    out.u8(r.from);
    out.u32(r.group as u32);
    out.u8(r.variant);
    out.u8(r.category);
    out.u8(r.county);
    out.u8(r.spare);
    out.u32(r.payload as u32);
}

fn decode_record(input: &mut Reader<'_>) -> Result<crate::message::Record, LoadError> {
    Ok(crate::message::Record {
        to: input.u8()?,
        from: input.u8()?,
        group: input.u32()? as u16,
        variant: input.u8()?,
        category: input.u8()?,
        county: input.u8()?,
        spare: input.u8()?,
        payload: input.u32()? as i32,
    })
}

fn decode_messages(input: &mut Reader<'_>) -> Result<crate::message::MessageQueue, LoadError> {
    let mut q = crate::message::MessageQueue::new();
    if input.u8()? != 0 {
        let r = decode_record(input)?;
        let timer = input.u32()? as i32;
        q.reopen(r, timer);
    }
    let count = input.u32()? as usize;
    if count > crate::message::RING {
        return Err(bad_count(input, count, "queued messages"));
    }
    for _ in 0..count {
        let r = decode_record(input)?;
        // Straight into the ring: the peer filter already ran when the record
        // was first enqueued, and re-running it against a game loaded by a
        // different local player would silently drop messages the file holds.
        q.restore(r);
    }
    Ok(q)
}

/// The campaign section, read back. Every field is range-checked, because a
/// campaign counter past the table is an out-of-bounds map lookup.
fn decode_campaign(input: &mut Reader<'_>) -> Result<crate::victory::Campaign, LoadError> {
    use crate::victory::{Campaign, Track, CAMPAIGN_LENGTH};
    use l2_kingdom::victory::Outcome;

    let track = match input.u8()? {
        0 => Track::First,
        1 => Track::Second,
        other => return Err(bad_count(input, other as usize, "campaign track")),
    };
    let map = input.u32()? as usize;
    if map > CAMPAIGN_LENGTH {
        return Err(bad_count(input, map, "campaign map"));
    }
    let outcome_byte = input.u8()?;
    let outcome = Outcome::from_value(outcome_byte)
        .ok_or_else(|| bad_count(input, outcome_byte as usize, "outcome"))?;
    let ranking = l2_kingdom::victory::Ranking {
        leader: input.u8()?,
        trailer: input.u8()?,
        opponents_remaining: input.u8()?,
        realms_in_play: input.u8()?,
    };
    Ok(Campaign { track, map, outcome, ranking })
}

fn decode_prefix(input: &mut Reader<'_>, kingdom: Kingdom) -> Result<Game, LoadError> {
    let player = input.u8()?;
    if player as usize >= MAX_REALMS {
        return Err(LoadError::Player(player));
    }
    let map_slot = input.u32()?;
    if map_slot as usize >= MAP_COUNT {
        return Err(LoadError::MapSlot(map_slot));
    }
    let selected = input.u8()?;
    if selected != 0 && selected as usize > kingdom.county_count {
        return Err(LoadError::Selected(selected));
    }
    let turns_played = input.u32()?;

    let realms = input.u32()? as usize;
    if realms != MAX_REALMS {
        return Err(bad_count(input, realms, "realm count"));
    }
    let mut realm_colour = [0u8; MAX_REALMS];
    realm_colour.copy_from_slice(input.raw(MAX_REALMS)?);
    let mut player_names = [crate::text::PlayerName::EMPTY; MAX_REALMS];
    for slot in player_names.iter_mut() {
        let mut bytes = [0u8; crate::text::PLAYER_NAME_LEN];
        bytes.copy_from_slice(input.raw(crate::text::PLAYER_NAME_LEN)?);
        *slot = crate::text::PlayerName::from_bytes(bytes);
    }
    let mut gold_last = [0i32; MAX_REALMS];
    for slot in gold_last.iter_mut() {
        *slot = input.i32()?;
    }

    let counties = input.u32()? as usize;
    if counties != MAX_COUNTIES {
        return Err(bad_count(input, counties, "county count"));
    }
    let mut anchor_x = [0u8; MAX_COUNTIES];
    anchor_x.copy_from_slice(input.raw(MAX_COUNTIES)?);
    let mut anchor_y = [0u8; MAX_COUNTIES];
    anchor_y.copy_from_slice(input.raw(MAX_COUNTIES)?);

    let last_report = match input.option(decode_report)? {
        None => None,
        Some(report) => Some(report?),
    };
    let campaign = decode_campaign(input)?;
    let messages = decode_messages(input)?;

    Ok(Game {
        kingdom,
        messages,
        // `g_multiplayer` — session, not world. A save carries no session.
        multiplayer: false,
        player,
        // **A save is between turns, always.** The original saves from the
        // campaign map and nowhere else, so a loaded game has no half-run turn,
        // no battle waiting to be answered and no half-made levy — the three
        // fields below are session state rather than world state, which is why
        // they are not in the ten-field prefix and why `VERSION` did not have to
        // move.
        //
        // The levy is the clearest case of the three: `g_levyBasket` is scratch
        // that `Army_Create` spends and abandons, and the only durable half of
        // it — the realm's weapon stocks it was seeded from — is in the kingdom
        // already, encoded by `l2_kingdom::save` and hashed into the lockstep
        // digest with everything else.
        field_policy: crate::engagement::Answer::Decline,
        turn: None,
        levy: crate::game::LevyOrder::default(),
        battle: None,
        map_slot: map_slot as usize,
        realm_colour,
        player_names,
        selected,
        anchor_x,
        anchor_y,
        gold_last,
        last_report,
        turns_played,
        campaign,
        // **Deliberately not in the file, and this is the note that says so.**
        // `Prefs` is what this machine is like — sound, animations, scroll
        // speed — and the presentation quirks are what this reader wants to
        // look at. Neither is a property of the *game*: recording them in a
        // save would put one person's preferences into a world another person
        // then loads, and `VERSION` would move every time somebody added a
        // volume control. The behavioural quirks, which really are the world's,
        // are in `l2_kingdom::save` where they belong.
        prefs: crate::game::Prefs::default(),
        presentation_quirks: crate::game::Quirks::default(),
    })
}

fn bad_count(input: &Reader<'_>, found: usize, expected: &'static str) -> LoadError {
    LoadError::Malformed(CodecError::BadTag {
        tag: found.min(255) as u8,
        expected,
        at: input.position(),
    })
}

// ---------------------------------------------------------------------------
// The season report
// ---------------------------------------------------------------------------
//
// This is the only part of the file that is a **tagged union**, and every tag
// is decoded through a total function that can fail: an unknown pass index, an
// unknown message tag, an event id the deck does not have and an unknown
// bankruptcy action are four separate refusals rather than a default.
//
// The tags are written here rather than in `l2-kingdom` on purpose. A wire tag
// is a promise about a *file*, and `l2-kingdom` has no files; giving `Message`
// a tag byte there would put a save-format constant in the crate that is
// supposed to be able to change its enums freely. The cost is that adding a
// variant breaks the two matches below at compile time, which is the behaviour
// we want.

/// `Pass` goes out as its position in `SEASON_PIPELINE` rather than as a tag of
/// its own: the pipeline is the ordering every test already compares against,
/// and `Pass::Industry(Commodity)` is four of its entries, so this needs no
/// second enum for the commodity.
fn encode_report(out: &mut Canonical, report: &SeasonReport) {
    out.seq(&report.passes, |o, pass| o.u32(pass.order() as u32));
    out.seq(&report.messages, encode_message);
    out.bytes(&report.revolts);
}

fn decode_report(input: &mut Reader<'_>) -> Result<Result<SeasonReport, LoadError>, CodecError> {
    // `Reader::seq` hands the element decoder a `CodecError` channel and
    // nothing wider, so the richer refusals — an unknown pass, an unknown
    // event — come back through the *value* as an inner `Result` and are
    // unwrapped here rather than being flattened into a bare "malformed".
    let passes = input.seq(|r| {
        let index = r.u32()?;
        Ok(SEASON_PIPELINE.get(index as usize).copied().ok_or(LoadError::BadPass(index)))
    })?;
    let mut collected: Vec<Pass> = Vec::with_capacity(passes.len());
    for pass in passes {
        match pass {
            Ok(p) => collected.push(p),
            Err(e) => return Ok(Err(e)),
        }
    }
    let messages = input.seq(decode_message)?;
    let mut kept: Vec<Message> = Vec::with_capacity(messages.len());
    for message in messages {
        match message {
            Ok(m) => kept.push(m),
            Err(e) => return Ok(Err(e)),
        }
    }
    let revolts = input.bytes()?.to_vec();
    Ok(Ok(SeasonReport { passes: collected, messages: kept, revolts }))
}

fn encode_message(out: &mut Canonical, message: &Message) {
    match *message {
        Message::UnrestWarning { county } => {
            out.u8(1);
            out.u8(county);
        }
        Message::UnrestRising { county, level } => {
            out.u8(2);
            out.u8(county);
            out.u8(level);
        }
        Message::Revolt { county } => {
            out.u8(3);
            out.u8(county);
        }
        Message::Bankrupt { realm, stage, action } => {
            out.u8(4);
            out.u8(realm);
            out.u8(stage);
            out.u8(bankruptcy_tag(action));
        }
        Message::Event { county, kind } => {
            out.u8(5);
            out.u8(county);
            out.u16(kind.id());
        }
        Message::CastleBuilt { county, castle_type } => {
            out.u8(6);
            out.u8(county);
            out.u8(castle_type);
        }
        Message::ArmyStarving { realm, unit, county, stage } => {
            out.u8(7);
            out.u8(realm);
            out.u32(unit as u32);
            out.u8(county);
            out.i32(stage);
        }
        Message::CountySeceded { realm, county } => {
            out.u8(8);
            out.u8(realm);
            out.u8(county);
        }
        Message::LandsDivide { realm, counties } => {
            out.u8(9);
            out.u8(realm);
            out.u8(counties);
        }
    }
}

fn decode_message(input: &mut Reader<'_>) -> Result<Result<Message, LoadError>, CodecError> {
    let tag = input.u8()?;
    Ok(match tag {
        1 => Ok(Message::UnrestWarning { county: input.u8()? }),
        2 => Ok(Message::UnrestRising { county: input.u8()?, level: input.u8()? }),
        3 => Ok(Message::Revolt { county: input.u8()? }),
        4 => {
            let realm = input.u8()?;
            let stage = input.u8()?;
            let action = input.u8()?;
            match bankruptcy_from_tag(action) {
                Some(action) => Ok(Message::Bankrupt { realm, stage, action }),
                None => Err(LoadError::BadBankruptcy(action)),
            }
        }
        5 => {
            let county = input.u8()?;
            let id = input.u16()?;
            match EventKind::from_id(id) {
                Some(kind) => Ok(Message::Event { county, kind }),
                None => Err(LoadError::BadEvent(id)),
            }
        }
        6 => Ok(Message::CastleBuilt { county: input.u8()?, castle_type: input.u8()? }),
        7 => Ok(Message::ArmyStarving {
            realm: input.u8()?,
            unit: input.u32()? as usize,
            county: input.u8()?,
            stage: input.i32()?,
        }),
        8 => Ok(Message::CountySeceded { realm: input.u8()?, county: input.u8()? }),
        9 => Ok(Message::LandsDivide { realm: input.u8()?, counties: input.u8()? }),
        other => Err(LoadError::BadMessage(other)),
    })
}

/// Exhaustive both ways, so adding a stage to the escalation is a compile
/// error here rather than a save that reads back a different message.
fn bankruptcy_tag(action: BankruptcyAction) -> u8 {
    match action {
        BankruptcyAction::None => 0,
        BankruptcyAction::MercenariesDesert => 1,
        BankruptcyAction::Warned => 2,
        BankruptcyAction::Desertion => 3,
        BankruptcyAction::LastWarning => 4,
        BankruptcyAction::Mutiny => 5,
    }
}

fn bankruptcy_from_tag(tag: u8) -> Option<BankruptcyAction> {
    Some(match tag {
        0 => BankruptcyAction::None,
        1 => BankruptcyAction::MercenariesDesert,
        2 => BankruptcyAction::Warned,
        3 => BankruptcyAction::Desertion,
        4 => BankruptcyAction::LastWarning,
        5 => BankruptcyAction::Mutiny,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a_game() -> Game {
        let mut g = Game::new(0x0F1E_2D3C);
        g.kingdom.set_county_count(4);
        for id in 1..=4u8 {
            g.kingdom.counties[id as usize].owner = if id % 2 == 0 { 2 } else { 1 };
            g.anchor_x[id as usize] = id * 3;
            g.anchor_y[id as usize] = id * 5;
        }
        g.player = 1;
        g.map_slot = 7;
        g.selected = 3;
        g.turns_played = 11;
        g.realm_colour = [0, 1, 2, 3, 4, 5];
        g.gold_last = [0, 1500, -20, 3, 4, 5];
        g
    }

    #[test]
    fn a_game_round_trips_field_for_field() {
        let game = a_game();
        let bytes = encode(&game);
        let back = decode(&bytes, Tables::DEFAULT).expect("our own bytes");
        assert_eq!(back, game);
    }

    #[test]
    fn the_header_reads_without_decoding_the_body() {
        let bytes = encode(&a_game());
        let h = peek(&bytes).expect("a header");
        assert_eq!(h.version, VERSION);
        assert_eq!(HEADER_LEN + h.prefix_len + h.kingdom_len + 8, bytes.len());
    }

    #[test]
    fn a_season_report_round_trips_including_its_tagged_union() {
        let mut game = a_game();
        let mut report = SeasonReport::new();
        report.passes = SEASON_PIPELINE.to_vec();
        report.message(Message::UnrestWarning { county: 1 });
        report.message(Message::UnrestRising { county: 2, level: 3 });
        report.message(Message::Revolt { county: 2 });
        report.message(Message::Bankrupt {
            realm: 1,
            stage: 4,
            action: BankruptcyAction::LastWarning,
        });
        report.message(Message::Event { county: 3, kind: EventKind::Plague });
        report.message(Message::CastleBuilt { county: 4, castle_type: 2 });
        report.message(Message::ArmyStarving { realm: 2, unit: 40, county: 1, stage: 5 });
        game.last_report = Some(report);

        let back = decode(&encode(&game), Tables::DEFAULT).expect("our own bytes");
        assert_eq!(back, game);
        assert_eq!(back.last_report.as_ref().unwrap().revolts, vec![2]);
    }

    #[test]
    fn every_pass_in_the_pipeline_survives_the_round_trip() {
        // The pass index *is* the pipeline position, so a pipeline that gained
        // or lost an entry would silently renumber every save. This pins that
        // the mapping is total and one-to-one in both directions.
        for (i, pass) in SEASON_PIPELINE.iter().enumerate() {
            assert_eq!(pass.order(), i, "{pass:?}");
        }
    }

    #[test]
    fn a_file_that_is_not_ours_is_refused_on_the_magic() {
        assert_eq!(decode(b"not a save at all, really", Tables::DEFAULT), Err(LoadError::NotASave));
        assert_eq!(decode(&[], Tables::DEFAULT), Err(LoadError::NotASave));
        // The original's own save, whose first bytes are anything but ours.
        let mut theirs = vec![0u8; 256];
        theirs[..8].copy_from_slice(b"L2KSAVE\x01");
        assert_eq!(decode(&theirs, Tables::DEFAULT), Err(LoadError::NotASave));
    }

    #[test]
    fn a_version_this_build_does_not_know_is_named_rather_than_read() {
        let mut bytes = encode(&a_game());
        bytes[8..12].copy_from_slice(&(VERSION + 1).to_le_bytes());
        assert_eq!(
            decode(&bytes, Tables::DEFAULT),
            Err(LoadError::UnsupportedVersion { found: VERSION + 1, supported: VERSION })
        );
        let text = LoadError::UnsupportedVersion { found: 99, supported: VERSION }.to_string();
        assert!(text.contains("99"), "{text}");
        assert!(text.contains("Refusing rather than guessing"), "{text}");
    }

    #[test]
    fn a_worlds_version_this_build_does_not_know_comes_back_named_too() {
        let bytes = encode(&a_game());
        let h = peek(&bytes).unwrap();
        // The kingdom blob's own version word: four bytes into its header.
        let at = HEADER_LEN + h.prefix_len + 8;
        let mut bad = bytes.clone();
        let bumped = l2_kingdom::save::VERSION + 1;
        bad[at..at + 4].copy_from_slice(&bumped.to_le_bytes());
        rehash(&mut bad);
        assert_eq!(
            decode(&bad, Tables::DEFAULT),
            Err(LoadError::Kingdom(l2_kingdom::save::LoadError::UnsupportedVersion {
                found: bumped,
                supported: l2_kingdom::save::VERSION,
            })),
            "the world's version is the world's to refuse, and it says so in its own words"
        );
    }

    #[test]
    fn a_truncated_file_is_refused_rather_than_read_short() {
        let bytes = encode(&a_game());
        let short = &bytes[..bytes.len() - 40];
        assert!(matches!(
            decode(short, Tables::DEFAULT),
            Err(LoadError::TruncatedBody { .. }) | Err(LoadError::NotASave)
        ));
    }

    #[test]
    fn one_flipped_byte_anywhere_in_the_body_is_caught_by_the_checksum() {
        let good = encode(&a_game());
        for at in [HEADER_LEN, HEADER_LEN + 3, good.len() - 20] {
            let mut bad = good.clone();
            bad[at] ^= 0xFF;
            assert!(
                matches!(decode(&bad, Tables::DEFAULT), Err(LoadError::Corrupt { .. })),
                "byte {at} flipped and the file still loaded"
            );
        }
    }

    #[test]
    fn a_player_that_is_not_a_realm_is_refused() {
        let mut game = a_game();
        game.player = 9;
        assert_eq!(decode(&encode(&game), Tables::DEFAULT), Err(LoadError::Player(9)));
        // And the guard is on the *file*, not on the struct: a save whose
        // player byte was corrupted in transit is refused the same way.
        let mut bytes = encode(&a_game());
        bytes[HEADER_LEN] = MAX_REALMS as u8;
        rehash(&mut bytes);
        assert_eq!(
            decode(&bytes, Tables::DEFAULT),
            Err(LoadError::Player(MAX_REALMS as u8)),
            "the player byte is the first byte of the prefix"
        );
    }

    #[test]
    fn a_selection_that_is_not_a_county_on_this_map_is_refused() {
        let mut game = a_game();
        game.selected = 12; // four counties on this map
        let bytes = encode(&game);
        assert_eq!(decode(&bytes, Tables::DEFAULT), Err(LoadError::Selected(12)));
    }

    #[test]
    fn a_map_slot_past_the_sixty_is_refused() {
        let mut game = a_game();
        game.map_slot = MAP_COUNT;
        let bytes = encode(&game);
        assert_eq!(decode(&bytes, Tables::DEFAULT), Err(LoadError::MapSlot(MAP_COUNT as u32)));
    }

    #[test]
    fn a_ruleset_the_save_was_not_written_under_is_refused_by_the_kingdom() {
        let bytes = encode(&a_game());
        let mut other = Tables::DEFAULT;
        other.ration_happiness_slope += 1;
        assert!(
            matches!(
                decode(&bytes, other),
                Err(LoadError::Kingdom(l2_kingdom::save::LoadError::RulesetMismatch { .. }))
            ),
            "a mod set that is not the one the save was made with must be named, not applied"
        );
    }

    /// Recompute the trailer after a deliberate poke, so that a test about one
    /// guard is not answered by the checksum instead.
    fn rehash(bytes: &mut [u8]) {
        let n = bytes.len();
        let mut c = Canonical::hashing();
        c.raw(&bytes[HEADER_LEN..n - 8]);
        let hash = c.finish().hash;
        bytes[n - 8..].copy_from_slice(&hash.to_le_bytes());
    }
}
