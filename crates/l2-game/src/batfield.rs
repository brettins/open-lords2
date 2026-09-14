//! **Where `batfield.pl8` enters the program** — the open field's twin of
//! [`crate::castle`], and for the same reason: `Battlefield_BuildRandom`
//! (`0x0047AAA3`) opens the file itself, in the middle of raising a battle,
//! out of a process-global working directory, and no caller hands it a path or
//! the bytes.
//!
//! ```c
//! Restore_WorkingDir();
//! File_ReadChunk(s_batfield_pl8, buf, 1000, 0);
//! File_ReadChunk(s_batfield_pl8, buf, 0x1900, u24_at(g_battlefieldMap * 0x10 + 0x0C));
//! ```
//!
//! A checkout with no install leaves the slot empty and [`field`] falls back
//! to [`l2_sim::runner::blank_field`] — the editor's empty template, which is
//! what every field battle used before this module existed.

use std::sync::OnceLock;

use l2_sim::terrain::{build_field, FieldSheets};
use l2_sim::Battlefield;

static SHEETS: OnceLock<FieldSheets> = OnceLock::new();

/// Publish the open fields. First call wins, like [`crate::castle::publish`]:
/// two different `batfield.pl8` files in one process would mean two different
/// worlds for one map index.
pub fn publish(sheets: FieldSheets) -> bool {
    SHEETS.set(sheets).is_ok()
}

/// Parse and publish, if the overlay has the file. Returns how many fields
/// were found — 0 on a checkout without the install.
pub fn publish_from(read: impl FnOnce(&str) -> Option<Vec<u8>>) -> usize {
    let Some(bytes) = read(FieldSheets::FILE) else { return 0 };
    let Some(sheets) = FieldSheets::parse(&bytes) else { return 0 };
    let n = sheets.len();
    if publish(sheets) {
        n
    } else {
        SHEETS.get().map(FieldSheets::len).unwrap_or(0)
    }
}

/// Whether the fields are loaded at all.
pub fn loaded() -> bool {
    SHEETS.get().is_some()
}

/// **The battlefield a field battle is fought on.**
///
/// The original takes the map from a 48-entry playlist at `0x0057CAE0` that a
/// cursor walks one field per battle; we take it from the battle seed, so it
/// is a pure function of state the digest already carries
/// (`docs/netcode.md`). Everything downstream of the raster is the original's.
pub fn field(seed: u64) -> Battlefield {
    match SHEETS.get() {
        Some(sheets) if !sheets.is_empty() => build_field(sheets.pick(seed), seed as u32),
        _ => l2_sim::runner::blank_field(),
    }
}
