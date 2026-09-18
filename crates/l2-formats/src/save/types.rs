#![allow(unused_imports)]
use super::*;



/// One realm's view of one other realm — realm `+0x84 + other * 0x10`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DiploPair {
    /// `+0x00` — the standing, `Diplo_Init`'s 5 for an in-play AI and 0 for a
    /// human, moved by the +1-a-turn heal and by `Diplo_Offend`.
    pub standing: i8,
    /// `+0x01` — set and cleared in pairs.
    pub allied: bool,
    /// `+0x02` — accumulates while allied.
    pub grudge: u8,
    /// `+0x03` — the warning ladder, 0 … 3.
    pub warnings_sent: u8,
    /// `+0x04` — at war.
    pub at_war: bool,
    /// `+0x05` — how many compliments the other realm has sent this one.
    pub compliments_from: u8,
    /// `+0x08` — the largest single gift ever received from them.
    pub best_gift: i32,
    /// `+0x0C` — a letter from them is waiting in this realm's inbox.
    pub has_mail: bool,
    /// `+0x0D` — starts at 1 and rises each time this realm is paid to help
    /// them, so the price of help doubles, trebles, quadruples.
    pub help_price_multiple: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Realm {
    pub index: usize,
    pub ai_step: i32,
    /// `+0x04` — a weighted strength count, not the `inPlay` flag
    /// `docs/kingdom.md` §2 called it. Every reader tests it against zero.
    pub strength: u8,
    pub is_human: bool,
    pub lord: u8,
    /// `+0x0A` — which shield and flag colour this realm flies.
    ///
    /// `Game_SetupRealmsAndCounties` initialises it to the realm id,
    /// game has `shield_index == index`; a custom game's colour picker
    /// (`0x0049CE1F`, a free-slot pool) permutes it, which is the only case
    /// where the two differ.
    pub shield_index: u8,
    pub tax_hap_empire: i8,
    /// `+0x29` — owned counties. **One each for realms 1..=5 in the shipped
    /// save**, which is the same correction the county owner bytes carry, read
    /// off a different field.
    pub county_count: u8,
    pub rank: u8,
    pub score: i32,
    pub wages: i32,
    pub gold: i32,
    pub iron: i32,
    pub stone: i32,
    pub wood: i32,
    pub weapons: [i32; WEAPON_TYPES],
    /// `+0x84 + other * 0x10` — this realm's view of each other realm.
    pub pairs: [DiploPair; REALM_RECORDS],
}

impl Realm {
    pub fn in_play(&self) -> bool {
        self.index != 0 && self.strength != 0
    }
}

/// One slot of the six-slot player table — `0x00553D50 + realm * 0x2C`.
///
/// **The four bytes at `+0x00` are the whole reason this is a record and not a
/// name array.** They are the DirectPlay id of the person driving the realm,
/// and `0` means nobody is connected:
///
/// * `FUN_0043E9E2` clears `1 … 5` and then writes
///   `*(int *)(&DAT_00553d50 + g_localPlayer * 0x2c) = g_dpPlayerId`;
/// * `FUN_0043E98B(id)` walks `1 … 5` and returns the realm whose `+0x00`
///   equals `id` — the id → realm lookup the network read path needs;
/// * `Mp_DropDepartedPlayers` (`0x0049B2A3`) eliminates a realm marked human
///   whose `+0x00` **has gone to zero**;
/// * `Player_SetHuman` writes it only when the caller passes something other
///   than `999`, which is the single-player sentinel;
/// * `Net_ReadField(&DAT_00553d50 + realm * 0x2c, 4)` puts it on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Player {
    pub index: usize,
    /// `+0x00` — the DirectPlay player id, or 0 for nobody.
    pub dp_player_id: i32,
    /// `+0x04` — the lord's name. A person's is what was typed on setup page 4
    /// and an AI's is `L2.eng` group 7 indexed by the **lord**.
    pub name: [u8; PLAYER_NAME_LEN],
    /// `+0x25` — the shield the slot holds. Written from
    /// `g_realms[realm].shieldIndex` by `Player_SetHuman` and by the colour
    /// picker, and read back by `Realms_AssignLords` to mark a colour taken.
    pub shield: u8,
    /// `+0x27` — 1 from `Player_SetHuman`, 0 from `FUN_0049BB9D`, its opposite
    /// number. **Not a clean "a person drives this"**: `Realm_Eliminate` also
    /// leaves 1 behind, so read it as the byte those three write and take
    /// `g_realms[realm].isHuman` for the question itself.
    pub human_flag: u8,
}

