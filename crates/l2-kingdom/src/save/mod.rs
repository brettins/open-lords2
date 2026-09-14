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
//!   walked by ascending index.
//! * **Determinism is a property of the encoder**, not of a promise made here.
//!
//! # Versioned, and it refuses
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

mod codec_part;
pub use codec_part::*;

mod codec;
pub use codec::*;
mod domain;
pub use domain::*;

use crate::county::{ChangeReason, County, Industry, MAX_COUNTIES, MAX_INFLOW_SOURCES, MAX_NEIGHBOURS};
use crate::kingdom::{History, HistoryEntry, Kingdom, Options};
use crate::phase::{Phase, TurnMachine};
use crate::realm::{Realm, MAX_REALMS};
use crate::tables::{
    Tables, Weather, HISTORY_COUNTIES, HISTORY_SEASONS, JOB_COUNT, WEAPON_TYPE_COUNT,
};
use l2_net::canonical::{Canonical, CodecError, Decode, Encode, Reader};

