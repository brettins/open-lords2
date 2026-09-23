//! `score.dat` — `FUN_0042D216` (read) and `FUN_0042D2DC` (write), around
//! `FUN_0042CC4A` in `FUN_0042C64F` (`00420000.c:4980-4982`).
//!
//! ```text
//! 0x51FD00 0xA0   top scores (10 i32), then DAT_0051FD28 mine (10 i32), then 0x50 of 0x51FD50
//! 0x51FBC0 0x140  ten 0x20-byte names
//! 0x53E9E0 4 rank; 0x568988 4 battles; 0x568214 4 total
//! ```
use super::*;
use std::path::{Path, PathBuf};

pub const FILE: &str = "score.dat";
pub const NAME_LEN: usize = 0x20;
/// `0xA0` read at `0x51FD00` runs past `DAT_0051FD28` into `0x51FD50`, the
/// `battles.eng` text buffer (`00420000.c:4268`). [V]
pub const SPILL: usize = 0xA0 - 2 * TOP * 4;
pub const SIZE: usize = 0xA0 + 0x140 + 12;

/// [D] Restore_WorkingDir puts it in the install; ours sits beside the saves (rule 2).
pub fn path() -> Option<PathBuf> {
    crate::saves::dir().map(|d| d.join(FILE))
}

pub fn encode(b: &BattleMaster) -> Vec<u8> {
    let mut out = Vec::with_capacity(SIZE);
    for i in 0..TOP {
        out.extend_from_slice(&b.top.get(i).map_or(0, |e| e.score).to_le_bytes());
    }
    for s in b.mine {
        out.extend_from_slice(&s.to_le_bytes());
    }
    // [D] The spill is battles.eng text in the original; we write zeros.
    out.resize(out.len() + SPILL, 0);
    for i in 0..TOP {
        let mut name = [0u8; NAME_LEN];
        let text = b.top.get(i).map_or("", |e| e.name.as_str());
        for (d, c) in name[..NAME_LEN - 1].iter_mut().zip(text.chars()) {
            *d = u8::try_from(u32::from(c)).unwrap_or(b'?');
        }
        out.extend_from_slice(&name);
    }
    for v in [b.rank as i32, b.battles, b.total] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

/// `FUN_0042D216`'s five `__read`s: each overwrites only what the file holds.
pub fn decode(bytes: &[u8], b: &mut BattleMaster) {
    let word = |at: usize| bytes.get(at..at + 4).map(|w| i32::from_le_bytes([w[0], w[1], w[2], w[3]]));
    b.top.resize(TOP, Entry::default());
    for i in 0..TOP {
        if let Some(s) = word(i * 4) {
            b.top[i].score = s;
        }
        if let Some(s) = word((TOP + i) * 4) {
            b.mine[i] = s;
        }
        if let Some(n) = bytes.get(0xA0 + i * NAME_LEN..0xA0 + (i + 1) * NAME_LEN) {
            b.top[i].name = n.iter().take_while(|&&c| c != 0).map(|&c| char::from(c)).collect();
        }
    }
    let tail = 0xA0 + 0x140;
    if let Some(v) = word(tail) {
        b.rank = v.max(0) as usize;
    }
    if let Some(v) = word(tail + 4) {
        b.battles = v;
    }
    if let Some(v) = word(tail + 8) {
        b.total = v;
    }
}

impl BattleMaster {
    /// `FUN_0042D216` (`0x0042D216`): no file zeroes rank, battles and total
    /// and leaves both tables standing. [V]
    pub fn read_score_dat(&mut self, path: &Path) {
        match std::fs::read(path) {
            Ok(bytes) => decode(&bytes, self),
            Err(_) => {
                self.rank = 0;
                self.battles = 0;
                self.total = 0;
            }
        }
    }

    /// `FUN_0042D2DC` (`0x0042D2DC`): `__open(…, 0x8301, 0x180)`, create and
    /// truncate; a failed open writes nothing. [V]
    pub fn write_score_dat(&self, path: &Path) {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(path, encode(self));
    }

    /// `FUN_0042C64F`'s tail, `00420000.c:4980-4982`: read, record, write.
    pub fn record_to_file(&mut self, path: Option<&Path>, local: &str, foe: &str, mine: i32, theirs: i32) {
        if let Some(p) = path {
            self.read_score_dat(p);
        }
        self.record(local, foe, mine, theirs);
        if let Some(p) = path {
            self.write_score_dat(p);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn book() -> BattleMaster {
        let mut b = BattleMaster::default();
        b.top = (0..TOP).map(|i| Entry { name: format!("Lord {i}"), score: 500 - i as i32 }).collect();
        b.mine[0] = 321;
        (b.rank, b.battles, b.total) = (3, 7, 2100);
        b
    }

    #[test]
    fn score_dat_is_the_five_reads_in_the_originals_order() {
        let bytes = encode(&book());
        assert_eq!(bytes.len(), 0xA0 + 0x140 + 12, "FUN_0042D2DC's five writes");
        assert_eq!(&bytes[0..4], &500i32.to_le_bytes(), "DAT_0051FD00 first");
        assert_eq!(&bytes[40..44], &321i32.to_le_bytes(), "DAT_0051FD28 inside the 0xA0");
        assert_eq!(&bytes[0xA0..0xA6], b"Lord 0", "names at +0xA0");
        assert_eq!(&bytes[0x1E0 + 4..0x1E0 + 8], &7i32.to_le_bytes(), "battles");
        let mut back = BattleMaster::default();
        decode(&bytes, &mut back);
        assert_eq!((back.top, back.mine, back.rank, back.battles, back.total),
            (book().top, book().mine, 3, 7, 2100));
    }

    #[test]
    fn a_missing_score_dat_zeroes_the_counters_and_keeps_the_tables() {
        let mut b = book();
        b.read_score_dat(Path::new("no/such/dir/score.dat"));
        assert_eq!((b.rank, b.battles, b.total), (0, 0, 0), "00420000.c FUN_0042D216 -1 limb");
        assert_eq!(b.top, book().top);
    }
}