impl Player {
    pub fn name(&self) -> String {
        self.name.iter().take_while(|b| **b != 0).map(|b| *b as char).collect()
    }

    pub fn is_named(&self) -> bool {
        self.name[0] != 0
    }
}

/// **They are one array on purpose.** The type byte at `+0x08` is the only
/// thing that tells them apart, and three offsets carry a different field per
/// type — `+0x14F` is an army's name index and a merchant's route number,
/// `+0x164` is an army's year of formation and a merchant's route cursor, and
/// `+0x167` is an army's county-defence mark and a merchant's or transport's
/// county. This struct reads the bytes and names them after the offset where
/// the two meanings would fight; [`Unit::route`] and [`Unit::route_cursor`]
/// are the merchant-side readings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unit {
    pub index: usize,
    /// `+0x00` — realm 1…5, **0 means the slot is free**, 6 an ownerless unit
    /// (a merchant, or a defence levied by a county).
    pub owner: u8,
    pub owner_is_human: bool,
    pub shield: u8,
    pub player_driven: bool,
    pub sprite_frame: u8,
    /// `+0x08` — 1 army, 2 revolting peasants, 3 merchant, 4 transport.
    pub kind: u8,
    pub facing: u8,
    pub x: u8,
    pub y: u8,
    /// `+0x0C` — `(y * 64 + x) * 8`, a byte offset into `g_tiles`. It is a
    /// **self-checking invariant**: it has to agree with `x` and `y`, and
    /// nothing but a correct stride makes it agree 151 records running.
    pub tile_offset: i32,
    pub county: u8,
    pub home_county: u8,
    pub step_target_x: u8,
    pub step_target_y: u8,
    pub dest_x: u8,
    pub dest_y: u8,
    pub walk_phase: u8,
    /// `+0x1C` — steps remaining in [`Unit::path`].
    pub path_len: u8,
    /// `+0x1D …` — 150 `(x, y)` pairs. The original walks them from
    /// `path_len − 1` downwards, so the *next* step is the last live entry.
    pub path: [(u8, u8); UNIT_PATH_STEPS],
    /// `+0x14C` — 0 idle, 2 moving.
    pub move_state: u8,
    pub on_road: bool,
    pub ignore_settlements: bool,
    /// `+0x14F` — an army's name index, a merchant's route number.
    pub name_index: u8,
    pub needs_destination: bool,
    pub dest_county: u8,
    pub merge_target: u8,
    pub moves_used: i8,
    /// `+0x154` — 15 for an army and 10 for the other three, **but rewritten
    /// unconditionally at the top of each type's tick handler**. A unit created
    /// mid-turn carries 0 until it is next ticked, which is what the six
    /// merchants of the England turn-one fixture and the raised defence of
    /// `battle-during.sav` both show. It is a tick-maintained invariant, not an
    /// initial value. `docs/armies.md` §8b.4.
    pub move_allowance: i8,
    pub starvation: i8,
    pub wages: i32,
    /// `+0x164` — an army's year of formation, a merchant's route cursor in the
    /// low byte.
    pub year_formed: u16,
    /// `+0x166` — an army's morale. `Merchant_SpawnAll` writes **100** here for
    /// a merchant and nothing reads it back. `docs/formats/plane4.md` §5.
    pub morale: u8,
    /// `+0x167` — **two fields at one offset**: an army's county-defence mark
    /// (1 levied on the spot, 2 an existing army pressed into the role), and a
    /// merchant's or transport's county. `docs/armies.md` §1.5, §8.1.
    pub role: u8,
    pub men: i32,
    /// `+0x16C + t*2` — eleven `i16`s; the campaign writes 0…6.
    pub troops: [i16; UNIT_TROOP_SLOTS],
    pub merc_troop: u8,
    pub merc_men: u8,
    pub merc_band: u8,
    /// `+0x198` — non-zero: garrisoned in that county's castle. Excluded from
    /// the county troop count and never starves.
    pub garrison_county: u8,
    pub besieging_county: u8,
    pub besieged_by: u8,
    pub siege_seasons_left: u8,
}

impl Unit {
    pub fn is_live(&self) -> bool {
        self.index != 0 && self.owner != 0 && self.kind != 0
    }

    pub fn path(&self) -> Vec<(u8, u8)> {
        let n = (self.path_len as usize).min(UNIT_PATH_STEPS);
        self.path[..n].iter().rev().copied().collect()
    }

