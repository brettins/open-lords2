//! **Where `stnfield.pl8` enters the program.**
//!
//! `Battlefield_BuildCastle` (`0x0047C4BA`) opens the layout file itself, in
//! the middle of raising a battle, out of a process-global working directory:
//!
//! ```c
//! Restore_WorkingDir();
//! File_ReadChunk(s_Q_Q_Qbatfield_pl8 + index * 0x0E + 5, DAT_004EABEC, 1000, 0);
//! ```
//!
//! No caller hands it a path and no caller hands it the bytes. This module is
//! that: one process-wide slot, filled by [`crate::game::Assets::load`] from
//! the mod overlay, and read by [`crate::engagement::begin_fight`] — which,
//! like the original's builder, is reached through six signatures that carry a
//! `Kingdom` and nothing else.
//!
//! **A checkout with no install leaves it empty**, and a siege then falls back
//! to [`l2_sim::siege::our_castle`], the stand-in ring. That is the same
//! degradation every other asset takes, and the shape of the castle is the one
//! thing about a siege that a missing file can change.

use std::sync::OnceLock;

use l2_sim::CastleSheets;

static SHEETS: OnceLock<CastleSheets> = OnceLock::new();

/// Publish the campaign layouts. First call wins; later calls are ignored and
/// reported, because two different `stnfield.pl8` files in one process would
/// mean two different castles for one `g_castleLevel`.
pub fn publish(sheets: CastleSheets) -> bool {
    SHEETS.set(sheets).is_ok()
}

/// Parse and publish, if the overlay has the file. Returns how many castles
/// were found — 0 when there is no install, which is not an error.
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

/// One castle's two layers, or `None` on a checkout without the install.
pub fn sheet(level: u8) -> Option<&'static l2_sim::CastleSheet> {
    SHEETS.get()?.get(level)
}

/// Whether the layouts are loaded at all.
pub fn loaded() -> bool {
    SHEETS.get().is_some()
}
