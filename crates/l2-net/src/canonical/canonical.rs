#![allow(unused_imports)]
use super::*;
use super::reader::*;
use super::impls::*;
use crate::fixed::Fixed;
use crate::hash::XxHash64;

impl Digest {
    /// The section whose hash differs from `other`'s, if exactly one
    /// does.
    ///
    /// Returns `None` both when the states agree and when several
    /// sections differ, because those two cases lead to different
    /// questions: nothing to explain, versus a divergence that has
    /// already spread. §6's advice applies — if the PRNG section is the
    /// only one that differs, someone drew a random number outside the
    /// simulation; if it differs along with everything else, it is a
/// consequence.
    pub fn sole_difference(&self, other: &Digest) -> Option<&'static str> {
        if self.sections.len() != other.sections.len() {
            return None;
        }
        let mut found = None;
        for (a, b) in self.sections.iter().zip(&other.sections) {
            if a.name != b.name {
                return None;
            }
            if a.hash != b.hash || a.len != b.len {
                if found.is_some() {
                    return None;
                }
                found = Some(a.name);
            }
        }
        found
    }

    /// Every section that differs, in encode order.
    pub fn differences<'a>(&'a self, other: &'a Digest) -> Vec<&'static str> {
        let mut out = Vec::new();
        for a in &self.sections {
            match other.sections.iter().find(|b| b.name == a.name) {
                Some(b) if b.hash == a.hash && b.len == a.len => {}
                _ => out.push(a.name),
            }
        }
        out
    }
}

impl Canonical {
    /// Hash without keeping the bytes. The per-tick checksum path — no
    /// allocation at all once the section vector has grown.
    pub fn hashing() -> Canonical {
        Canonical {
            root: XxHash64::with_seed(CHECKSUM_SEED),
            open: None,
            sections: Vec::new(),
            bytes: None,
        }
    }

    /// Hash *and* keep the bytes. The snapshot and desync-dump path.
    pub fn recording() -> Canonical {
        let mut c = Canonical::hashing();
        c.bytes = Some(Vec::new());
        c
    }

    /// Total bytes written so far.
    pub fn len(&self) -> u64 {
        self.root.len()
    }

    pub fn is_empty(&self) -> bool {
        self.root.len() == 0
    }

    /// Begin a named section, ending any section already open.
    ///
    /// Sections do not nest. Flat is what §6's localisation wants — a
    /// list of subsystems, one of which is the culprit — and nesting
    /// would raise the question of whether an inner section's bytes
    /// belong to the outer one, which is a question with no useful
    /// answer.
    pub fn section(&mut self, name: &'static str) {
        self.end_section();
        self.open = Some((name, XxHash64::with_seed(CHECKSUM_SEED)));
    }

    /// End the current section, if any. Called automatically by
    /// [`Canonical::section`] and [`Canonical::finish`].
    pub fn end_section(&mut self) {
        if let Some((name, hasher)) = self.open.take() {
            self.sections.push(SectionDigest {
                name,
                hash: hasher.finish(),
                len: hasher.len(),
            });
        }
    }

    /// Raw bytes, with **no** length prefix.
    ///
    /// The escape hatch,
    /// encoding ambiguous. Use it for fixed-size arrays whose length is
    /// part of the schema (an 80×80 terrain grid); use
    /// [`Canonical::bytes`] for anything whose length can vary.
    pub fn raw(&mut self, bytes: &[u8]) {
        self.root.write(bytes);
        if let Some((_, hasher)) = &mut self.open {
            hasher.write(bytes);
        }
        if let Some(buf) = &mut self.bytes {
            buf.extend_from_slice(bytes);
        }
    }

    pub fn u8(&mut self, v: u8) {
        self.raw(&[v]);
    }

    pub fn u16(&mut self, v: u16) {
        self.raw(&v.to_le_bytes());
    }

    pub fn u32(&mut self, v: u32) {
        self.raw(&v.to_le_bytes());
    }

    pub fn u64(&mut self, v: u64) {
        self.raw(&v.to_le_bytes());
    }

    pub fn i8(&mut self, v: i8) {
        self.raw(&v.to_le_bytes());
    }

    pub fn i16(&mut self, v: i16) {
        self.raw(&v.to_le_bytes());
    }

    pub fn i32(&mut self, v: i32) {
        self.raw(&v.to_le_bytes());
    }

    pub fn i64(&mut self, v: i64) {
        self.raw(&v.to_le_bytes());
    }

    /// One byte, `0` or `1`. Never the host's `bool` representation,
    pub fn bool(&mut self, v: bool) {
        self.u8(u8::from(v));
    }

    /// A [`Fixed`] as its raw `i32`. The value, not a rendering of it.
    pub fn fixed(&mut self, v: Fixed) {
        self.i32(v.raw());
    }

    /// A count, as `u32`.
    ///
    /// Panics above `u32::MAX`. Truncating silently would be the
    /// classic serialisation bug — a length that no longer matches its
    /// data — and this game has no collection that can approach 4
    /// billion entries,
    /// wildly wrong call, both of which want to stop immediately.
    pub fn len32(&mut self, n: usize) {
        assert!(n <= u32::MAX as usize, "canonical length {n} exceeds u32");
        self.u32(n as u32);
    }

    /// Length-prefixed bytes.
    pub fn bytes(&mut self, v: &[u8]) {
        self.len32(v.len());
        self.raw(v);
    }

    /// Length-prefixed UTF-8.
    ///
    /// Strings are rare in simulation state and common in handshakes.
    /// The length is in *bytes*, not characters, because that is the
    /// only definition two implementations cannot disagree about.
    pub fn str(&mut self, v: &str) {
        self.bytes(v.as_bytes());
    }

    /// A length-prefixed sequence, written by a closure per item.
    ///
/// Takes a slice on purpose: an iterator
    /// over a `HashMap` would compile, and D-4 exists to make sure that
    /// never happens. A slice has one order and it is the one in
    /// memory.
    pub fn seq<T>(&mut self, items: &[T], mut each: impl FnMut(&mut Canonical, &T)) {
        self.len32(items.len());
        for item in items {
            each(self, item);
        }
    }

    /// A value that may be absent: a tag byte, then the value.
    pub fn option<T>(&mut self, value: Option<&T>, each: impl FnOnce(&mut Canonical, &T)) {
        match value {
            None => self.u8(0),
            Some(v) => {
                self.u8(1);
                each(self, v);
            }
        }
    }

    pub fn encode<T: Encode>(&mut self, value: &T) {
        value.encode(self);
    }

    /// Close the encoder and produce the digest.
    pub fn finish(mut self) -> Digest {
        self.end_section();
        Digest {
            hash: self.root.finish(),
            len: self.root.len(),
            sections: self.sections,
            bytes: self.bytes,
        }
    }

    /// The checksum of one encodable value, with nothing kept.
    pub fn hash_of<T: Encode>(value: &T) -> u64 {
        let mut c = Canonical::hashing();
        value.encode(&mut c);
        c.finish().hash
    }

    /// The canonical bytes of one encodable value.
    pub fn bytes_of<T: Encode>(value: &T) -> Vec<u8> {
        let mut c = Canonical::recording();
        value.encode(&mut c);
        c.finish().bytes.expect("recording encoder keeps its bytes")
    }
}

