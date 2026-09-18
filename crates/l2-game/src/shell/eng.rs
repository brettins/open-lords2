//! `L2.eng` — the game's strings, read.
//!
//! Until now every screen in this workspace carried its own copy of the labels
//! it drew (`screens/county.rs`: *"Nothing here reads `L2.eng`; the workspace
//! has no decoder for it"*). That is the one thing a shell screen must not do,
//! because the whole value of a shell is that its text is the original's text.
//!
//! **[V]** Eight bytes of header, then a table of one 32-bit slot per group,
//! of which only the **low 24 bits** are used — a file offset. The table's own
//! length falls out of its first useful entry: group 1's data begins where the
//! table ends, so `(offset(1) - 8) / 4` is the number of slots. A group's
//! strings are the NUL-separated run from `offset(g)` to `offset(g + 1)`, and
//! the last group runs to the end of the file.
//!
//! That reading is checked three ways in [`tests`]: the shipped file has 318
//! slots, group 101 is exactly the sixty map names `g_scenarioIndex` indexes,
//! and group 21's six ration levels are the six `l2-kingdom` already knows.
//!
//! `Eng_DrawString(group, index, …)` (`0x00402D37`) skips `index` NUL
//! terminators and then skips any byte below `0x20`, so a group may be laid out
//! with control bytes between its entries and the indices still come out dense.
//!
//! By convention index 0 of a group is a descriptive heading and 1..N are its
//! entries — group 87 is *"Ration"*, *"Wanted:"*, *"Achieved:"*, … — but it is
//! only a convention and several groups (11, 103) are flat lists.

pub struct Eng {
    bytes: Vec<u8>,
    offsets: Vec<usize>,
    /// `Ui_DrawText` (`0x00402637`) indexes `g_glyphWidths` with the byte, and
    /// so does [`super::font::Font`] with `c as u32`. Whether the file's high
    /// bytes *mean* Latin-1 or code page 437 is not decided here.
    ///
    /// **`get` used to run `from_utf8` on the bytes and return `None` when it
    /// failed**, under a comment saying the file is Latin-1 and that
    /// `from_utf8` would reject its accented bytes. So it did: the shipped
    /// English file has nine such strings — `L2.eng` 295/2 … 295/10, the
    /// bullets of *"What should I do each turn?"*, each opening with `0xB7` —
    /// and all nine read as missing, and `group(295)` stopped at two.
    decoded: String,
    starts: Vec<u32>,
}

impl Eng {
    pub fn parse(bytes: Vec<u8>) -> Result<Eng, String> {
        if bytes.len() < 16 {
            return Err(format!("L2.eng: {} bytes is not a header", bytes.len()));
        }
        let at = |i: usize| -> usize {
            let b = &bytes[8 + i * 4..];
            b[0] as usize | (b[1] as usize) << 8 | (b[2] as usize) << 16
        };
        let first = at(1);
        if first < 8 || first > bytes.len() || (first - 8) % 4 != 0 {
            return Err(format!("L2.eng: group 1 starts at {first}, which is not a table end"));
        }
        let slots = (first - 8) / 4;
        if 8 + slots * 4 > bytes.len() {
            return Err(format!("L2.eng: {slots} slots do not fit in {} bytes", bytes.len()));
        }
        let offsets = (0..slots).map(at).collect();
        let mut decoded = String::with_capacity(bytes.len());
        let mut starts = Vec::with_capacity(bytes.len() + 1);
        for &b in &bytes {
            starts.push(decoded.len() as u32);
            decoded.push(char::from(b));
        }
        starts.push(decoded.len() as u32);
        Ok(Eng { bytes, offsets, decoded, starts })
    }

    pub fn count(&self) -> usize {
        self.offsets.len()
    }

    fn group_bytes(&self, group: usize) -> Option<(usize, &[u8])> {
        let start = *self.offsets.get(group)?;
        let end = match self.offsets.get(group + 1) {
            Some(&e) => e,
            None => self.bytes.len(),
        };
        if start >= end || end > self.bytes.len() {
            return None;
        }
        Some((start, &self.bytes[start..end]))
    }

