//! `Save_Write` (`0x004ADE93`) does not serialise anything. It walks a table of
//! `{u32 address, u32 length}` records at **`0x004DE960`**, writes each of those
//! regions of live memory back to back, and then appends `castles.dat` as
//! sixteen blocks of `0x3200`.

mod pe;
pub use pe::*;
mod layout;
pub use layout::*;
mod save;
pub use save::*;

mod types;
pub use types::*;

/// Where the block table lives, and how the game walks it. **[V]** — read out
/// of `Save_Write`'s own loop.
const SAVE_TABLE_VA: u32 = 0x004D_E960;
const MAX_ENTRIES: usize = 0xE1;
const CASTLE_BLOCKS: usize = 16;
const CASTLE_BLOCK: usize = 0x3200;

const IMAGE_BASE: u32 = 0x0040_0000;

pub const COUNTY_BASE: u32 = 0x0053_F9B0;
pub const COUNTY_STRIDE: usize = 0x300;
pub const COUNTY_RECORDS: usize = 17;

pub const REALM_BASE: u32 = 0x0057_BF00;
pub const REALM_STRIDE: usize = 0x160;
pub const REALM_RECORDS: usize = 6;

/// **`g_players` — the six 44-byte player slots, of which `g_playerNames`
/// (`0x00553D54`) is the *name member*, not an array of its own.**
///
/// `g_saveBlocks` entry 2 is `{0x00553D50, 264}` and `264 = 6 × 0x2C`, which
/// invites the reading that six 44-byte names begin four bytes before
/// `g_playerNames` — a block misaligned by a dword, with realm 5's record
/// running past its end. It is neither. `0x00553D50` is the base of a six-slot
/// **player table** whose name lives at `+0x04`, so the block covers slots
/// 0 … 5 exactly and nothing is truncated.
///
/// **[V]** — `Player_SetHuman` (`0x0049BAE9`) writes both halves of one slot
/// from one argument list, at the same stride:
///
/// ```c
/// g_realms[realm].isHuman = 1;
/// (&DAT_00553d77)[realm * 0x2c] = 1;                      /* +0x27 */
/// FUN_00401136(name, (int)(&g_playerNames + realm * 0x2c), 0x1f);
/// if (dpId != 999) *(int *)(&DAT_00553d50 + realm * 0x2c) = dpId;  /* +0x00 */
/// if (g_multiplayer == 0)
///     (&DAT_00553d75)[realm * 0x2c] = g_realms[realm].shieldIndex; /* +0x25 */
/// ```
pub const PLAYER_BASE: u32 = 0x0055_3D50;
pub const PLAYER_STRIDE: usize = 0x2C;
/// Slot 0 is never a realm, and nothing writes its name: `Realms_AssignLords`
/// (`0x0049CAAA`) walks `1 … 5` and `Player_SetHuman` is never called for 0.
pub const PLAYER_RECORDS: usize = 6;
/// `+0x04 …` — `FUN_00401136(src, dst, 0x1F)`'s width where a person's typed
/// name is copied in. The AI half copies **sixteen**, `Eng_Seek(7, lord)` then
/// `FUN_00401136(g_engCursor, …, 0x10)`, so an AI lord's record
/// carries the three or five bytes of `L2.eng` that follow its title.
pub const PLAYER_NAME_LEN: usize = 0x1F;

/// The neighbour ids live at county `+0x5C`, and the next identified field is
/// `anchorX` at `+0x6C`. That is sixteen bytes, so sixteen slots is what the
/// record affords — a layout fact.
pub const NEIGHBOUR_SLOTS: usize = 16;

/// One weapon counter per type, realm `+0x140 + t*4`.
pub const WEAPON_TYPES: usize = 6;

