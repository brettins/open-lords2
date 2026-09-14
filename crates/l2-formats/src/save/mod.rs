//! Reading a Lords of the Realm II save file.
//!
//! # The save is a memory dump, and the executable is its schema
//!
//! `Save_Write` (`0x004ADE93`) does not serialise anything. It walks a table of
//! `{u32 address, u32 length}` records at **`0x004DE960`**, writes each of those
//! regions of live memory back to back, and then appends `castles.dat` as
//! sixteen blocks of `0x3200`.
//! version — the file *is* the game's `.data`, in the order that table names.
//!
//! Two consequences shape this module.
//!
//! **The schema lives in the user's executable, not in this repository.** To
//! know where county 3 sits in the file you must read the block table out of
//! `Lords2.exe` and map a runtime address through it. That is why [`Save::open`]
//! takes the executable's bytes as well as the save's: we ship code, the user
//! supplies both the data and the layout that describes it. Nothing derived from
//! the binary is embedded here.
//!
//! **The whole reading validates itself.** The blocks' lengths sum to a number,
//! plus sixteen castle blocks, and that total must equal the file's size
//! exactly. For the shipped `lastturn.sav` that is
//! `267,028 + 16 × 12,800 = 471,828` — and if a single length were misread the
//! arithmetic would not close. [`Save::open`] checks it and refuses the file
//! otherwise,
//! numbers from the wrong offsets.
//!
//! # What this is for
//!
//! The England turn-one scenario. `crates/l2-kingdom` reproduces the *rules* exactly, but
//! its test built the *scenario* from a document —
//! four counties owned by one realm, where the file holds five owned by
//! five different realms. Reading the save turns that from a self-consistent
//! test into a real one.

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
/// `castles.dat`, appended after the blocks: sixteen of these.
const CASTLE_BLOCKS: usize = 16;
const CASTLE_BLOCK: usize = 0x3200;

/// The image base. `Lords2.exe` has no ASLR and a fixed base, which is what
/// makes every address in this project a constant.
const IMAGE_BASE: u32 = 0x0040_0000;

pub const COUNTY_BASE: u32 = 0x0053_F9B0;
pub const COUNTY_STRIDE: usize = 0x300;
/// Seventeen records, of which fourteen are counties on the England map: record
/// 0 is never a county, and the last two are spare.
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
/// The largest count in the England turn-one fixture is seven.
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
/// Record 0 is never a unit; the array is `1 ..= 150`.
pub const UNIT_RECORDS: usize = 151;

/// `+0x16C` is **one eleven-entry `i16` array**, not two. The campaign writes
/// types 0…6; `Army_PrepareForBattle` fills 7…10 from the siege build records
/// at `+0x182` immediately before a battle. `docs/armies.md` §1.3.
pub const UNIT_TROOP_SLOTS: usize = 11;

/// `+0x1D … +0x148` — 150 `(x, y)` pairs, and `+0x1D + 150 × 2 = +0x149`, the
/// next offset anything in the binary references.
pub const UNIT_PATH_STEPS: usize = 150;

/// `g_merchantRoutes` — six rows of sixteen county ids, built from plane 4 of
/// the map's castle tiles by `Map_LoadPlanes`. `docs/formats/plane4.md` §1.
pub const MERCHANT_ROUTES: u32 = 0x0056_7970;
pub const MERCHANT_ROUTE_ROWS: usize = 6;
pub const MERCHANT_ROUTE_LEN: usize = 16;

/// `g_merchantStartCounty` — six bytes, the county each route's merchant is
/// spawned in. A zero entry stops `Merchant_SpawnAll` dead
/// skipped. `docs/formats/plane4.md` §2.1.
pub const MERCHANT_START_COUNTIES: u32 = 0x0056_9518;

/// The scalars `Save_Write` stores outside the two arrays.
///
/// Each is its own four-byte entry in the block table, so their addresses are
/// pinned by the same arithmetic that pins everything else: if one were wrong
/// the block it names would not exist and [`Save::i32_at`] would say so rather
/// than returning a plausible number from somewhere else.
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
    ///
/// Five of those are spent while the world is built and are not
    /// needed afterwards. `g_optFightHumansOnly` is not: it decides every turn
    /// whether a battle the person is not in is fought or auto-resolved, and a
    /// reloaded game takes whatever value happens to be in memory. Asking for
    /// it returns [`SaveError::NotSaved`], which is how this was found — the
    /// battle fixtures went red the moment it was added to this list.
    /// `docs/bugs.md`.
    pub const AI_LORDS: u32 = 0x0053_F268;
    pub const MERCHANT_COUNT: u32 = 0x0055_30B4;
    pub const WEATHER_COUNTY: u32 = 0x0055_4020;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveError {
    NotPe,
    /// The block table ran off the end of the image, or held nothing.
    BadBlockTable,
    /// The blocks and castle data do not account for the file exactly. The
    /// arithmetic closing is the evidence the schema was read correctly,
    /// mismatch means the reading is wrong, not that the file is odd.
    SizeMismatch { expected: usize, actual: usize },
    /// A runtime address that no saved block covers.
    NotSaved { va: u32 },
    /// A record index past the end of its array.
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

/// One saved region: where it lived in memory, and where it landed in the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Block {
    pub va: u32,
    pub len: u32,
    pub offset: usize,
}

/// The save's schema, read out of the executable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    blocks: Vec<Block>,
    total: usize,
}

/// A save file, with the schema needed to read it.
#[derive(Debug, Clone)]
pub struct Save {
    layout: Layout,
    bytes: Vec<u8>,
}

