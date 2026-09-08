//! Just enough PE to turn one of `Lords2.exe`'s virtual addresses into a file
//! offset.
//!
//! The game has no ASLR and a fixed image base of `0x400000`, so a virtual
//! address written in `docs/symbols.md` is a constant and the bytes behind it
//! can be read straight off disk — no process, no window, nothing a screen lock
//! can spoil (`docs/decisions.md` C16).
//!
//! This was written three times: once in `l2-formats`'s save reader, once in
//! `l2-view/tests/install.rs`, and once in each of the four PowerShell scripts
//! under `tools/oracle`. The Rust copies are now this one, so an oracle test in
//! any crate is four lines rather than forty and there is no reason left not to
//! write one.

/// `Lords2.exe`'s fixed image base.
pub const IMAGE_BASE: u32 = 0x0040_0000;

/// File offset of a virtual address, or `None` when no section covers it.
///
/// Uses the **raw** size rather than the virtual size, so an address in
/// uninitialised `.data` returns `None` instead of an offset into whatever
/// follows it in the file. That distinction is C14/C16: the bytes for a runtime
/// constant are not in the image, and a reader that confidently returns zero for
/// them is worse than one that admits it cannot answer.
pub fn va_to_offset(exe: &[u8], va: u32) -> Option<usize> {
    let pe = u32::from_le_bytes(exe.get(0x3C..0x40)?.try_into().ok()?) as usize;
    if u32::from_le_bytes(exe.get(pe..pe + 4)?.try_into().ok()?) != 0x0000_4550 {
        return None;
    }
    let sections = u16::from_le_bytes(exe.get(pe + 6..pe + 8)?.try_into().ok()?) as usize;
    let opt_size = u16::from_le_bytes(exe.get(pe + 20..pe + 22)?.try_into().ok()?) as usize;
    for i in 0..sections {
        let s = pe + 24 + opt_size + i * 40;
        let rva = u32::from_le_bytes(exe.get(s + 12..s + 16)?.try_into().ok()?);
        let raw_size = u32::from_le_bytes(exe.get(s + 16..s + 20)?.try_into().ok()?);
        let raw_ptr = u32::from_le_bytes(exe.get(s + 20..s + 24)?.try_into().ok()?);
        let start = IMAGE_BASE + rva;
        if va >= start && va < start + raw_size {
            return Some((raw_ptr + (va - start)) as usize);
        }
    }
    None
}

/// A table of fixed-width little-endian integers, read out of the image.
///
/// Panics with the address in the message when the table is not in an
/// initialised section, because at that point the test's premise is wrong and
/// continuing would compare against zeros.
pub struct Table<'a> {
    exe: &'a [u8],
    base: usize,
    va: u32,
}

impl<'a> Table<'a> {
    pub fn at(exe: &'a [u8], va: u32) -> Table<'a> {
        let base = va_to_offset(exe, va).unwrap_or_else(|| {
            panic!("{va:#010X} is in no initialised section of Lords2.exe")
        });
        Table { exe, base, va }
    }

    /// Address of this table, for failure messages.
    pub fn va(&self) -> u32 {
        self.va
    }

    pub fn i32_at(&self, index: usize) -> i32 {
        let o = self.base + index * 4;
        i32::from_le_bytes(self.exe[o..o + 4].try_into().unwrap())
    }

    pub fn u16_at(&self, index: usize) -> u16 {
        let o = self.base + index * 2;
        u16::from_le_bytes(self.exe[o..o + 2].try_into().unwrap())
    }

    pub fn u8_at(&self, index: usize) -> u8 {
        self.exe[self.base + index]
    }

    /// `count` consecutive `i32`s.
    pub fn i32s(&self, count: usize) -> Vec<i32> {
        (0..count).map(|i| self.i32_at(i)).collect()
    }

    /// Column `col` of a `rows` x `cols` table of `i32`, row-major — the shape
    /// `g_troopBattleStats` and friends are stored in.
    pub fn column(&self, rows: usize, cols: usize, col: usize) -> Vec<i32> {
        (0..rows).map(|r| self.i32_at(r * cols + col)).collect()
    }
}
