//! WinHelp `.hlp`, as far as `L2help.hlp` needs it. Format facts from the public
//! "Windows Help File Format" description (C5); no code taken.
//!
//! [V] measured on the install's `L2help.hlp` (2 980 023 bytes): `|SYSTEM` minor 33
//! (HCW 4.0), flags 0, so `|TOPIC` is 4 KiB blocks with no LZ77; no `|Phrases`, no
//! `|PhrIndex`; record 3 (contents) is TOPICOFFSET 0; no `.cnt` beside it, so
//! `Opt_GameHelpContents` 0x00434942's `HELP_CONTENTS` (WinHelpA(...,3,1), 00430000.c:1831) opens that topic. 170 topics.
//! [V] LinkData1 of all 694 display records parses to exactly DataLen1 with the
//! commands below; commands seen: 0x80 0x82 0x83 0x86 0x87 0x89 0xE2 (57) 0xE3 (446).
//! [V] all 168 `|CONTEXT` offsets equal a topic header's TOPICOFFSET as [`Hlp::parse`]
//! counts it. Not read: fonts, bitmaps, `|KWBTREE`.

use self::rd::Rd;

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
    /// `|CONTEXT` leaves: (hash, TOPICOFFSET).
    context: Vec<(u32, u32)>,
    /// Every topic header: (TOPICOFFSET, TOPICPOS).
    topics: Vec<(u32, u32)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    /// The `|CONTEXT` hash of a 0xE2 (popup) or 0xE3 (jump) hotspot, which runs to 0x89.
    pub jump: Option<u32>,
}

/// One paragraph (0x82) or line break (0x81).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Line {
    pub spans: Vec<Span>,
}

