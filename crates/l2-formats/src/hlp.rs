//! WinHelp `.hlp`, as far as `L2help.hlp`'s contents topic needs it. Format facts
//! from the public "Windows Help File Format" description (C5); no code taken.
//!
//! [V] measured on the install's `L2help.hlp` (2 980 023 bytes): `|SYSTEM` minor 33
//! (HCW 4.0), flags 0, so `|TOPIC` is 4 KiB blocks with no LZ77; no `|Phrases`, no
//! `|PhrIndex`; record 3 (contents) is TOPICOFFSET 0; no `.cnt` beside it, so
//! `Opt_GameHelpContents` (0x00434942, `WinHelpA(g_hwnd,"l2help.hlp",3,1)` 00430000.c:1831)
//! opens that topic with `HELP_CONTENTS`. 170 topics.
//! Not read: LinkData1 formatting (fonts, hotspots, jumps via `|CONTEXT`), bitmaps.

const MAGIC: i32 = 0x0003_5F3F;
const BLOCK: usize = 4096;
const BLOCK_HEADER: usize = 12;
/// TOPICPOS = block * 0x4000 + offset in the block's decompressed bytes.
const POS_BLOCK: u32 = 0x4000;
const TL_TOPIC_HEADER: u8 = 2;
const TL_DISPLAY: u8 = 0x20;
const TL_TABLE: u8 = 0x23;

pub struct Hlp {
    pub title: String,
    /// `|SYSTEM` record 3, the topic `HELP_CONTENTS` opens.
    pub contents: u32,
    stream: Vec<u8>,
    /// Each `|TOPIC` block header's FirstTopicLink (+4), a TOPICPOS.
    first_links: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Topic {
    pub title: String,
    /// One per text segment of the topic's display records; a segment ends
    /// where LinkData1 has a formatting command [I] (unread: see the module).
    pub lines: Vec<String>,
}

fn u16_at(b: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(o..o + 2)?.try_into().ok()?))
}
fn i16_at(b: &[u8], o: usize) -> Option<i16> {
    u16_at(b, o).map(|v| v as i16)
}
fn i32_at(b: &[u8], o: usize) -> Option<i32> {
    Some(i32::from_le_bytes(b.get(o..o + 4)?.try_into().ok()?))
}
fn latin1(b: &[u8]) -> String {
    b.iter().map(|&c| c as char).collect()
}

/// Internal file body: FILEHEADER is ReservedSpace i32, UsedSpace i32, flags u8.
fn body(b: &[u8], at: usize) -> Option<&[u8]> {
    let used = i32_at(b, at + 4)? as usize;
    b.get(at + 9..at + 9 + used)
}

/// The directory B+ tree's leaves: `STRINGZ name, i32 offset`. BTREEHEADER is 38
/// bytes (PageSize +4, RootPage +26, NLevels +32); an index page's first child is
/// at +4, a leaf's entry count at +2 and next page at +6, entries from +8.
fn directory(b: &[u8]) -> Option<Vec<(String, usize)>> {
    let tree = body(b, i32_at(b, 4)? as usize)?;
    let page_size = u16_at(tree, 4)? as usize;
    let mut page = i16_at(tree, 26)?;
    let pages = 38;
    for _ in 1..i16_at(tree, 32)? {
        page = i16_at(tree, pages + page as usize * page_size + 4)?;
    }
    let mut out = Vec::new();
    while page >= 0 {
        let p = pages + page as usize * page_size;
        let mut o = p + 8;
        for _ in 0..i16_at(tree, p + 2)? {
            let end = o + tree.get(o..)?.iter().position(|&c| c == 0)?;
            out.push((latin1(&tree[o..end]), i32_at(tree, end + 1)? as usize));
            o = end + 5;
        }
        page = i16_at(tree, p + 6)?;
    }
    Some(out)
}

impl Hlp {
    /// `None` for anything outside what `L2help.hlp` uses: another magic, an
    /// old `|SYSTEM` (minor <= 16), compression, or phrase tables.
    pub fn parse(b: &[u8]) -> Option<Hlp> {
        if i32_at(b, 0)? != MAGIC {
            return None;
        }
        let dir = directory(b)?;
        let find = |name: &str| dir.iter().find(|(n, _)| n == name).map(|&(_, o)| o);
        if find("|Phrases").is_some() || find("|PhrIndex").is_some() {
            return None;
        }
        let sys = body(b, find("|SYSTEM")?)?;
        let (minor, flags) = (u16_at(sys, 2)?, u16_at(sys, 10)?);
        if minor <= 16 || flags != 0 {
            return None;
        }
        let (mut title, mut contents, mut o) = (String::new(), 0, 12);
        while o + 4 <= sys.len() {
            let (kind, n) = (u16_at(sys, o)?, u16_at(sys, o + 2)? as usize);
            let data = sys.get(o + 4..o + 4 + n)?;
            match kind {
                1 => title = latin1(data.split(|&c| c == 0).next()?),
                3 => contents = i32_at(data, 0)? as u32,
                _ => {}
            }
            o += 4 + n;
        }
        let topic = body(b, find("|TOPIC")?)?;
        let stream = topic.chunks(BLOCK).flat_map(|c| c.get(BLOCK_HEADER..).unwrap_or(&[])).copied().collect();
        let first_links = topic.chunks(BLOCK).filter_map(|c| i32_at(c, 4)).map(|v| v as u32).collect();
        Some(Hlp { title, contents, stream, first_links })
    }

    /// A TOPICOFFSET is block * 0x8000 + characters into it; only a block's
    /// first topic (characters 0) is found here, which record 3's 0 is [V].
    pub fn topic_at_offset(&self, offset: u32) -> Option<Topic> {
        if offset % 0x8000 != 0 {
            return None;
        }
        self.topic(*self.first_links.get((offset / 0x8000) as usize)?)
    }

    fn index(pos: u32) -> Option<usize> {
        let off = (pos % POS_BLOCK) as usize;
        Some((pos / POS_BLOCK) as usize * (BLOCK - BLOCK_HEADER) + off.checked_sub(BLOCK_HEADER)?)
    }

    /// TOPICLINK: BlockSize +0, DataLen2 +4, NextBlock +12 (a TOPICPOS), DataLen1
    /// +16, RecordType +20; LinkData2 runs from DataLen1 to BlockSize, NUL-split.
    pub fn topic(&self, pos: u32) -> Option<Topic> {
        let s = &self.stream;
        let (mut pos, mut title, mut lines) = (pos, None, Vec::new());
        loop {
            let at = Self::index(pos)?;
            let (size, next, len1) = (i32_at(s, at)? as usize, i32_at(s, at + 12)?, i32_at(s, at + 16)? as usize);
            let data2 = s.get(at + len1..at + size)?;
            match *s.get(at + 20)? {
                TL_TOPIC_HEADER if title.is_some() => break,
                TL_TOPIC_HEADER => title = Some(latin1(data2.split(|&c| c == 0).next()?)),
                TL_DISPLAY | TL_TABLE => lines.extend(
                    data2.split(|&c| c == 0).map(latin1).filter(|t| !t.trim().is_empty()),
                ),
                _ => {}
            }
            if next < 0 || next as u32 <= pos {
                break;
            }
            pos = next as u32;
        }
        Some(Topic { title: title?, lines })
    }
}