    /// The tile `+0x0C` names.
    pub fn tile_of_offset(&self) -> Option<(u8, u8)> {
        if self.tile_offset < 0 || self.tile_offset % 8 != 0 {
            return None;
        }
        let index = self.tile_offset / 8;
        if index >= 64 * 64 {
            return None;
        }
        Some(((index % 64) as u8, (index / 64) as u8))
    }

    /// Whether `+0x0C` agrees with `x` and `y`.
    pub fn tile_offset_agrees(&self) -> bool {
        self.tile_of_offset() == Some((self.x, self.y))
    }

    pub fn route(&self) -> u8 {
        self.name_index
    }

    /// A merchant's route cursor — the low byte of `+0x164`.
    pub fn route_cursor(&self) -> u8 {
        self.year_formed as u8
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Globals {
    pub county_count: i32,
    pub scenario_index: i32,
    pub local_player: i32,
    pub season: i32,
    pub season_next: i32,
    pub year: i32,
    pub turn_count: i32,
    pub turn_phase: i32,
    pub turn_phase_step: i32,
    pub opt_difficulty: i32,
    pub opt_advanced_farming: i32,
    pub opt_armies_eat: i32,
    pub opt_exploration: i32,
    pub opt_time_limit: i32,
    /// `0x0053F268` — **how many AI lords the game was started with.**
    ///
    /// `Setup_CommitOptions` (`0x00499DC3`) computes it as
    /// `(nobles + 2) - humanPlayers` from the *Nobles* drop-down, and
    /// `Realms_AssignLords` hands out at most this many lords and writes
    /// `strength = 0` into every realm past it. It is the only one of the six
    /// *starting condition* options that survives into the save at all — see
    /// the note on [`Globals::opt_exploration`]'s neighbours below.
    pub ai_lords: i32,
    pub merchant_count: i32,
    pub weather_county: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct County {
    pub index: usize,
    pub owner: u8,
    pub health_band: i8,
    pub health_meter: i8,
    pub happiness: i8,
    pub happiness_last: i8,
    /// `+0x0E`, `+0x10`, `+0x11` — the three happiness terms as the season
    /// left them. **Not the same thing as the `shown_*` copies**: those are
    /// taken while happiness is computed, and `d_hap_ration` is written again
    /// by the ration *preview* that ends the season. County 1 of the shipped
    /// save is where the two disagree.
    pub d_hap_tax: i8,
    pub d_hap_health: i8,
    pub d_hap_ration: i8,
    pub happiness_avg: i8,
    pub happiness_sum: i32,
    pub shown_tax: i8,
    pub shown_ration: i8,
    pub shown_health: i8,
    pub shown_army: i8,
    pub shown_events: i8,
    pub unrest: u8,
    pub population: i32,
    pub pop_last: i32,
    pub births: i32,
    pub deaths: i32,
    pub emigrants: i32,
    pub immigrants: i32,
    pub neighbour_count: u8,
    pub anchor_x: u8,
    pub anchor_y: u8,
    pub pop_band: u8,
    pub tax_rate: u8,
    pub tax_collected: i32,
    pub ration_achieved: i8,
    pub ration_wanted: i8,
    /// `+0x15F` — the percentage of the food requirement taken from livestock
/// **0 means all grain**, which is what puts county 1 on
    /// Half rations.
    pub ration_split: i8,
    pub grain_eaten: i32,
    pub herd_eaten: i32,
    /// `+0x180`, `+0x184` — the stores as the last ration pass saw them. In
    /// the England turn-one fixture they equal [`County::grain`] and [`County::herd`]
    /// exactly, which is evidence that the pass that wrote them spent nothing.
    pub grain_available: i32,
    pub herd_available: i32,
    pub castle_type: u8,
    pub castle_building: u8,
    pub fields_fallow: u8,
    pub fields_cattle: u8,
    pub fields_grain: u8,
    pub fertility: i32,
    pub weather: u8,
    pub dryness: i8,
    pub grain: i32,
    pub herd: i32,
    /// `+0x5C …` — the adjacency ids, of which the first
    /// [`County::neighbour_count`] are live. Read as the full sixteen slots the
/// record affords so the trailing zeros are visible.
    pub neighbours: [u8; NEIGHBOUR_SLOTS],
}

impl County {
    pub fn neighbours(&self) -> &[u8] {
        &self.neighbours[..(self.neighbour_count as usize).min(NEIGHBOUR_SLOTS)]
    }
}

impl County {
    pub fn is_county(&self) -> bool {
        self.population > 0 || self.owner != 0
    }

    pub fn is_owned(&self) -> bool {
        self.owner != 0
    }
}

