//! The engine's own ruleset — the bottom layer, shipped in the binary.
//!
//! `docs/modding.md` §1 takes OpenXcom's lesson as the design: **the base game
//! is itself the first mod**, and nothing in the engine can tell whether a
//! value came from the original game or from someone's rebalance, because
//! there is no other path for a value to arrive by. These documents are that
//! bottom layer.
//!
//! # Why these are shipped and `troops.toml` is generated
//!
//! `crate::seed` generates `rules/troops.toml` on the player's machine,
//! because it is a transcription of `TROOPS*.ENG` and `CLAUDE.md` rule 1 keeps
//! game data out of this repository. The documents here are the opposite case:
//! they are *our own* numbers, read out of `Lords2.exe` by `docs/battle.md`
//! and `docs/kingdom.md` and already living in this repository as Rust
//! constants. Writing the same numbers as `.toml` adds nothing that was not
//! already committed.
//!
//! That line — generated where the numbers are the player's, shipped where
//! they are ours — is worth stating because it is the only rule that decides
//! where a future ruleset belongs.
//!
//! # Why compiled in rather than read from disk
//!
//! A rules directory that can go missing is a rules directory that can go
//! missing *on one peer only*, and two lockstep peers running different rules
//! is precisely the failure `docs/netcode.md` exists to prevent. Compiled in,
//! the core rules are as present as the code. [`write_to`] still drops a
//! readable copy next to the player's mods, because the first thing a
//! would-be mod author should be able to do is open the base rules and read
//! them.
//!
//! # Keeping them honest
//!
//! Both documents are *rendered* from the tables they describe, and
//! `tests/core.rs` asserts the shipped text is exactly what the renderer
//! produces and that loading it reproduces the table it came from. So the file
//! cannot drift from the code and the code cannot drift from the file, and
//! neither can be edited alone.

use std::io;
use std::path::Path;

/// The battle-simulation combat constants.
pub const UNITS_TOML: &str = include_str!("../rulesets/core/rules/units.toml");

/// The kingdom economy.
pub const KINGDOM_TOML: &str = include_str!("../rulesets/core/rules/kingdom.toml");

/// The layer id the core documents are attributed to in diagnostics.
pub const CORE_LAYER: &str = "core";

/// Every core document, as `(source name, text)`, in the order they apply.
///
/// The order is fixed here rather than derived from a directory listing, so it
/// cannot depend on how a filesystem chooses to enumerate. The two documents
/// share no keys, so the order changes nothing today — which is exactly when
/// to pin it, rather than after something depends on it.
pub const DOCUMENTS: [(&str, &str); 2] =
    [("core:rules/kingdom.toml", KINGDOM_TOML), ("core:rules/units.toml", UNITS_TOML)];

/// Write a readable copy of the core ruleset into `dir/rules/`.
///
/// For the player, not for the engine: the engine already has these compiled
/// in. This is so the base rules sit on disk next to the mods, which is how
/// people learn to mod OpenXcom and how they will learn to mod this.
///
/// Writes only under `dir`, which must not be the game install — nothing here
/// takes a path from [`crate::Vfs`], which has no write API at all.
pub fn write_to(dir: &Path) -> io::Result<Vec<std::path::PathBuf>> {
    let rules = dir.join("rules");
    std::fs::create_dir_all(&rules)?;
    let mut written = Vec::new();
    for (source, text) in DOCUMENTS {
        let name = source.rsplit('/').next().unwrap_or(source);
        let path = rules.join(name);
        std::fs::write(&path, text)?;
        written.push(path);
    }
    Ok(written)
}

/// Render every core document from the tables it describes.
///
/// The regeneration path: `L2_MODS_REGENERATE=1 cargo test -p l2-mods` writes
/// these over the shipped files, and the same test without the variable
/// asserts they are already identical.
pub fn render() -> [(&'static str, String); 2] {
    [
        ("kingdom.toml", crate::kingdom::render_toml(&l2_kingdom::tables::Tables::DEFAULT)),
        ("units.toml", crate::units::render_toml(&l2_sim::TroopTable::DEFAULT)),
    ]
}

/// Where the shipped documents live in the source tree.
pub fn source_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("rulesets").join("core").join("rules")
}
