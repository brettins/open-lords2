//! Starting from the shipped scenario, rather than from a made-up one.
//!
//! `lastturn.sav` in a real install is a turn-1 autosave of the England map,
//! and `l2_formats::save` reads it by walking the block table out of the user's
//! own `Lords2.exe` (the save is a memory dump; the executable is its schema).
//!
//! **The conversion itself is `l2-scenario`'s**, not this module's. That crate
//! is the seam between a file format and a simulation that may not know about
//! each other, and duplicating it here would have meant two importers that
//! could disagree. What is left in this module is the part that is genuinely
//! the application's:
//!
//! * finding the two files through the mod overlay rather than on disk;
//! * `g_scenarioIndex`, which picks the **map slot** — a scenario for the
//!   kingdom does not need it and a screen that draws the map does;
//! * seeding the interface's own state: each county's anchor tile, the
//!   treasury the turn started from, and which county the game opens on.
//!
//! # Why it is worth the trouble
//!
//! `docs/plan.md`'s review found that `l2-kingdom`'s reproduction test built
//! the *scenario* from a document — four counties owned by one realm — while
//! the file holds **five owned counties, one for each of realms 1 to 5, and
//! nine unowned**, with the human realm owning county 8 alone. Reading the file
//! means the interface cannot inherit that fiction, and `tests/scenario.rs`
//! asserts what the bytes say rather than what any document says.

use l2_formats::maps::{MapSlot, PLANE_DIM};
use l2_formats::save::Save;
use l2_kingdom::tables::Tables;
use l2_mods::vfs::Vfs;
use l2_scenario::{ImportError, Scenario};

use crate::game::Game;

/// The two files a scenario is read from. The executable is the schema and the
/// save is the data; neither is shipped by us.
pub const EXECUTABLE: &str = "Lords2.exe";
pub const SAVE: &str = "lastturn.sav";

/// `g_scenarioIndex` (`0x0053F034`, **verified** in `docs/symbols.md`): *"its
/// low 2 bits select the season variant of the tile set; the rest selects the
/// map slot"*.
const SCENARIO_INDEX: u32 = 0x0053_F034;

/// The seed the kingdom's generator starts on.
///
/// **Ours, not the original's, and it could not be otherwise.** The original
/// draws from two 31-bit LFSRs (`FUN_00404A46`) whose state is not among the
/// blocks the save writes, and `l2-kingdom` draws from `l2_net::Pcg32`, which
/// is a different generator with a different value stream. So the weather and
/// the event deck will not follow the original's from this save; everything
/// that does not draw a random number will.
pub const SEED: u64 = 0x10D_52;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// A file the scenario needs is not in the overlay.
    Missing { name: String, detail: String },
    /// The save would not open, or would not convert.
    Import(ImportError),
    /// The save opened but an address the interface needs is not in a saved
    /// block.
    Save(l2_formats::SaveError),
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Error::Missing { name, detail } => write!(f, "{name}: {detail}"),
            Error::Import(e) => write!(f, "{e}"),
            Error::Save(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<ImportError> for Error {
    fn from(e: ImportError) -> Error {
        Error::Import(e)
    }
}

impl From<l2_formats::SaveError> for Error {
    fn from(e: l2_formats::SaveError) -> Error {
        Error::Save(e)
    }
}

/// Read the shipped scenario into a playable [`Game`].
///
/// `tables` is the ruleset the kingdom will run on for the rest of its life —
/// `Tables::DEFAULT`, or whatever `l2-mods` built from the core ruleset and any
/// enabled mod.
pub fn load(vfs: &Vfs, tables: Tables) -> Result<Game, Error> {
    let exe = vfs
        .read(EXECUTABLE)
        .map_err(|e| Error::Missing { name: EXECUTABLE.into(), detail: e.to_string() })?;
    let bytes = vfs
        .read(SAVE)
        .map_err(|e| Error::Missing { name: SAVE.into(), detail: e.to_string() })?;
    let save = Save::open(&exe, &bytes)?;
    from_save(&save, tables)
}

/// The same, from an already-opened save.
pub fn from_save(save: &Save, tables: Tables) -> Result<Game, Error> {
    let scenario = Scenario::from_save(save)?;
    let mut game = Game::new(SEED);
    game.kingdom = scenario.kingdom_with_tables(SEED, tables);
    game.player = scenario.local_player;
    game.map_slot = (save.i32_at(SCENARIO_INDEX)? >> 2).max(0) as usize;

    for id in scenario.county_ids() {
        if let Some(c) = &scenario.counties[id] {
            game.anchor_x[id] = c.anchor.0;
            game.anchor_y[id] = c.anchor.1;
        }
    }
    for (id, realm) in game.kingdom.realms.iter().enumerate() {
        game.gold_last[id] = realm.gold;
    }

    // Open on a county the player holds, if any. Ascending, so two peers with
    // the same save open on the same county.
    game.selected = game
        .kingdom
        .county_ids()
        .find(|&id| game.kingdom.counties[id].owner == game.player)
        .unwrap_or(0) as u8;

    Ok(game)
}

/// County adjacency **derived from the map's county plane**: two counties are
/// neighbours when a tile of one is 4-adjacent to a tile of the other.
///
/// This is not how a game is loaded — the save stores the adjacency list and
/// `l2-scenario` reads it. It exists as an **independent check** on that
/// reading: `L2_maps.dat` and `lastturn.sav` were authored separately, and over
/// the England map the derivation reproduces the stored list for all fourteen
/// counties, ids and all. `tests/scenario.rs` asserts it.
///
/// Returns one ascending list per county id, index 0 unused.
pub fn adjacency_from_map(map: &MapSlot<'_>, county_count: usize) -> Vec<Vec<u8>> {
    let n = county_count.min(16);
    let mut adjacent = vec![[false; 17]; 17];
    for y in 0..PLANE_DIM {
        for x in 0..PLANE_DIM {
            let c = map.county_at(x, y) as usize;
            if c < 1 || c > n {
                continue;
            }
            let mut consider = |nx: usize, ny: usize| {
                let o = map.county_at(nx, ny) as usize;
                if o >= 1 && o <= n && o != c {
                    adjacent[c][o] = true;
                }
            };
            if x + 1 < PLANE_DIM {
                consider(x + 1, y);
            }
            if x > 0 {
                consider(x - 1, y);
            }
            if y + 1 < PLANE_DIM {
                consider(x, y + 1);
            }
            if y > 0 {
                consider(x, y - 1);
            }
        }
    }
    (0..=16)
        .map(|id| {
            if id < 1 || id > n {
                return Vec::new();
            }
            (1..=n as u8).filter(|&o| adjacent[id][o as usize]).collect()
        })
        .collect()
}
