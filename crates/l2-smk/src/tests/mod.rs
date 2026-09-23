

mod decoding;

use std::collections::BTreeMap;

use super::*;

#[derive(Default)]
struct BitWriter {
    bytes: Vec<u8>,
    len: usize,
}

impl BitWriter {
    pub(super) fn bit(&mut self, b: u32) {
        if self.len % 8 == 0 {
            self.bytes.push(0);
        }
        if b & 1 != 0 {
            *self.bytes.last_mut().unwrap() |= 1 << (self.len % 8);
        }
        self.len += 1;
    }
    pub(super) fn bits(&mut self, v: u32, n: u32) {
        for i in 0..n {
            self.bit((v >> i) & 1);
        }
    }
    fn path(&mut self, p: &[u8]) {
        for &b in p {
            self.bit(b as u32);
        }
    }
}

type Codes = BTreeMap<u32, Vec<u8>>;

fn emit(w: &mut BitWriter, leaves: &[u32], prefix: Vec<u8>, codes: &mut Codes, leaf: &dyn Fn(&mut BitWriter, u32)) {
    if leaves.len() == 1 {
        w.bit(0);
        leaf(w, leaves[0]);
        codes.insert(leaves[0], prefix);
        return;
    }
    w.bit(1);
    let (a, b) = leaves.split_at(leaves.len() / 2);
    let mut zero = prefix.clone();
    zero.push(0);
    emit(w, a, zero, codes, leaf);
    let mut one = prefix;
    one.push(1);
    emit(w, b, one, codes, leaf);
}

fn tree8(w: &mut BitWriter, leaves: &[u32]) -> Codes {
    let mut codes = Codes::new();
    w.bit(1);
    emit(w, leaves, Vec::new(), &mut codes, &|w, v| w.bits(v, 8));
    w.bit(0);
    codes
}

fn distinct(v: impl Iterator<Item = u32>) -> Vec<u32> {
    let mut s: Vec<u32> = v.collect();
    s.sort();
    s.dedup();
    s
}

fn tree16(w: &mut BitWriter, leaves: &[u32], escapes: [u32; 3]) -> Codes {
    w.bit(1);
    let lows = tree8(w, &distinct(leaves.iter().map(|v| v & 0xFF)));
    let highs = tree8(w, &distinct(leaves.iter().map(|v| v >> 8)));
    for e in escapes {
        w.bits(e, 16);
    }
    let mut codes = Codes::new();
    emit(w, leaves, Vec::new(), &mut codes, &|w, v| {
        w.path(&lows[&(v & 0xFF)]);
        w.path(&highs[&(v >> 8)]);
    });
    w.bit(0);
    codes
}

fn pad4(v: &mut Vec<u8>) {
    while v.len() % 4 != 0 {
        v.push(0);
    }
}

const ESC: [u32; 3] = [0xFFF0, 0xFFF1, 0xFFF2];

fn synthetic() -> Vec<u8> {
    let mut t = BitWriter::default();
    let mmap = tree16(&mut t, &[0x8001], ESC);
    let mclr = tree16(&mut t, &[0x0907], ESC);
    let full = tree16(&mut t, &[0x0B0A, 0x0D0C], ESC);
    let types = tree16(&mut t, &[0x0000, 0x0001, 0x0002, 0x0503, 0xFFF0, 0xFFF1, 0xFFF2], ESC);
    let trees = t.bytes;

    let mut f0 = Vec::new();
    let pal0: Vec<u8> = vec![
        0x3F, 0x00, 0x00, // entry 0 = (63, 0, 0)
        0x00, 0x10, 0x00, // entry 1 = (0, 16, 0)
        0x81, //             skip entries 2 and 3
        0x40, 0x00, //       entry 4 = old entry 0, which is still black
        0x01, 0x02, 0x03, // entry 5
        0x80, 0x80, 0x80, // three one-entry skips: padding the chunk to 16
    ];
    f0.push(((pal0.len() + 1) / 4) as u8);
    f0.extend_from_slice(&pal0);
    let mut a = BitWriter::default();
    a.bit(1); // data present
    a.bit(0); // mono
    a.bit(0); // 8-bit
    let deltas = tree8(&mut a, &[0x01, 0xFF]);
    a.bits(0x80, 8);
    for d in [0x01, 0x01, 0xFF] {
        a.path(&deltas[&d]);
    }
    let mut audio = 4u32.to_le_bytes().to_vec();
    audio.extend_from_slice(&a.bytes);
    f0.extend_from_slice(&((audio.len() + 4) as u32).to_le_bytes());
    f0.extend_from_slice(&audio);
    let mut v = BitWriter::default();
    v.path(&types[&0x0503]);
    v.path(&types[&0x0000]);
    v.path(&mclr[&0x0907]);
    v.path(&mmap[&0x8001]);
    v.path(&types[&0x0001]);
    for _ in 0..4 {
        v.path(&full[&0x0B0A]);
        v.path(&full[&0x0D0C]);
    }
    v.path(&types[&0x0002]);
    f0.extend_from_slice(&v.bytes);
    pad4(&mut f0);

    let mut f1 = Vec::new();
    let pal1: Vec<u8> = vec![
        0x00, 0x00, 0x3F, // entry 0 = blue
        0x40, 0x00, //       entry 1 = OLD entry 0, which is red, not blue
        0x80, 0x80, //       padding
    ];
    f1.push(((pal1.len() + 1) / 4) as u8);
    f1.extend_from_slice(&pal1);
    let mut v = BitWriter::default();
    v.path(&types[&0x0002]); // skip; recent = [2, 0, 0]
    v.path(&types[&ESC[0]]); // recent[0] = skip
    v.path(&types[&0x0503]); // solid 5; recent = [0x503, 2, 0]
    v.path(&types[&ESC[1]]); // recent[1] = skip
    f1.extend_from_slice(&v.bytes);
    pad4(&mut f1);

    let mut file = Vec::new();
    file.extend_from_slice(b"SMK2");
    for x in [8u32, 8, 2] {
        file.extend_from_slice(&x.to_le_bytes());
    }
    file.extend_from_slice(&(-8333i32).to_le_bytes());
    file.extend_from_slice(&0u32.to_le_bytes());
    file.extend_from_slice(&4u32.to_le_bytes());
    file.extend_from_slice(&[0u8; 24]);
    file.extend_from_slice(&(trees.len() as u32).to_le_bytes());
    file.extend_from_slice(&[0u8; 16]);
    file.extend_from_slice(&0x8000_2B11u32.to_le_bytes());
    file.extend_from_slice(&[0u8; 24]);
    file.extend_from_slice(&[0u8; 4]);
    assert_eq!(file.len(), HEADER_LEN);
    file.extend_from_slice(&(f0.len() as u32).to_le_bytes());
    file.extend_from_slice(&(f1.len() as u32).to_le_bytes());
    file.extend_from_slice(&[3, 1]);
    file.extend_from_slice(&trees);
    file.extend_from_slice(&f0);
    file.extend_from_slice(&f1);
    file
}

