//! Starting from the England turn-one scenario, rather than from a made-up one.
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
use l2_kingdom::realm::MAX_REALMS;
use l2_scenario::newgame::{MapError, NewGame};
use l2_scenario::{ImportError, Scenario};

use crate::game::Game;

/// The two files a scenario is read from. The executable is the schema and the
/// save is the data; neither is shipped by us.
pub const EXECUTABLE: &str = "Lords2.exe";
pub const SAVE: &str = "lastturn.sav";

/// `g_scenarioIndex` (`0x0053F034`), and it **is the map slot**, 0..=59.
///
/// `docs/formats/maps.md` says its low two bits select the tile set's season
/// variant and the rest selects the slot, and that is wrong — see
/// `docs/screens.md` §3.1. Three readings settle it: `Map_LoadLattice(slot)`
/// seeks `slot * 0x80C1`, which is the whole slot stride; `Gfx_LoadCountyMode`
/// takes the season from `g_season`, a separate global; and
/// `Eng_DrawString(101, g_scenarioIndex, …)` indexes `L2.eng` group 101, whose
/// 60 strings name slots 0..=59 one for one.
const SCENARIO_INDEX: u32 = 0x0053_F034;

/// `g_realms` and its record stride, so the realm colour byte at `+0x0A` can be
/// read without teaching `l2-formats::save::Realm` about presentation.
const REALM_BASE: u32 = l2_formats::save::REALM_BASE;
const REALM_STRIDE: u32 = l2_formats::save::REALM_STRIDE as u32;
const REALM_COLOUR: u32 = 0x0A;

/// The seed the kingdom's generator starts on.
///
/// **Ours, not the original's, and it could not be otherwise.** The original
/// draws from two 31-bit LFSRs (`FUN_00404A46`) whose state is not among the
/// blocks the save writes, and `l2-kingdom` draws from `l2_net::Pcg32`, which
/// is a different generator with a different value stream. So the weather and
/// the event deck will not follow the original's from this save; everything
/// that does not draw a random number will.
pub const SEED: u64 = 0x0001_0D52;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// A file the scenario needs is not in the overlay.
    Missing { name: String, detail: String },
    /// The save would not open, or would not convert.
    Import(ImportError),
    /// The save opened but an address the interface needs is not in a saved
    /// block.
    Save(l2_formats::SaveError),
    /// A map slot that could not be made into a world.
    Map(MapError),
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Error::Missing { name, detail } => write!(f, "{name}: {detail}"),
            Error::Import(e) => write!(f, "{e}"),
            Error::Save(e) => write!(f, "{e}"),
            Error::Map(e) => write!(f, "{e}"),
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

/// Read the England turn-one scenario into a playable [`Game`].
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
    game.map_slot = save.i32_at(SCENARIO_INDEX)?.max(0) as usize;

    // Realm `+0x0A`, the colour byte, stored **raw**.
    //
    // It is not clamped here on purpose. `FUN_004171EE` clamps 0 up to 1 and
    // anything above 5 down to 5 at the point of use, and doing the same thing
    // here would hide a misread: if this offset were wrong the bytes would come
    // back zero and a clamp would quietly turn them into a plausible-looking
    // colour 1 for every realm. Raw, a wrong offset reads as zero and the test
    // that checks the five realms fly five different colours fails.
    for (id, slot) in game.realm_colour.iter_mut().enumerate() {
        let va = REALM_BASE + id as u32 * REALM_STRIDE + REALM_COLOUR;
        *slot = save.u8_at(va).unwrap_or(0);
    }

    for id in scenario.county_ids() {
        if let Some(c) = &scenario.counties[id] {
            game.anchor_x[id] = c.anchor.0;
            game.anchor_y[id] = c.anchor.1;
        }
    }
    for (id, realm) in game.kingdom.realms.iter().enumerate() {
        game.gold_last[id] = realm.gold;
    }

    // **The pair block is carried now, so this no longer runs.**
    //
    // `l2_formats::save::Realm` did not read `+0x84 … +0xE3` at all, so every
    // load re-ran `Diplo_Init` -- right for the England turn-one fixture, where
    // nothing has moved, and wrong for every later save: a player who loaded a
    // mid-game file found the AI had forgotten every war.
    //
    // The fixtures can tell the difference, which is why this is asserted
    // rather than inferred. `Diplo_Init` opens an in-play AI at 5;
    // `siege-lastturn.sav` carries 18, `old_turn.sav` 8 and `battle-after.sav`
    // 10 -- the +1-a-turn heal, thirteen turns of it in the first. A test that
    // could only be run against turn one could not have failed.
    // `docs/decisions.md` C83.

    // Open on a county the player holds, if any. Ascending, so two peers with
    // the same save open on the same county.
    game.selected = game
        .kingdom
        .county_ids()
        .find(|&id| game.kingdom.counties[id].owner == game.player)
        .unwrap_or(0) as u8;

    Ok(game)
}

