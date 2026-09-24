//! `Season_Advance` (`0x00448440`) calls 29 functions in a fixed order, and
//! `docs/kingdom.md` §3.4 is explicit that **the order is the rule**. So the
//! driver here is a loop over an array:

mod history;
mod pipeline;
mod economy;
mod state;
mod tests;

use crate::county::{County, MAX_COUNTIES};
use crate::phase::TurnMachine;
use crate::realm::{Realm, MAX_REALMS};
use crate::tables::Tables;
use l2_net::Pcg32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    pub difficulty: u8,
    pub advanced_farming: bool,
    pub armies_eat: bool,
    /// `g_optFightHumansOnly` (`0x0053F284`) **as the option byte**, which the
    /// original stores *inverted*: it is 0 when the option displays *Yes*. Kept
/// as the byte so that the sense cannot drift from
    /// the binary's — [`crate::battle::settlement`] tests it against 0 exactly
    /// as `FUN_004A6A30` does. [`crate::battle::FIGHT_HUMANS_ONLY_DEFAULT`] is
    /// the game's default.
    pub fight_humans_only_byte: u8,
    /// `g_optExploration` (`0x0053F264`), the *Exploration* drop-down — **the
    /// fog of war**.
    ///
    /// **Read by no rule in this crate, and that is the original's shape rather
    /// than a gap.** `L2.eng` group 218 index 3 says what it does: *"When
    /// Exploration is turned on, the world outside your county is blacked out.
    pub exploration: bool,
    /// `g_optTimeLimit` (`0x0053F26C`) — **seconds**, 0 for no limit.
    ///
    /// Not the *Time limit* drop-down's index: `Setup_CommitOptions`
    /// (`0x00499DC3`) puts the index through `g_timeLimitSeconds`
    /// (`0x004DBBF8`) = `30, 60, 120, 240, 480, 600, 0`, so what a game runs on
    /// is a duration and the seven strings are its labels. Carried for the same
    /// reason as [`Options::exploration`]:
    pub time_limit: i32,
    /// There is **no `g_optQuirks`**; the original has no such setting and
    /// could not, because to it these are not settings at all. This field is a
/// declared divergence, and it is here on [`Options`]
    /// [`Tables`] for the reason `docs/bugs.md` §6.3 works out and
    /// `docs/decisions.md` C62 records:
    pub quirks: l2_net::Quirks,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            difficulty: 0,
            advanced_farming: false,
            armies_eat: false,
            fight_humans_only_byte: crate::battle::FIGHT_HUMANS_ONLY_DEFAULT,
            exploration: false,
            time_limit: 0,
            quirks: l2_net::Quirks::FAITHFUL,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kingdom {
    pub counties: [County; MAX_COUNTIES],
    pub realms: [Realm; MAX_REALMS],
    pub county_count: usize,
    pub season: u8,
    pub season_next: u8,
    pub season_prev: u8,
    pub year: i32,
    pub year_next: i32,
    pub turn_count: u32,
    pub turn: TurnMachine,
    pub options: Options,
    pub rng: Pcg32,
    pub history: History,
    /// `g_weatherCounty` (`0x00554020`) — the county that got last season's
    /// local weather swing, which is the fallback when this season's masked
    /// draw lands outside the map. See [`weather::chosen_county`].
    pub weather_county: usize,
    pub campaign: Campaign,
    pub diplomacy: crate::diplomacy::Diplomacy,
    pub tables: Tables,
}

/// `FUN_004AE7DD` — the history ring `docs/kingdom.md` §3.4 names and nothing
/// more.
///
/// **400 seasons x 16 counties x `{population, happiness}`.** The length is not
/// inferred: save block 10 is `0x0056D8C0` for 51,200 bytes and
/// `400 * 16 * 8` is exactly 51,200. Three globals go with it — the write head
/// (`0x0055300C`), the oldest entry (`0x00568DA8`) and the number of entries
/// held (`0x00553F30`, saturating at 400) — and each is its own four-byte save
/// block, so the whole structure is confirmed by the save layout
/// only by the code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct History {
    pub(crate) entries: Vec<[HistoryEntry; crate::tables::HISTORY_COUNTIES]>,
    pub(crate) head: usize,
    pub(crate) tail: usize,
    pub(crate) len: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HistoryEntry {
    pub population: i32,
    pub happiness: i8,
}