/// `g_units` — **one array for four kinds of thing**: armies, revolting
/// peasants, merchants and transports, told apart by the type byte at `+0x08`.
///
/// **[V]** — save block 5 is exactly `151 × 0x1A4 = 63,420` bytes, which is the
/// arithmetic that pins both the stride and the count. `docs/armies.md` §0.
pub const UNIT_BASE: u32 = 0x0052_F0B0;
pub const UNIT_STRIDE: usize = 0x1A4;
pub const UNIT_RECORDS: usize = 151;

/// `+0x16C` is **one eleven-entry `i16` array**, not two. The campaign writes
/// types 0…6; `Army_PrepareForBattle` fills 7…10 from the siege build records
/// at `+0x182` immediately before a battle. `docs/armies.md` §1.3.
pub const UNIT_TROOP_SLOTS: usize = 11;

/// `+0x1D … +0x148` — 150 `(x, y)` pairs, and `+0x1D + 150 × 2 = +0x149`, the
/// next offset anything in the binary references.
pub const UNIT_PATH_STEPS: usize = 150;

pub const MERCHANT_ROUTES: u32 = 0x0056_7970;
pub const MERCHANT_ROUTE_ROWS: usize = 6;
pub const MERCHANT_ROUTE_LEN: usize = 16;

pub const MERCHANT_START_COUNTIES: u32 = 0x0056_9518;

mod globals {
    pub const COUNTY_COUNT: u32 = 0x0056_D5DC;
    pub const SCENARIO_INDEX: u32 = 0x0053_F034;
    pub const LOCAL_PLAYER: u32 = 0x0057_C8CC;
    pub const SEASON: u32 = 0x0057_C934;
    pub const SEASON_NEXT: u32 = 0x0057_C92C;
    pub const YEAR: u32 = 0x0055_3EDC;
    pub const TURN_COUNT: u32 = 0x0055_3240;
    pub const TURN_PHASE: u32 = 0x0056_9584;
    pub const TURN_PHASE_STEP: u32 = 0x0053_F658;
    pub const OPT_DIFFICULTY: u32 = 0x0053_F23C;
    pub const OPT_ADVANCED_FARMING: u32 = 0x0053_F25C;
    pub const OPT_ARMIES_EAT: u32 = 0x0053_F260;
    pub const OPT_EXPLORATION: u32 = 0x0053_F264;
    pub const OPT_TIME_LIMIT: u32 = 0x0053_F26C;
    /// **The six option globals in this block are the six that `Save_Write`
    /// stores, and there are not more.** The custom game sets twelve; the block
    /// table covers `0x0053F23C`, `0x0053F258`, `0x0053F25C`, `0x0053F260`,
    /// `0x0053F264`, `0x0053F268` and `0x0053F26C` and nothing else in the
    /// range. So `g_optFightHumansOnly` (`0x0053F284`) is **not saved**, nor
    /// are the starting gold, castle, armoury, garrison or county-status
    /// globals, nor the twelve selections at `0x0053F288`.
    pub const AI_LORDS: u32 = 0x0053_F268;
    pub const MERCHANT_COUNT: u32 = 0x0055_30B4;
    pub const WEATHER_COUNTY: u32 = 0x0055_4020;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveError {
    NotPe,
    BadBlockTable,
    SizeMismatch { expected: usize, actual: usize },
    NotSaved { va: u32 },
    OutOfRange { index: usize, count: usize },
}

impl core::fmt::Display for SaveError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            SaveError::NotPe => write!(f, "not a PE executable"),
            SaveError::BadBlockTable => write!(f, "the save-block table is unreadable"),
            SaveError::SizeMismatch { expected, actual } => write!(
                f,
                "the block table accounts for {expected} bytes but the save is {actual}; \
                 the schema was misread"
            ),
            SaveError::NotSaved { va } => write!(f, "address {va:#010x} is in no saved block"),
            SaveError::OutOfRange { index, count } => {
                write!(f, "record {index} of {count}")
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Block {
    pub va: u32,
    pub len: u32,
    pub offset: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    blocks: Vec<Block>,
    total: usize,
}

#[derive(Debug, Clone)]
pub struct Save {
    layout: Layout,
    bytes: Vec<u8>,
}

