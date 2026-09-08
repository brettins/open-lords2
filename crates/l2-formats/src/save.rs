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
//! The shipped scenario. `crates/l2-kingdom` reproduces the *rules* exactly, but
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
            shown_tax: self.i8_at(base + 0x12)?,
            shown_ration: self.i8_at(base + 0x13)?,
            shown_health: self.i8_at(base + 0x14)?,
            shown_events: self.i8_at(base + 0x17)?,
            unrest: self.u8_at(base + 0x20)?,
            population: self.i32_at(base + 0x24)?,
            pop_last: self.i32_at(base + 0x28)?,
            births: self.i32_at(base + 0x30)?,
            deaths: self.i32_at(base + 0x34)?,
            neighbour_count: self.u8_at(base + 0x5A)?,
            anchor_x: self.u8_at(base + 0x6C)?,
            anchor_y: self.u8_at(base + 0x6D)?,
            pop_band: self.u8_at(base + 0xB8)?,
            tax_rate: self.u8_at(base + 0xB9)?,
            tax_collected: self.i32_at(base + 0xBC)?,
            ration_achieved: self.i8_at(base + 0x15D)?,
            ration_wanted: self.i8_at(base + 0x15E)?,
            grain_eaten: self.i32_at(base + 0x178)?,
            herd_eaten: self.i32_at(base + 0x17C)?,
            castle_type: self.u8_at(base + 0x1C0)?,
            fields_fallow: self.u8_at(base + 0x1FF)?,
            fields_cattle: self.u8_at(base + 0x200)?,
            fields_grain: self.u8_at(base + 0x201)?,
            fertility: self.i32_at(base + 0x208)?,
            weather: self.u8_at(base + 0x21B)?,
            grain: self.i32_at(base + 0x224)?,
            herd: self.i32_at(base + 0x250)?,
        })
    }

    /// Every record, including the three that are never counties. Callers that
    /// want only the real ones filter on [`County::is_county`].
    pub fn counties(&self) -> Result<Vec<County>, SaveError> {
        (0..COUNTY_RECORDS).map(|i| self.county(i)).collect()
    }
}

/// A county as the shipped save holds it.
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
    pub shown_tax: i8,
    pub shown_ration: i8,
    pub shown_health: i8,
    pub shown_events: i8,
    pub unrest: u8,
    pub population: i32,
    pub pop_last: i32,
    pub births: i32,
    pub deaths: i32,
    pub neighbour_count: u8,
    pub anchor_x: u8,
    pub anchor_y: u8,
    pub pop_band: u8,
    pub tax_rate: u8,
    pub tax_collected: i32,
    pub ration_achieved: i8,
    pub ration_wanted: i8,
    pub grain_eaten: i32,
    pub herd_eaten: i32,
    pub castle_type: u8,
    pub fields_fallow: u8,
    pub fields_cattle: u8,
    pub fields_grain: u8,
    pub fertility: i32,
    pub weather: u8,
    pub grain: i32,
    pub herd: i32,
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