impl Default for History {
    fn default() -> Self {
        History::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Campaign {
    pub units: crate::unit::Units,
    pub map: crate::map::CampaignMap,
    pub mercenaries: crate::mercenary::MercenaryBands,
    /// Realm `+0x2D` — twenty-four name counters a lord.
    pub names: crate::unit::ArmyNames,
    pub routes: crate::merchant::MerchantRoutes,
    /// `DAT_004E59DC` — the peasant mobs' **shared** destination cursor.
    ///
    /// One counter for the whole map, not one per mob: `FUN_004AC5BA` bumps it
    /// and hands the result to whichever mob asked. Simulation state, and a
    /// small one that would be easy to leave out of a save and never notice
    /// until two lockstep peers sent their mobs to different counties.
    pub mob_cursor: usize,
    /// `DAT_0057CAE0` and `DAT_005653F8` — the dealt order of `batfield.pl8`
    /// frames and the cursor an open-field battle walks it by. Same reason as
    /// `mob_cursor`: one shared counter for the whole map, and the original
    /// net-syncs both halves at every battle start
    /// (`FUN_00444A2F`). [`crate::field_playlist`].
    pub field_playlist: crate::field_playlist::FieldPlaylist,
    pub explored: crate::explore::Explored,
    /// `g_battleApproachLane` (`0x0056D6B0`) and `g_battleRallyGroup`, as the
    /// last `Battle_Start` (`0x004778A0`, `00470000.c:3104-3118`) left them;
    /// its multiplayer arm steps them from here. `.bss`, so 0 at start.
    pub battle_selectors: (u8, u8),
    /// `DAT_00544050`/`DAT_00544070` and `g_rallyWaypoints`/`DAT_00522D60` as the last open
    /// field left them; a castle field has no writer and reads these. `.bss`, so 0 at start.
    /// [D] saved and hashed, which the original's save is not: a loaded or lockstep game reads them.
    pub battle_markers: BattleMarkers,
    /// `g_battleCounty`, `.bss`: FUN_004A6C68 0x004A6C68 (004a0000.c:2865), Battle_BeginFromCampaign (:2973),
    /// Siege_LaunchAssault (:3621) and 00430000.c:4173 write it; the skirmish path does not [V].
    /// [D] not saved or hashed: the original's save does not hold it [I]; a load keeps the process value.
    pub battle_county: u8,
    /// `g_moveCost` (`0x004F4080`), `.bss`: only Move_BuildCostMap 0x0046FF43 writes it (Game_NewGame,
    /// Turn_Tick phase 7, Unit_OrderMove, Map_BeginMoveSelection). In no Save_Write block, so a load
    /// keeps the previous game's map; the first game in a process starts all-zero [V]. Not saved or
    /// hashed; the load screen carries it across [D]. B108.
    pub move_cost: crate::map::CostMap,
}

/// `[side][slot]` enemy ends, `[side][group][index]` rally waypoints: `l2_sim::terrain::MarkerTables`.
pub type BattleMarkers = ([[(u8, u8); 4]; 2], [[[(u8, u8); 3]; 2]; 2]);

impl Default for Campaign {
    fn default() -> Self {
        Campaign::new()
    }
}

impl Campaign {
    pub fn new() -> Campaign {
        Campaign {
            units: crate::unit::Units::new(),
            map: crate::map::CampaignMap::empty(),
            mercenaries: crate::mercenary::MercenaryBands::none(),
            names: crate::unit::ArmyNames::new(),
            routes: crate::merchant::MerchantRoutes::none(),
            mob_cursor: 0,
            // Zeros until a new game deals it: `Game_NewGame` (`0x00497CED`)
            // reaches it through `PlayerStart_Shuffle`, so
            // `Kingdom::with_tables` is where the deal happens.
            field_playlist: crate::field_playlist::FieldPlaylist::empty(),
            // `Map_InitScenario` clears every seen bit (`FUN_0046DF51`) straight
            // after the planes are loaded.
            explored: crate::explore::Explored::new(),
            battle_selectors: (0, 0),
            battle_markers: Default::default(),
            battle_county: 0,
            move_cost: crate::map::CostMap::uniform(0),
        }
    }

    /// `Move_BuildCostMap` (`0x0046FF43`).
    pub fn rebuild_move_cost(&mut self) {
        self.move_cost = self.map.cost_map();
    }
}

