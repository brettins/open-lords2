//! Reading a Lords of the Realm II save file.
//!
//! # The save is a memory dump, and the executable is its schema
//!
//! `Save_Write` (`0x004ADE93`) does not serialise anything. It walks a table of
//! `{u32 address, u32 length}` records at **`0x004DE960`**, writes each of those
//! regions of live memory back to back, and then appends `castles.dat` as
//! sixteen blocks of `0x3200`. There is no header, no field order, and no
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
//! otherwise, so a wrong schema fails loudly rather than returning plausible
//! numbers from the wrong offsets.
//!
//! # What this is for
//!
//! The England turn-one scenario. `crates/l2-kingdom` reproduces the *rules* exactly, but
//! its test built the *scenario* from a document rather than from the save —
//! four counties owned by one realm, where the file actually holds five owned by
//! five different realms. Reading the save turns that from a self-consistent
//! test into a real one.

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

/// The neighbour ids live at county `+0x5C`, and the next identified field is
/// `anchorX` at `+0x6C`. That is sixteen bytes, so sixteen slots is what the
/// record affords — a layout fact rather than a count anyone has observed used.
/// The largest count in the England turn-one fixture is seven.
pub const NEIGHBOUR_SLOTS: usize = 16;

/// One weapon counter per type, realm `+0x140 + t*4`.
pub const WEAPON_TYPES: usize = 6;

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
    pub const MERCHANT_COUNT: u32 = 0x0055_30B4;
    pub const WEATHER_COUNTY: u32 = 0x0055_4020;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveError {
    NotPe,
    /// The block table ran off the end of the image, or held nothing.
    BadBlockTable,
    /// The blocks and castle data do not account for the file exactly. The
    /// arithmetic closing is the evidence the schema was read correctly, so a
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

/// Little-endian by construction rather than by host layout: a reader that
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
    /// Walk `Save_Write`'s table. Stops at the first zero length, exactly as the
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
        })
    }

    pub fn realms(&self) -> Result<Vec<Realm>, SaveError> {
        (0..REALM_RECORDS).map(|i| self.realm(i)).collect()
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
            merchant_count: self.i32_at(globals::MERCHANT_COUNT)?,
            weather_county: self.i32_at(globals::WEATHER_COUNTY)?,
        })
    }
}

/// A realm as the England turn-one fixture holds it. Index 0 is never a realm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Realm {
    pub index: usize,
    pub ai_step: i32,
    /// `+0x04` — a weighted strength count, not the `inPlay` flag
    /// `docs/kingdom.md` §2 called it. Every reader tests it against zero.
    pub strength: u8,
    pub is_human: bool,
    pub lord: u8,
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
}

impl Realm {
    /// A realm somebody is playing. Realm 0 is an array slot.
    pub fn in_play(&self) -> bool {
        self.index != 0 && self.strength != 0
    }
}

/// The scalars `Save_Write` stores outside `g_counties` and `g_realms`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Globals {
    pub county_count: i32,
    pub scenario_index: i32,
    /// `g_localPlayer` — the realm a person is driving. **1** in the shipped
    /// save, which owns county 8 and nothing else.
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
    pub merchant_count: i32,
    pub weather_county: i32,
}

/// A county as the England turn-one fixture holds it.
///
/// Only fields whose meaning is established in `docs/kingdom.md` are read. The
/// record is `0x300` bytes and most of it is still unnamed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct County {
    pub index: usize,
    /// Realm number, or 0 for unowned. **Not** a human/AI flag — the shipped
    /// save has five owned counties, one for each of realms 1 to 5.
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
    /// rather than grain. **0 means all grain**, which is what puts county 1 on
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
    /// record affords so the trailing zeros are visible rather than assumed.
    pub neighbours: [u8; NEIGHBOUR_SLOTS],
}

impl County {
    /// The neighbour ids actually present, in stored order.
    pub fn neighbours(&self) -> &[u8] {
        &self.neighbours[..(self.neighbour_count as usize).min(NEIGHBOUR_SLOTS)]
    }
}

impl County {
    /// Records 0, 15 and 16 are array slots rather than places: zero
    /// population, zero everything.
    pub fn is_county(&self) -> bool {
        self.population > 0 || self.owner != 0
    }

    pub fn is_owned(&self) -> bool {
        self.owner != 0
    }
}