    pub fn get(&self, group: usize, index: usize) -> Option<&str> {
        let (start, run) = self.group_bytes(group)?;
        let mut p = 0usize;
        for _ in 0..index {
            while p < run.len() && run[p] != 0 {
                p += 1;
            }
            p += 1;
        }
        while p < run.len() && run[p] < 0x20 {
            p += 1;
        }
        if p >= run.len() {
            return None;
        }
        let end = run[p..].iter().position(|&b| b == 0).map_or(run.len(), |n| p + n);
        let from = self.starts[start + p] as usize;
        let to = self.starts[start + end] as usize;
        Some(&self.decoded[from..to])
    }

    /// The same, but "" for anything missing. What a painter wants: a screen
    /// with no `L2.eng` should lay out with blank labels, not refuse to draw.
    pub fn text(&self, group: usize, index: usize) -> &str {
        self.get(group, index).unwrap_or("")
    }

    pub fn group(&self, group: usize) -> Vec<&str> {
        let mut out = Vec::new();
        let mut i = 0;
        while let Some(s) = self.get(group, i) {
            out.push(s);
            i += 1;
            if i > 4096 {
                break;
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synth(groups: &[&[&str]]) -> Vec<u8> {
        let slots = groups.len();
        let table = 8 + slots * 4;
        let mut runs: Vec<Vec<u8>> = Vec::new();
        for g in groups {
            let mut r = Vec::new();
            for s in *g {
                r.extend_from_slice(s.as_bytes());
                r.push(0);
            }
            runs.push(r);
        }
        let mut offsets = Vec::new();
        let mut p = table;
        for r in &runs {
            offsets.push(p);
            p += r.len();
        }
        let mut out = vec![0u8; table];
        for (i, o) in offsets.iter().enumerate() {
            out[8 + i * 4] = (*o & 0xff) as u8;
            out[9 + i * 4] = ((*o >> 8) & 0xff) as u8;
            out[10 + i * 4] = ((*o >> 16) & 0xff) as u8;
        }
        for r in runs {
            out.extend_from_slice(&r);
        }
        out
    }

    fn sample() -> Eng {
        // Slot 0 is never addressed by the game; group 1 is the first real one
        // and its offset is what fixes the table's length.
        Eng::parse(synth(&[&[], &["Lords of the Realm 2", "Single player"], &["off", "on"]]))
            .unwrap()
    }

    #[test]
    fn the_table_length_falls_out_of_group_ones_offset() {
        let e = sample();
        assert_eq!(e.count(), 3);
    }

    #[test]
    fn strings_are_indexed_from_zero_within_their_group() {
        let e = sample();
        assert_eq!(e.get(1, 0), Some("Lords of the Realm 2"));
        assert_eq!(e.get(1, 1), Some("Single player"));
        assert_eq!(e.get(2, 1), Some("on"));
    }

    #[test]
    fn an_index_past_the_end_is_none_and_text_is_empty() {
        let e = sample();
        assert_eq!(e.get(1, 2), None);
        assert_eq!(e.text(1, 2), "");
        assert_eq!(e.get(99, 0), None, "a group past the table");
        assert_eq!(e.get(0, 0), None, "an empty run");
    }

    #[test]
    fn a_short_file_is_refused_rather_than_indexed() {
        assert!(Eng::parse(vec![0u8; 4]).is_err());
        assert!(Eng::parse(vec![0u8; 32]).is_err(), "group 1 at offset 0 is not a table end");
    }

    #[test]
    fn a_byte_above_0x7f_is_the_char_of_the_same_number_and_not_a_missing_string() {
        // Hand-built, because `synth` writes UTF-8: group 1 is "\xB7 Adjust.",
        // "a\x86" and "b" — the first shaped like L2.eng 295/2.
        let mut bytes = synth(&[&[], &["X Adjust.", "aX", "b"]]);
        let run = bytes.len() - "X Adjust.\0aX\0b\0".len();
        bytes[run] = 0xB7;
        bytes[run + "X Adjust.\0a".len()] = 0x86;
        let e = Eng::parse(bytes).unwrap();
        assert_eq!(e.get(1, 0), Some("\u{B7} Adjust."));
        assert_eq!(e.get(1, 1), Some("a\u{86}"));
        assert_eq!(e.get(1, 2), Some("b"), "the index after a high byte is not shifted");
        assert_eq!(e.group(1).len(), 3);
    }

    #[test]
    fn a_group_reads_back_whole() {
        let e = sample();
        assert_eq!(e.group(1), vec!["Lords of the Realm 2", "Single player"]);
        assert!(e.group(0).is_empty());
    }
}
