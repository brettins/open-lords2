//! `BATTLES.ENG` — the skirmish page's own vocabulary, and not `L2.eng`.
//!
//! `FUN_0042AA90` (`0x0042AA90`, `00420000.c:4248-4263`) reads the whole file
//! into `DAT_0051FD50` and does one thing to it:
//!
//! ```c
//! for (i = 0; i < len; i++) if ((byte)buf[i] < 0x20) buf[i] = 0;
//! ```
//!
//! `FUN_0042AB15(n)` (`0x0042AB15`, `00420000.c:4275-4285`) then walks that
//! buffer counting `*p == 0 && p[-1] != 0` — the end of each **non-empty** run
//! — and skips every byte `< 0x20` before returning. So entry *n* is the
//! *n*'th non-empty line, blank lines and all, and the file is triples.
//! `docs/formats/eng.md` §2.

/// `(DAT_0053E91C * 10 + row) * 3` — the index `FUN_00421005`
/// (`00420000.c:396`) and `FUN_00421231` (`00420000.c:432`, `:440`) build.
/// **Ten**, not the row count: category 2 has fifteen rows and reaches entries
/// 20…34 all the same, and entries 35…56 are unreachable from this page.
pub const ROWS_PER_CATEGORY: usize = 10;

/// name, then title, then description — `+0`, `+1`, `+2` on the same index.
pub const FIELDS: usize = 3;

/// The three fields of one battle, in the order the file holds them and with
/// the widths the matching `.skr` record uses (13, 29, 141 —
/// [`crate::skr::ScenarioText`]).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BattleText {
    /// The list row, `FUN_00421005` at `(0x1D4, 0xBA + 16n)`.
    pub name: String,
    /// The heading `FUN_00421231` draws at `(0x78, 0x179)` in `g_fontHeading`.
    pub title: String,
    /// The two-line body at `(0x98, 0x172)`.
    pub description: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Battles {
    entries: Vec<String>,
}

impl Battles {
    /// `FUN_0042AA90` and the seek in `FUN_0042AB15` together: every maximal
    /// run of bytes `>= 0x20`, in order.
    ///
    /// Nothing is trimmed. Six of the shipped file's descriptions end in
    /// spaces and `Ui_DrawCentred` measures them, so trimming would move text
    /// the original leaves where it is.
    pub fn parse(bytes: &[u8]) -> Battles {
        let entries = bytes
            .split(|&b| b < 0x20)
            .filter(|run| !run.is_empty())
            .map(|run| String::from_utf8_lossy(run).into_owned())
            .collect();
        Battles { entries }
    }

    /// `FUN_004AF70C` returning 0 — no file, and `FUN_0042AA90` returns 0.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// How many whole triples the file holds — 57 on a shipped install.
    pub fn count(&self) -> usize {
        self.entries.len() / FIELDS
    }

    /// One entry, or `""`. The original walks off the end of the buffer
    /// instead; we stop.
    pub fn entry(&self, n: usize) -> &str {
        self.entries.get(n).map_or("", String::as_str)
    }

    pub fn index(kind: usize, row: usize) -> usize {
        (kind * ROWS_PER_CATEGORY + row) * FIELDS
    }

    pub fn at(&self, kind: usize, row: usize) -> BattleText {
        let i = Battles::index(kind, row);
        BattleText {
            name: self.entry(i).to_string(),
            title: self.entry(i + 1).to_string(),
            description: self.entry(i + 2).to_string(),
        }
    }

    /// The short name alone, which is all the six list rows draw.
    pub fn name(&self, kind: usize, row: usize) -> &str {
        self.entry(Battles::index(kind, row))
    }

    /// Every third entry, trimmed — the slug source, not a drawn string.
    pub fn names(&self) -> Vec<String> {
        self.entries.chunks(FIELDS).filter(|c| c.len() == FIELDS).map(|c| c[0].trim().to_string()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_crlf_file_splits_into_non_empty_runs() {
        let b = Battles::parse(b"Three Bridges\r\nA Bridge Too Far\r\n\r\nYou may be sure.  \r\n");
        assert_eq!(b.len(), 3);
        assert_eq!(b.entry(0), "Three Bridges");
        assert_eq!(b.entry(1), "A Bridge Too Far");
        // FUN_0042AB15 skips the blank line rather than counting it.
        assert_eq!(b.entry(2), "You may be sure.  ");
        assert_eq!(b.entry(3), "");
    }

    /// The `* 10` of `(DAT_0053E91C * 10 + row) * 3`: category 2 row 0 is
    /// entry 20, not entry 30, and the fifteen castles run to entry 34.
    #[test]
    fn the_category_stride_is_ten_rows_and_not_the_row_count() {
        assert_eq!(Battles::index(0, 0), 0);
        assert_eq!(Battles::index(1, 0), 30);
        assert_eq!(Battles::index(2, 0), 60);
        assert_eq!(Battles::index(2, 14), 102);
        assert_eq!(Battles::index(2, 14) / FIELDS, 34);
    }

    #[test]
    fn missing_file_answers_empty_strings() {
        let b = Battles::parse(&[]);
        assert!(b.is_empty());
        assert_eq!(b.name(0, 0), "");
        assert_eq!(b.at(2, 9), BattleText::default());
    }
}