impl Line {
    pub fn text(&self) -> String {
        self.spans.iter().map(|s| s.text.as_str()).collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Topic {
    pub title: String,
    pub lines: Vec<Line>,
}

enum Cmd {
    Break,
    Tab,
    Jump(u32),
    EndHotspot,
    Other,
}

mod rd {
    /// The format's compressed integers: low bit 0 means the short form.
    pub struct Rd<'a> {
        pub b: &'a [u8],
        pub o: usize,
    }
    impl Rd<'_> {
        pub fn u8(&mut self) -> Option<u8> {
            let v = *self.b.get(self.o)?;
            self.o += 1;
            Some(v)
        }
        pub fn u16(&mut self) -> Option<u16> {
            let v = super::u16_at(self.b, self.o)?;
            self.o += 2;
            Some(v)
        }
        pub fn u32(&mut self) -> Option<u32> {
            let v = super::i32_at(self.b, self.o)? as u32;
            self.o += 4;
            Some(v)
        }
        fn long(&self) -> Option<bool> {
            Some(self.b.get(self.o)? & 1 == 1)
        }
        /// Compressed unsigned short: 1 or 2 bytes, value >> 1.
        pub fn cus(&mut self) -> Option<u32> {
            Some(if self.long()? { u32::from(self.u16()? >> 1) } else { u32::from(self.u8()? >> 1) })
        }
        /// Compressed signed short / long: the same, biased by half the range.
        pub fn css(&mut self) -> Option<i32> {
            Some(if self.long()? { i32::from(self.u16()? >> 1) - 0x4000 } else { i32::from(self.u8()? >> 1) - 0x40 })
        }
        pub fn csl(&mut self) -> Option<i64> {
            Some(if self.long()? { i64::from(self.u32()? >> 1) - 0x4000_0000 } else { i64::from(self.u16()? >> 1) - 0x4000 })
        }
        pub fn skip(&mut self, n: usize) {
            self.o += n;
        }
    }
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

/// A B+ tree's leaves in order. BTREEHEADER is 38 bytes (PageSize +4, RootPage
/// +26, NLevels +32); an index page's first child is at +4, a leaf's entry count
/// at +2 and next page at +6, entries from +8. `entry` returns the next entry's offset.
fn leaves(tree: &[u8], mut entry: impl FnMut(usize) -> Option<usize>) -> Option<()> {
    let page_size = u16_at(tree, 4)? as usize;
    let mut page = i16_at(tree, 26)?;
    for _ in 1..i16_at(tree, 32)? {
        page = i16_at(tree, 38 + page as usize * page_size + 4)?;
    }
    while page >= 0 {
        let p = 38 + page as usize * page_size;
        let mut o = p + 8;
        for _ in 0..i16_at(tree, p + 2)? {
            o = entry(o)?;
        }
        page = i16_at(tree, p + 6)?;
    }
    Some(())
}

/// The directory's leaves: `STRINGZ name, i32 offset`.
fn directory(b: &[u8]) -> Option<Vec<(String, usize)>> {
    let tree = body(b, i32_at(b, 4)? as usize)?;
    let mut out = Vec::new();
    leaves(tree, |o| {
        let end = o + tree.get(o..)?.iter().position(|&c| c == 0)?;
        out.push((latin1(&tree[o..end]), i32_at(tree, end + 1)? as usize));
        Some(end + 5)
    })?;
    Some(out)
}

/// LinkData1 of a 0x20/0x23 record: TopicSize (csl), TopicLength (cus), for 0x23
/// the column table, then per paragraph (per cell, `i16 column` first, -1 ends) two
/// bytes, `u16 id`, `u16 bits` and the fields `bits` names, then commands to 0xFF.
/// Returns TopicLength and the commands; a table cell's 0xFF is a [`Cmd::Break`].
fn link_data1(b: &[u8], table: bool) -> Option<(u32, Vec<Cmd>)> {
    let mut r = Rd { b, o: 0 };
    r.csl()?;
    let length = r.cus()?;
    if table {
        let cols = r.u8()? as usize;
        if matches!(r.u8()?, 0 | 2) {
            r.skip(2);
        }
        r.skip(cols * 4);
    }
    let mut cmds = Vec::new();
    loop {
        if table && r.u16()? as i16 == -1 {
            break;
        }
        if table {
            r.skip(3);
        }
        r.skip(4);
        let bits = r.u16()?;
        if bits & 1 != 0 {
            r.csl()?;
        }
        for m in [2, 4, 8, 0x10, 0x20, 0x40] {
            if bits & m != 0 {
                r.css()?;
            }
        }
        if bits & 0x100 != 0 {
            r.skip(3);
        }
        if bits & 0x200 != 0 {
            for _ in 0..r.css()? {
                if r.cus()? & 0x4000 != 0 {
                    r.cus()?;
                }
            }
        }
        loop {
            let c = r.u8()?;
            cmds.push(match c {
                0xFF => Cmd::Break,
                0x81 | 0x82 => Cmd::Break,
                0x83 => Cmd::Tab,
                0x89 => Cmd::EndHotspot,
                0x8B | 0x8C => Cmd::Other,
                0x20 => {
                    r.skip(4);
                    Cmd::Other
                }
                0x21 | 0x80 => {
                    r.skip(2);
                    Cmd::Other
                }
                0x86..=0x88 => {
                    let kind = r.u8()?;
                    let size = r.csl()? as usize;
                    if kind == 0x22 {
                        r.cus()?;
                    }
                    r.skip(size);
                    Cmd::Other
                }
                0xC8 | 0xCC | 0xEA | 0xEB | 0xEE | 0xEF => {
                    let n = r.u16()? as usize;
                    r.skip(n);
                    Cmd::Other
                }
                0xE0 | 0xE1 => {
                    r.skip(4);
                    Cmd::Other
                }
                0xE2 | 0xE3 | 0xE6 | 0xE7 => Cmd::Jump(r.u32()?),
                _ => return None,
            });
            if c == 0xFF {
                break;
            }
        }
        if !table {
            cmds.pop();
            cmds.push(Cmd::Other);
            break;
        }
    }
    (r.o == b.len()).then_some((length, cmds))
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
        let ctx = body(b, find("|CONTEXT")?)?;
        let mut context = Vec::new();
        leaves(ctx, |o| {
            context.push((i32_at(ctx, o)? as u32, i32_at(ctx, o + 4)? as u32));
            Some(o + 8)
        })?;
        let topic = body(b, find("|TOPIC")?)?;
        let stream = topic.chunks(BLOCK).flat_map(|c| c.get(BLOCK_HEADER..).unwrap_or(&[])).copied().collect();
        let mut hlp = Hlp { title, contents, stream, context, topics: Vec::new() };
        hlp.topics = hlp.headers(i32_at(topic, 4)? as u32)?;
        Some(hlp)
    }

