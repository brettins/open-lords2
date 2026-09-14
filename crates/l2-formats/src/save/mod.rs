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

/// Just enough PE to turn a virtual address into a file offset.
///
/// Deliberately minimal and dependency-free: this crate has no third-party
/// dependencies and a save reader is no reason to acquire one.
struct Pe<'a> {
    bytes: &'a [u8],
    sections: Vec<(u32, u32, u32, u32)>, // vaddr, vsize, rawptr, rawsize
}

impl<'a> Pe<'a> {
    fn parse(bytes: &'a [u8]) -> Result<Pe<'a>, SaveError> {
        let pe_off = read_u32(bytes, 0x3C).ok_or(SaveError::NotPe)? as usize;
        if read_u32(bytes, pe_off) != Some(0x0000_4550) {
            return Err(SaveError::NotPe);
        }
        let n = read_u16(bytes, pe_off + 6).ok_or(SaveError::NotPe)? as usize;
        let opt = read_u16(bytes, pe_off + 20).ok_or(SaveError::NotPe)? as usize;
        let table = pe_off + 24 + opt;
        let mut sections = Vec::with_capacity(n);
        for i in 0..n {
            let o = table + i * 40;
            let g = |k: usize| read_u32(bytes, o + k).ok_or(SaveError::NotPe);
            sections.push((g(12)?, g(8)?, g(20)?, g(16)?));
        }
        Ok(Pe { bytes, sections })
    }

    fn offset(&self, va: u32) -> Option<usize> {
        let rva = va.checked_sub(IMAGE_BASE)?;
        for &(vaddr, vsize, rawptr, rawsize) in &self.sections {
            if rva >= vaddr && rva < vaddr + vsize.max(rawsize) {
                return Some((rawptr + (rva - vaddr)) as usize);
            }
        }
        None
    }

    fn u32_at(&self, va: u32) -> Option<u32> {
        read_u32(self.bytes, self.offset(va)?)
    }
}

/// Little-endian by construction: a reader that
/// depends on the machine's endianness is a desync waiting for a different
/// machine.
fn read_u32(bytes: &[u8], at: usize) -> Option<u32> {
    let s = bytes.get(at..at + 4)?;
    Some(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn read_u16(bytes: &[u8], at: usize) -> Option<u16> {
    let s = bytes.get(at..at + 2)?;
    Some(u16::from_le_bytes([s[0], s[1]]))
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

impl Layout {
    /// Walk `Save_Write`'s table. Stops at the first zero length.
    /// game's loop does.
    pub fn from_executable(exe: &[u8]) -> Result<Layout, SaveError> {
        let pe = Pe::parse(exe)?;
        let mut blocks = Vec::new();
        let mut cum = 0usize;
        for i in 0..MAX_ENTRIES {
            let at = SAVE_TABLE_VA + (i as u32) * 8;
            let va = pe.u32_at(at).ok_or(SaveError::BadBlockTable)?;
            let len = pe.u32_at(at + 4).ok_or(SaveError::BadBlockTable)?;
            if len == 0 {
                break;
            }
            blocks.push(Block { va, len, offset: cum });
            cum += len as usize;
        }
        if blocks.is_empty() {
            return Err(SaveError::BadBlockTable);
        }
        Ok(Layout { blocks, total: cum })
    }

    pub fn blocks(&self) -> &[Block] {
        &self.blocks
    }

    /// The size a save written from this layout must have.
    pub fn expected_len(&self) -> usize {
        self.total + CASTLE_BLOCKS * CASTLE_BLOCK
    }

    /// Where a runtime address landed in the file.
    pub fn offset_of(&self, va: u32) -> Option<usize> {
        self.blocks
            .iter()
            .find(|b| va >= b.va && va < b.va + b.len)
            .map(|b| b.offset + (va - b.va) as usize)
    }
}

/// A save file, with the schema needed to read it.
#[derive(Debug, Clone)]
pub struct Save {
    layout: Layout,
    bytes: Vec<u8>,
}

impl Save {
    /// Read a save, checking the arithmetic closes.
    pub fn open(exe: &[u8], save: &[u8]) -> Result<Save, SaveError> {
        let layout = Layout::from_executable(exe)?;
        let expected = layout.expected_len();
        if save.len() != expected {
            return Err(SaveError::SizeMismatch { expected, actual: save.len() });
        }
        Ok(Save { layout, bytes: save.to_vec() })
    }

    pub fn layout(&self) -> &Layout {
        &self.layout
    }

    fn at(&self, va: u32) -> Result<usize, SaveError> {
        self.layout.offset_of(va).ok_or(SaveError::NotSaved { va })
    }

    pub fn u8_at(&self, va: u32) -> Result<u8, SaveError> {
        let o = self.at(va)?;
        self.bytes.get(o).copied().ok_or(SaveError::NotSaved { va })
    }

    pub fn i8_at(&self, va: u32) -> Result<i8, SaveError> {
        Ok(self.u8_at(va)? as i8)
    }

    pub fn u16_at(&self, va: u32) -> Result<u16, SaveError> {
        let o = self.at(va)?;
        let s = self.bytes.get(o..o + 2).ok_or(SaveError::NotSaved { va })?;
        Ok(u16::from_le_bytes([s[0], s[1]]))
    }

    pub fn i16_at(&self, va: u32) -> Result<i16, SaveError> {
        Ok(self.u16_at(va)? as i16)
    }

    pub fn i32_at(&self, va: u32) -> Result<i32, SaveError> {
        let o = self.at(va)?;
        let s = self.bytes.get(o..o + 4).ok_or(SaveError::NotSaved { va })?;
        Ok(i32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }

    /// One county record, by array index. Index 0 is never a county.
    pub fn county(&self, index: usize) -> Result<County, SaveError> {
        if index >= COUNTY_RECORDS {
            return Err(SaveError::OutOfRange { index, count: COUNTY_RECORDS });
        }
        let base = COUNTY_BASE + (index * COUNTY_STRIDE) as u32;
        Ok(County {
            index,
            owner: self.u8_at(base + 0x05)?,
            health_band: self.i8_at(base + 0x09)?,
            health_meter: self.i8_at(base + 0x0B)?,
            happiness: self.i8_at(base + 0x0C)?,
            happiness_last: self.i8_at(base + 0x0D)?,
            d_hap_tax: self.i8_at(base + 0x0E)?,
            d_hap_health: self.i8_at(base + 0x10)?,
            d_hap_ration: self.i8_at(base + 0x11)?,
            happiness_avg: self.i8_at(base + 0x18)?,
            happiness_sum: self.i32_at(base + 0x1C)?,
            shown_tax: self.i8_at(base + 0x12)?,
            shown_ration: self.i8_at(base + 0x13)?,
            shown_health: self.i8_at(base + 0x14)?,
            shown_army: self.i8_at(base + 0x15)?,
            shown_events: self.i8_at(base + 0x17)?,
            unrest: self.u8_at(base + 0x20)?,
            population: self.i32_at(base + 0x24)?,
            pop_last: self.i32_at(base + 0x28)?,
            births: self.i32_at(base + 0x30)?,
            deaths: self.i32_at(base + 0x34)?,
            emigrants: self.i32_at(base + 0x3C)?,
            immigrants: self.i32_at(base + 0x40)?,
            neighbour_count: self.u8_at(base + 0x5A)?,
            anchor_x: self.u8_at(base + 0x6C)?,
            anchor_y: self.u8_at(base + 0x6D)?,
            pop_band: self.u8_at(base + 0xB8)?,
            tax_rate: self.u8_at(base + 0xB9)?,
            tax_collected: self.i32_at(base + 0xBC)?,
            ration_achieved: self.i8_at(base + 0x15D)?,
            ration_wanted: self.i8_at(base + 0x15E)?,
            ration_split: self.i8_at(base + 0x15F)?,
            grain_eaten: self.i32_at(base + 0x178)?,
            herd_eaten: self.i32_at(base + 0x17C)?,
            grain_available: self.i32_at(base + 0x180)?,
            herd_available: self.i32_at(base + 0x184)?,
            castle_type: self.u8_at(base + 0x1C0)?,
            castle_building: self.u8_at(base + 0x1C1)?,
            fields_fallow: self.u8_at(base + 0x1FF)?,
            fields_cattle: self.u8_at(base + 0x200)?,
            fields_grain: self.u8_at(base + 0x201)?,
            fertility: self.i32_at(base + 0x208)?,
            weather: self.u8_at(base + 0x21B)?,
            dryness: self.i8_at(base + 0x21D)?,
            grain: self.i32_at(base + 0x224)?,
            herd: self.i32_at(base + 0x250)?,
            neighbours: {
                let mut ids = [0u8; NEIGHBOUR_SLOTS];
                for (slot, id) in ids.iter_mut().enumerate() {
                    *id = self.u8_at(base + 0x5C + slot as u32)?;
                }
                ids
            },
        })
    }

    /// Every record, including the three that are never counties. Callers that
    /// want only the real ones filter on [`County::is_county`].
    pub fn counties(&self) -> Result<Vec<County>, SaveError> {
        (0..COUNTY_RECORDS).map(|i| self.county(i)).collect()
    }

    /// One realm record, by array index. Index 0 is never a realm.
    pub fn realm(&self, index: usize) -> Result<Realm, SaveError> {
        if index >= REALM_RECORDS {
            return Err(SaveError::OutOfRange { index, count: REALM_RECORDS });
        }
        let base = REALM_BASE + (index * REALM_STRIDE) as u32;
        Ok(Realm {
            index,
            ai_step: self.i32_at(base)?,
            strength: self.u8_at(base + 0x04)?,
            is_human: self.u8_at(base + 0x05)? != 0,
            lord: self.u8_at(base + 0x07)?,
            shield_index: self.u8_at(base + 0x0A)?,
            tax_hap_empire: self.i8_at(base + 0x28)?,
            county_count: self.u8_at(base + 0x29)?,
            rank: self.u8_at(base + 0x2B)?,
            score: self.i32_at(base + 0x50)?,
            wages: self.i32_at(base + 0xFC)?,
            gold: self.i32_at(base + 0x118)?,
            iron: self.i32_at(base + 0x120)?,
            stone: self.i32_at(base + 0x128)?,
            wood: self.i32_at(base + 0x130)?,
            weapons: {
                let mut w = [0i32; WEAPON_TYPES];
                for (t, slot) in w.iter_mut().enumerate() {
                    *slot = self.i32_at(base + 0x140 + (t * 4) as u32)?;
                }
                w
            },
            // **`+0x84 … +0xE3`, and nothing read it until now.** The whole
            // diplomatic matrix — every alliance, every grudge — was in the
            // file and dropped on the floor, so `scenario::from_save` ran
            // `Diplo_Init` instead. Right for a turn-one fixture, and wrong for
            // every later save: a player who loaded a mid-game file found the
            // AI had forgotten every war.
            //
            // Found because somebody built the consumer. `docs/decisions.md`
            // C83.
            pairs: {
                let mut p = [DiploPair::default(); REALM_RECORDS];
                for (other, slot) in p.iter_mut().enumerate() {
                    let at = base + 0x84 + (other * 0x10) as u32;
                    *slot = DiploPair {
                        standing: self.i8_at(at)?,
                        allied: self.u8_at(at + 1)? != 0,
                        grudge: self.u8_at(at + 2)?,
                        warnings_sent: self.u8_at(at + 3)?,
                        at_war: self.u8_at(at + 4)? != 0,
                        compliments_from: self.u8_at(at + 5)?,
                        best_gift: self.i32_at(at + 8)?,
                        has_mail: self.u8_at(at + 0x0C)? != 0,
                        help_price_multiple: self.u8_at(at + 0x0D)?,
                    };
                }
                p
            },
        })
    }

    pub fn realms(&self) -> Result<Vec<Realm>, SaveError> {
        (0..REALM_RECORDS).map(|i| self.realm(i)).collect()
    }

    /// One `g_units` record, by slot. Slot 0 is never a unit.
    ///
    /// **Everything is read, including the fields whose meaning depends on the
    /// type byte.** `+0x14F`, `+0x164` and `+0x167` are each two fields sharing
    /// one offset (`docs/armies.md` §1), so this layer reads the bytes and
    /// leaves the naming to whoever knows the type — which is the same division
    /// the rest of this module keeps.
    pub fn unit(&self, index: usize) -> Result<Unit, SaveError> {
        if index >= UNIT_RECORDS {
            return Err(SaveError::OutOfRange { index, count: UNIT_RECORDS });
        }
        let base = UNIT_BASE + (index * UNIT_STRIDE) as u32;
        Ok(Unit {
            index,
            owner: self.u8_at(base)?,
            owner_is_human: self.u8_at(base + 0x01)? != 0,
            shield: self.u8_at(base + 0x02)?,
            player_driven: self.u8_at(base + 0x06)? != 0,
            sprite_frame: self.u8_at(base + 0x07)?,
            kind: self.u8_at(base + 0x08)?,
            facing: self.u8_at(base + 0x09)?,
            x: self.u8_at(base + 0x0A)?,
            y: self.u8_at(base + 0x0B)?,
            tile_offset: self.i32_at(base + 0x0C)?,
            county: self.u8_at(base + 0x10)?,
            home_county: self.u8_at(base + 0x11)?,
            step_target_x: self.u8_at(base + 0x14)?,
            step_target_y: self.u8_at(base + 0x15)?,
            dest_x: self.u8_at(base + 0x16)?,
            dest_y: self.u8_at(base + 0x17)?,
            walk_phase: self.u8_at(base + 0x1B)?,
            path_len: self.u8_at(base + 0x1C)?,
            path: {
                let mut steps = [(0u8, 0u8); UNIT_PATH_STEPS];
                for (n, step) in steps.iter_mut().enumerate() {
                    let at = base + 0x1D + (n * 2) as u32;
                    *step = (self.u8_at(at)?, self.u8_at(at + 1)?);
                }
                steps
            },
            move_state: self.u8_at(base + 0x14C)?,
            on_road: self.u8_at(base + 0x14D)? != 0,
            ignore_settlements: self.u8_at(base + 0x14E)? != 0,
            name_index: self.u8_at(base + 0x14F)?,
            needs_destination: self.u8_at(base + 0x150)? != 0,
            dest_county: self.u8_at(base + 0x151)?,
            merge_target: self.u8_at(base + 0x152)?,
            moves_used: self.i8_at(base + 0x153)?,
            move_allowance: self.i8_at(base + 0x154)?,
            starvation: self.i8_at(base + 0x155)?,
            wages: self.i32_at(base + 0x15C)?,
            year_formed: self.u16_at(base + 0x164)?,
            morale: self.u8_at(base + 0x166)?,
            role: self.u8_at(base + 0x167)?,
            men: self.i32_at(base + 0x168)?,
            troops: {
                let mut t = [0i16; UNIT_TROOP_SLOTS];
                for (n, slot) in t.iter_mut().enumerate() {
                    *slot = self.i16_at(base + 0x16C + (n * 2) as u32)?;
                }
                t
            },
            merc_troop: self.u8_at(base + 0x195)?,
            merc_men: self.u8_at(base + 0x196)?,
            merc_band: self.u8_at(base + 0x197)?,
            garrison_county: self.u8_at(base + 0x198)?,
            besieging_county: self.u8_at(base + 0x199)?,
            besieged_by: self.u8_at(base + 0x19A)?,
            siege_seasons_left: self.u8_at(base + 0x19C)?,
        })
    }

    /// Every unit record, including slot 0 and the free ones. Callers that want
    /// only the live units filter on [`Unit::is_live`].
    pub fn units(&self) -> Result<Vec<Unit>, SaveError> {
        (0..UNIT_RECORDS).map(|i| self.unit(i)).collect()
    }

    /// One player slot, by realm index. Slot 0 is never a realm.
    pub fn player(&self, index: usize) -> Result<Player, SaveError> {
        if index >= PLAYER_RECORDS {
            return Err(SaveError::OutOfRange { index, count: PLAYER_RECORDS });
        }
        let base = PLAYER_BASE + (index * PLAYER_STRIDE) as u32;
        let mut name = [0u8; PLAYER_NAME_LEN];
        for (n, slot) in name.iter_mut().enumerate() {
            *slot = self.u8_at(base + 0x04 + n as u32)?;
        }
        Ok(Player {
            index,
            dp_player_id: self.i32_at(base)?,
            name,
            shield: self.u8_at(base + 0x25)?,
            human_flag: self.u8_at(base + 0x27)?,
        })
    }

    pub fn players(&self) -> Result<Vec<Player>, SaveError> {
        (0..PLAYER_RECORDS).map(|i| self.player(i)).collect()
    }

    /// `g_merchantRoutes` — six rows of sixteen county ids, zero-padded.
    pub fn merchant_routes(
        &self,
    ) -> Result<[[u8; MERCHANT_ROUTE_LEN]; MERCHANT_ROUTE_ROWS], SaveError> {
        let mut rows = [[0u8; MERCHANT_ROUTE_LEN]; MERCHANT_ROUTE_ROWS];
        for (r, row) in rows.iter_mut().enumerate() {
            for (n, cell) in row.iter_mut().enumerate() {
                *cell = self.u8_at(MERCHANT_ROUTES + (r * MERCHANT_ROUTE_LEN + n) as u32)?;
            }
        }
        Ok(rows)
    }

    /// `g_merchantStartCounty` — the county each route's merchant was spawned
    /// in.
    pub fn merchant_start_counties(&self) -> Result<[u8; MERCHANT_ROUTE_ROWS], SaveError> {
        let mut out = [0u8; MERCHANT_ROUTE_ROWS];
        for (r, slot) in out.iter_mut().enumerate() {
            *slot = self.u8_at(MERCHANT_START_COUNTIES + r as u32)?;
        }
        Ok(out)
    }

    /// The scalars outside the two arrays — the clock, the options and who is
    /// playing.
    pub fn globals(&self) -> Result<Globals, SaveError> {
        Ok(Globals {
            county_count: self.i32_at(globals::COUNTY_COUNT)?,
            scenario_index: self.i32_at(globals::SCENARIO_INDEX)?,
            local_player: self.i32_at(globals::LOCAL_PLAYER)?,
            season: self.i32_at(globals::SEASON)?,
            season_next: self.i32_at(globals::SEASON_NEXT)?,
            year: self.i32_at(globals::YEAR)?,
            turn_count: self.i32_at(globals::TURN_COUNT)?,
            turn_phase: self.i32_at(globals::TURN_PHASE)?,
            turn_phase_step: self.i32_at(globals::TURN_PHASE_STEP)?,
            opt_difficulty: self.i32_at(globals::OPT_DIFFICULTY)?,
            opt_advanced_farming: self.i32_at(globals::OPT_ADVANCED_FARMING)?,
            opt_armies_eat: self.i32_at(globals::OPT_ARMIES_EAT)?,
            opt_exploration: self.i32_at(globals::OPT_EXPLORATION)?,
            opt_time_limit: self.i32_at(globals::OPT_TIME_LIMIT)?,
            ai_lords: self.i32_at(globals::AI_LORDS)?,
            merchant_count: self.i32_at(globals::MERCHANT_COUNT)?,
            weather_county: self.i32_at(globals::WEATHER_COUNTY)?,
        })
    }
}

