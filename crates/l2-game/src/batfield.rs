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

/// `batfiel2.pl8` — entry **2** of the builder's filename table at
/// `0x004D9103` (14 bytes a name, the name at `+5`), which
/// `Battlefield_BuildRandom` takes whenever `DAT_0057A0F0` is up.
static SKIRMISH_SHEETS: OnceLock<FieldSheets> = OnceLock::new();

pub fn publish(sheets: FieldSheets) -> bool {
    SHEETS.set(sheets).is_ok()
}

pub fn publish_from(read: impl Fn(&str) -> Option<Vec<u8>>) -> usize {
    if let Some(s) = read(FieldSheets::SKIRMISH_FILE).as_deref().and_then(FieldSheets::parse) {
        let _ = SKIRMISH_SHEETS.set(s);
    }
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

/// **No playlist and no draw** — `Battlefield_BuildRandom`'s `DAT_0057A0F0`
/// branch (`00470000.c:4487-4493`): `DAT_00553528 = DAT_0056D590` for category
/// 0 and `+ 10` for category 1, out of `batfiel2.pl8` for both. The row *is*
/// the field, so two skirmishes of the same row are the same ground.
///
/// The seed still colours it: `graphics_pass`'s LFSR is
/// `FUN_00404B2C`'s, which the original leaves running across battles.
pub fn skirmish_field(kind: usize, row: usize, seed: u64) -> Battlefield {
    let frame = row + if kind == 1 { 10 } else { 0 };
    match SKIRMISH_SHEETS.get().and_then(|s| s.get(frame)) {
        Some(raster) => build_field(raster, seed as u32),
        None => field(seed),
    }
}

/// The campaign's own random field, and the fallback when `batfiel2.pl8` is
/// not installed.
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
/// [D] The frame is taken modulo the sheet count; `Battlefield_BuildRandom` (`0x0047AAA3`,
/// `00470000.c:4501`) indexes the directory with the byte unbounded. Inert on the shipped 48.
pub fn field_frame(frame: u8, seed: u64) -> Battlefield {
    match SHEETS.get() {
        Some(sheets) if !sheets.is_empty() => {
            build_field(sheets.pick(frame as u64), seed as u32)
        }
        _ => l2_sim::runner::blank_field(),
    }
}
