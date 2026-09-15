#![allow(unused_imports)]
use super::*;
use super::reader::*;
use super::impls::*;
use crate::fixed::Fixed;
use crate::hash::XxHash64;

impl Digest {
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
    pub fn hashing() -> Canonical {
        Canonical {
            root: XxHash64::with_seed(CHECKSUM_SEED),
            open: None,
            sections: Vec::new(),
            bytes: None,
        }
    }

    pub fn recording() -> Canonical {
        let mut c = Canonical::hashing();
        c.bytes = Some(Vec::new());
        c
    }

    pub fn len(&self) -> u64 {
        self.root.len()
    }

    pub fn is_empty(&self) -> bool {
        self.root.len() == 0
    }

    pub fn section(&mut self, name: &'static str) {
        self.end_section();
        self.open = Some((name, XxHash64::with_seed(CHECKSUM_SEED)));
    }

    pub fn end_section(&mut self) {
        if let Some((name, hasher)) = self.open.take() {
            self.sections.push(SectionDigest {
                name,
                hash: hasher.finish(),
                len: hasher.len(),
            });
        }
    }

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

    pub fn bool(&mut self, v: bool) {
        self.u8(u8::from(v));
    }

    pub fn fixed(&mut self, v: Fixed) {
        self.i32(v.raw());
    }

    pub fn len32(&mut self, n: usize) {
        assert!(n <= u32::MAX as usize, "canonical length {n} exceeds u32");
        self.u32(n as u32);
    }

    pub fn bytes(&mut self, v: &[u8]) {
        self.len32(v.len());
        self.raw(v);
    }

    pub fn str(&mut self, v: &str) {
        self.bytes(v.as_bytes());
    }

    pub fn seq<T>(&mut self, items: &[T], mut each: impl FnMut(&mut Canonical, &T)) {
        self.len32(items.len());
        for item in items {
            each(self, item);
        }
    }

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

    pub fn finish(mut self) -> Digest {
        self.end_section();
        Digest {
            hash: self.root.finish(),
            len: self.root.len(),
            sections: self.sections,
            bytes: self.bytes,
        }
    }

    pub fn hash_of<T: Encode>(value: &T) -> u64 {
        let mut c = Canonical::hashing();
        value.encode(&mut c);
        c.finish().hash
    }

    pub fn bytes_of<T: Encode>(value: &T) -> Vec<u8> {
        let mut c = Canonical::recording();
        value.encode(&mut c);
        c.finish().bytes.expect("recording encoder keeps its bytes")
    }
}

