//! `Battlefield_BuildCastle` (`0x0047C4BA`) opens the layout file itself, in
//! the middle of raising a battle, out of a process-global working directory:
//!
//! ```c
//! Restore_WorkingDir();
//! File_ReadChunk(s_Q_Q_Qbatfield_pl8 + index * 0x0E + 5, DAT_004EABEC, 1000, 0);
//! ```

use std::sync::OnceLock;

use l2_sim::CastleSheets;

static SHEETS: OnceLock<CastleSheets> = OnceLock::new();

pub fn publish(sheets: CastleSheets) -> bool {
    SHEETS.set(sheets).is_ok()
}

pub fn publish_from(read: impl FnOnce(&str) -> Option<Vec<u8>>) -> usize {
    let Some(bytes) = read(CastleSheets::FILE) else { return 0 };
    let Some(sheets) = CastleSheets::parse(&bytes) else { return 0 };
    let n = sheets.len();
    if publish(sheets) {
        n
    } else {
        SHEETS.get().map(CastleSheets::len).unwrap_or(0)
    }
}

pub fn sheet(level: u8) -> Option<&'static l2_sim::CastleSheet> {
    SHEETS.get()?.get(level)
}

pub fn loaded() -> bool {
    SHEETS.get().is_some()
}
