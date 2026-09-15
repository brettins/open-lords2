
use l2_formats::maps::{MapSlot, PLANE_DIM};
use l2_formats::save::Save;
use l2_kingdom::tables::Tables;
use l2_mods::vfs::Vfs;
use l2_kingdom::realm::MAX_REALMS;
use l2_scenario::newgame::{MapError, NewGame};
use l2_scenario::{ImportError, Scenario};

use crate::game::Game;

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

/// The two crates read the same `0x1F` out of the same two `FUN_00401136`
/// calls, and [`from_save`] moves one array into the other by value. If they
/// ever disagree this fails to compile.
const _: () = assert!(l2_formats::save::PLAYER_NAME_LEN == crate::text::PLAYER_NAME_LEN);

/// **Ours, not the original's, and it could not be otherwise.** The original
/// draws from two 31-bit LFSRs (`FUN_00404A46`) whose state is not among the
/// blocks the save writes, and `l2-kingdom` draws from `l2_net::Pcg32`, which
/// is a different generator with a different value stream. So the weather and
/// the event deck will not follow the original's from this save; everything
/// that does not draw a random number will.
pub const SEED: u64 = 0x0001_0D52;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Missing { name: String, detail: String },
    Import(ImportError),
    Save(l2_formats::SaveError),
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

    // `g_saveBlocks[2] = {0x00553D50, 264}` is the six-slot player table and
    // the name is each slot's `+0x04` (`l2_formats::save::Player`, which also
    // says what the four bytes before it are). Nothing here read it, so a
    // loaded game had `Game::player_names` empty, and every screen that draws a
    // lord fell through `screens::message::lord_name` to `L2.eng` group 7 and
    // then to `REALM n`. `Realms_AssignLords` (`0x0049CAAA`) and
    // `Player_SetHuman` (`0x0049BAE9`) are what filled it before the save was
    // written; this is the reading half.
    //
    // **Nothing moves in the lockstep digest** — the digest is
    // `Canonical::hash_of(kingdom)` and this is on `Game`, beside
    // `realm_colour`, for exactly that reason. `docs/decisions.md` C198.
    for (id, slot) in game.player_names.iter_mut().enumerate() {
        if let Ok(p) = save.player(id) {
            *slot = crate::text::PlayerName::from_bytes(p.name);
        }
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

    // `l2_formats::save::Realm` did not read `+0x84 … +0xE3` at all, so every
    // load re-ran `Diplo_Init` -- right for the England turn-one fixture, where
    // nothing has moved, and wrong for every later save: a player who loaded a
    // mid-game file found the AI had forgotten every war.
    //
    // `docs/decisions.md` C83.

    game.selected = game
        .kingdom
        .county_ids()
        .find(|&id| game.kingdom.counties[id].owner == game.player)
        .unwrap_or(0) as u8;

    Ok(game)
}

/// `seed` decides one thing — which realm gets which start county, dealt by
/// `FUN_00497E65`. It is a parameter because a network
/// game's seed comes from the lobby and both peers must build the same world
/// from it (`docs/netcode.md`). The single-player path passes [`SEED`], so a
/// new game on a given map is reproducible today; a seed the player can see and
/// change is the lobby's to add.
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
        local_player: 1,
        shield,
        seed,
    };
    let scenario = Scenario::from_map(&map, &setup).map_err(Error::Map)?;

    let mut game = Game::new(seed);
    game.kingdom = scenario.kingdom_with_tables(seed, tables);
    game.player = scenario.local_player;
    game.map_slot = slot;

    // `docs/decisions.md` C161.

    // `docs/decisions.md` C130.
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