/// **A new game on a chosen `L2_maps.dat` slot.**
///
/// The other constructor. [`load`] reads a world the original built and this
/// one builds a world, which is what *New Game* has needed since the setup
/// screen learned to choose a map. `l2_scenario::newgame` is the whole of
/// `Map_InitScenario`; what is left here is the application's part, and it is
/// the same three things [`from_save`] does — find the file through the mod
/// overlay, keep `g_scenarioIndex`, and seed the interface's own state.
///
/// # The order is `Game_NewGame`'s
///
/// 1. `Map_InitScenario` and `County_Reset` — [`Scenario::from_map`];
/// 2. `Game_SetupRealmsAndCounties`' option half —
///    [`crate::setup::Settings::apply_to`], which the caller runs next because
///    it is the caller who has the settings;
/// 3. one immediate `Season_Advance` — [`l2_kingdom::Kingdom::start_new_game`],
///    which is why a new game begins in **Winter 1268** and not in the Autumn
///    1267 this function returns.
///
/// Steps 2 and 3 are the caller's on purpose: this returns the world, and the
/// twelve options are not the world.
///
/// # The seed
///
/// `seed` decides one thing — which realm gets which start county, dealt by
/// `FUN_00497E65`. It is a parameter rather than a constant because a network
/// game's seed comes from the lobby and both peers must build the same world
/// from it (`docs/netcode.md`). The single-player path passes [`SEED`], so a
/// new game on a given map is reproducible today; a seed the player can see and
/// change is the lobby's to add.
///
/// # The shield
///
/// `shield` is the colour the person picked on setup page 4, 1 … 5, and it sits
/// beside `seed` and `human_players` on purpose: all three are **lobby** facts
/// rather than option-grid ones. `settings` is the twelve values the custom
/// page committed, and a campaign row overwrites every one of them — but not
/// the colour, because page 4 is the page you pass *through* on the way to
/// pressing anything. So the shield cannot live on [`crate::setup::Settings`]
/// without being wiped by a campaign.
///
/// It changes the world rather than the picture: it moves which realm flies
/// which colour **and which lord sits behind each realm**
/// (`l2_scenario::newgame::assign_lords`, `docs/rules.md` §7a), so two lockstep
/// peers must agree on it before tick 0 the same way they agree on the seed.
/// `docs/netcode.md` D-3a.
pub fn new_game(
    assets: &crate::game::Assets,
    slot: usize,
    settings: &crate::setup::Settings,
    human_players: usize,
    shield: u8,
    seed: u64,
    tables: Tables,
) -> Result<Game, Error> {
    let map = assets.slot(slot).ok_or_else(|| Error::Missing {
        name: "L2_maps.dat".into(),
        detail: format!("has no slot {slot}"),
    })?;
    // `Setup_CommitOptions` keeps `DAT_0053F268 = nobles - humanPlayers`, so the
    // number of realms with a lord is that plus the people.
    let lords = (settings.ai_lords.max(0) as usize + human_players).clamp(1, MAX_REALMS - 1);
    let setup = NewGame {
        slot,
        options: settings.kingdom_options(),
        lords,
        // One person, realm 1. `g_localPlayer` is the lobby's in a network
        // game and there is no lobby.
        local_player: 1,
        shield,
        seed,
    };
    let scenario = Scenario::from_map(&map, &setup).map_err(Error::Map)?;

    let mut game = Game::new(seed);
    game.kingdom = scenario.kingdom_with_tables(seed, tables);
    game.player = scenario.local_player;
    game.map_slot = slot;

    // **`Scenario::kingdom` opens the happiness average on this season's
    // happiness and `County_Reset` opens it on zero.** The difference is one
    // extra sample in the empire-happiness average, and it belongs to the
    // save's constructor rather than to this one: a loaded game is mid-year and
    // a new game is not.
    for id in game.kingdom.county_ids() {
        game.kingdom.counties[id].happiness_sum = 0;
        game.kingdom.counties[id].happiness_avg = 0;
    }

    // **The colour a realm flies is the one `Realms_AssignLords` gave it**, and
    // this used to say it was the realm id: *"`Game_SetupRealms` seeds
    // `shieldIndex = i` and only a custom game's colour picker permutes it,
    // which this build has no screen for."* Two of those three clauses were
    // wrong by the time anybody read them — the picker is setup page 4, this
    // build has had it since the front end was drawn, and the seed is
    // overwritten for every AI on every `Realms_AssignLords`. The player who
    // reported *"I picked a colour and it didn't get honoured once the game
    // opened"* was reporting this line's premise, not this line.
    // `docs/decisions.md` CNEW-shield-colour.
    for (id, slot) in game.realm_colour.iter_mut().enumerate() {
        *slot = game.kingdom.realms.get(id).map(|r| r.shield_index).unwrap_or(0);
    }
    for id in scenario.county_ids() {
        if let Some(c) = &scenario.counties[id] {
            game.anchor_x[id] = c.anchor.0;
            game.anchor_y[id] = c.anchor.1;
        }
    }
    for (id, realm) in game.kingdom.realms.iter().enumerate() {
        game.gold_last[id] = realm.gold;
    }
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
