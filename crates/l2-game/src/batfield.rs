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

use std::sync::OnceLock;

use l2_sim::terrain::{build_field, FieldSheets};
use l2_sim::Battlefield;

static SHEETS: OnceLock<FieldSheets> = OnceLock::new();

pub fn publish(sheets: FieldSheets) -> bool {
    SHEETS.set(sheets).is_ok()
}

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

pub fn loaded() -> bool {
    SHEETS.get().is_some()
}

/// **No playlist.** `Battlefield_BuildRandom` (`0x0047AAA3`) takes this branch
/// when `DAT_0057A0F0 != 0` — the skirmish flag — and reads `DAT_0056D590`
/// (+10 for `batfiel2.pl8`) instead of walking `DAT_0057CAE0`. Ours picks from
/// the seed; that choice is the skirmish screen's and is untouched by the
/// playlist.
pub fn field(seed: u64) -> Battlefield {
    match SHEETS.get() {
        Some(sheets) if !sheets.is_empty() => build_field(sheets.pick(seed), seed as u32),
        _ => l2_sim::runner::blank_field(),
    }
}

/// **The campaign branch**, `DAT_00553528 = (&DAT_0057cae0)[DAT_005653F8]` in
/// `Battlefield_BuildRandom` (`0x0047AAA3`): the frame is the playlist entry
/// the caller already walked to, not a draw.
/// [`l2_kingdom::field_playlist`] holds the walk.
///
/// The frame is taken modulo the sheet count, because the playlist is dealt
/// over a fixed 48 and an install's `batfield.pl8` is what it is.
pub fn field_frame(frame: u8, seed: u64) -> Battlefield {
    match SHEETS.get() {
        Some(sheets) if !sheets.is_empty() => {
            build_field(sheets.pick(frame as u64), seed as u32)
        }
        _ => l2_sim::runner::blank_field(),
    }
}