    /// TOPICOFFSET = the |TOPIC block a record starts in * 0x8000 + the TopicLengths
    /// of the display records before it in that block.
    fn headers(&self, first: u32) -> Option<Vec<(u32, u32)>> {
        let (mut pos, mut block, mut count, mut out) = (first, u32::MAX, 0, Vec::new());
        loop {
            let (kind, next, data1, _) = self.record(pos)?;
            if pos / POS_BLOCK != block {
                (block, count) = (pos / POS_BLOCK, 0);
            }
            match kind {
                TL_TOPIC_HEADER => out.push((block * 0x8000 + count, pos)),
                TL_DISPLAY | TL_TABLE => count += link_data1(data1, kind == TL_TABLE)?.0,
                _ => {}
            }
            if next < 0 || next as u32 <= pos {
                return Some(out);
            }
            pos = next as u32;
        }
    }

    fn index(pos: u32) -> Option<usize> {
        let off = (pos % POS_BLOCK) as usize;
        Some((pos / POS_BLOCK) as usize * (BLOCK - BLOCK_HEADER) + off.checked_sub(BLOCK_HEADER)?)
    }

    /// TOPICLINK: BlockSize +0, DataLen2 +4, NextBlock +12 (a TOPICPOS), DataLen1
    /// +16, RecordType +20; LinkData1 from +21, LinkData2 from DataLen1 to BlockSize.
    fn record(&self, pos: u32) -> Option<(u8, i32, &[u8], &[u8])> {
        let (s, at) = (&self.stream, Self::index(pos)?);
        let (size, next, len1) = (i32_at(s, at)? as usize, i32_at(s, at + 12)?, i32_at(s, at + 16)? as usize);
        Some((*s.get(at + 20)?, next, s.get(at + 21..at + len1)?, s.get(at + len1..at + size)?))
    }

    pub fn topic_count(&self) -> usize {
        self.topics.len()
    }

    /// A hotspot's hash through `|CONTEXT` to a TOPICOFFSET.
    pub fn jump(&self, hash: u32) -> Option<u32> {
        self.context.iter().find(|&&(h, _)| h == hash).map(|&(_, o)| o)
    }

    pub fn topic_at_offset(&self, offset: u32) -> Option<Topic> {
        let &(_, pos) = self.topics.iter().find(|&&(o, _)| o == offset)?;
        self.topic(pos)
    }

    /// LinkData2 is NUL-split and segment i is followed by command i. Blank lines dropped.
    fn topic(&self, mut pos: u32) -> Option<Topic> {
        let (mut title, mut lines, mut line) = (None, Vec::new(), Line::default());
        let mut span = Span { text: String::new(), jump: None };
        let flush = |line: &mut Line, span: &mut Span, jump| {
            let done = std::mem::replace(span, Span { text: String::new(), jump });
            if !done.text.is_empty() {
                line.spans.push(done);
            }
        };
        loop {
            let (kind, next, data1, data2) = self.record(pos)?;
            match kind {
                TL_TOPIC_HEADER if title.is_some() => break,
                TL_TOPIC_HEADER => title = Some(latin1(data2.split(|&c| c == 0).next()?)),
                TL_DISPLAY | TL_TABLE => {
                    let (_, cmds) = link_data1(data1, kind == TL_TABLE)?;
                    let mut cmds = cmds.into_iter();
                    for seg in data2.split(|&c| c == 0) {
                        span.text.push_str(&latin1(seg));
                        match cmds.next() {
                            Some(Cmd::Break) => {
                                let jump = span.jump;
                                flush(&mut line, &mut span, jump);
                                let done = std::mem::take(&mut line);
                                if !done.text().trim().is_empty() {
                                    lines.push(done);
                                }
                            }
                            Some(Cmd::Tab) => span.text.push(' '),
                            Some(Cmd::Jump(h)) => flush(&mut line, &mut span, Some(h)),
                            Some(Cmd::EndHotspot) => flush(&mut line, &mut span, None),
                            Some(Cmd::Other) | None => {}
                        }
                    }
                }
                _ => {}
            }
            if next < 0 || next as u32 <= pos {
                break;
            }
            pos = next as u32;
        }
        flush(&mut line, &mut span, None);
        if !line.text().trim().is_empty() {
            lines.push(line);
        }
        Some(Topic { title: title?, lines })
    }
}
