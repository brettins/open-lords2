//! xxHash64 (XXH64), vendored.
//!
//! # What this is for
//!
//! One job: turn a canonical byte stream into a 64-bit number that two
//! machines will agree on. It is a checksum for desync detection
//! (`docs/netcode.md` §6), not a hash table hasher and not a
//! cryptographic anything. An attacker who can choose our input can
//! collide it trivially; that is fine, because §8 says plainly that we
//! do not defend against a modified client.
//!
//! # Why this algorithm
//!
//! Three candidates were rejected in the design and the reasons are
//! worth keeping next to the code, because each one is a trap that
//! passes tests:
//!
//! * **`DefaultHasher`** is deterministic within a binary *and across
//!   runs of the same binary*, and unstable only across Rust versions.
//!   That is the worst possible failure shape: every test passes on the
//!   machine that wrote them, and two players on different toolchains
//!   desync on tick 1.
//! * **`ahash`** says in its own README that it is not for network use
//!   or persisted values. It is randomly seeded.
//! * **`#[derive(Hash)]`, whatever the hasher underneath.** `core`'s
//!   own documentation says the bytes `Hash` feeds a `Hasher` are not
//!   portable across platforms or stable between compiler versions —
//!   `Vec` and `BTreeMap` write their lengths as `usize`. So the `Hash`
//!   trait is not implemented for anything in this crate that
//!   represents simulation state, and [`Canonical`](crate::Canonical)
//!   exists so there is an obvious right way to do it instead.
//!
//! FNV-1a was the design's first choice and was dropped for a real
//! reason: its avalanche on long runs of structured, mostly-zero
//! records — which is precisely what a serialised unit array is — is
//! poor, and a checksum whose whole purpose is that any one-bit
//! difference changes the output cannot afford that.
//!
//! # Why vendored rather than `twox-hash`
//!
//! Argued in Cargo.toml. Short version: 120 lines against a dependency,
//! where the 120 lines are pinned by someone else's published test
//! vectors.
//!
//! # This file is frozen
//!
//! XXH64 is an external specification and this is a transcription of
//! it. `tests/hash.rs` checks it against the reference implementation's
//! own values. Those tests are not a formality — a transcription error
//! in the tail handling, which only fires on inputs of certain lengths,
//! is exactly the kind of bug that would pass a "does it hash the same
//! twice" check and then produce a checksum nobody else in the world
//! agrees with.

const PRIME1: u64 = 0x9E37_79B1_85EB_CA87;
const PRIME2: u64 = 0xC2B2_AE3D_27D4_EB4F;
const PRIME3: u64 = 0x1656_67B1_9E37_79F9;
const PRIME4: u64 = 0x85EB_CA77_C2B2_AE63;
const PRIME5: u64 = 0x27D4_EB2F_1656_67C5;

/// Hash a complete buffer. Equivalent to `XXH64(bytes, len, seed)`.
///
/// A oneshot function, deliberately: `docs/netcode.md` §6 insists the
/// checksum is taken over *our* byte stream with a oneshot API and
/// never through the `Hash` trait, and an API that offers only bytes in
/// and a number out cannot be misused that way.
pub fn xxhash64(bytes: &[u8], seed: u64) -> u64 {
    let mut h = XxHash64::with_seed(seed);
    h.write(bytes);
    h.finish()
}

/// Streaming XXH64.
///
/// Produces the same value as [`xxhash64`] over the concatenation of
/// every `write`, regardless of how the writes are split. That property
/// is what lets [`Canonical`](crate::Canonical) hash a whole world
/// state field by field without ever building the byte stream in
/// memory — the serialisation of a battle is not large, but at 10 Hz it
/// would be a megabyte a second of pointless allocation.
#[derive(Debug, Clone)]
pub struct XxHash64 {
    seed: u64,
    total_len: u64,
    acc: [u64; 4],
    /// Bytes not yet part of a complete 32-byte block.
    buf: [u8; 32],
    buf_len: usize,
}

impl Default for XxHash64 {
    fn default() -> Self {
        XxHash64::with_seed(0)
    }
}

impl XxHash64 {
    pub fn new() -> XxHash64 {
        XxHash64::with_seed(0)
    }

    pub fn with_seed(seed: u64) -> XxHash64 {
        XxHash64 {
            seed,
            total_len: 0,
            acc: [
                seed.wrapping_add(PRIME1).wrapping_add(PRIME2),
                seed.wrapping_add(PRIME2),
                seed,
                seed.wrapping_sub(PRIME1),
            ],
            buf: [0; 32],
            buf_len: 0,
        }
    }

    /// Total bytes written so far. Cheap, and worth carrying into a
    /// desync dump: two peers whose state hashes differ *and* whose
    /// serialised lengths differ have a structural divergence — a
    /// different number of units — rather than a numeric one, which is
    /// a much shorter path to the cause.
    pub fn len(&self) -> u64 {
        self.total_len
    }

    pub fn is_empty(&self) -> bool {
        self.total_len == 0
    }

    pub fn write(&mut self, mut bytes: &[u8]) {
        self.total_len += bytes.len() as u64;

        if self.buf_len > 0 {
            let take = core::cmp::min(32 - self.buf_len, bytes.len());
            self.buf[self.buf_len..self.buf_len + take].copy_from_slice(&bytes[..take]);
            self.buf_len += take;
            bytes = &bytes[take..];
            if self.buf_len < 32 {
                return;
            }
            let block = self.buf;
            self.consume_block(&block);
            self.buf_len = 0;
        }

        while bytes.len() >= 32 {
            let (block, rest) = bytes.split_at(32);
            self.consume_block(block);
            bytes = rest;
        }

        if !bytes.is_empty() {
            self.buf[..bytes.len()].copy_from_slice(bytes);
            self.buf_len = bytes.len();
        }
    }

    fn consume_block(&mut self, block: &[u8]) {
        debug_assert_eq!(block.len(), 32);
        for lane in 0..4 {
            self.acc[lane] = round(self.acc[lane], read_u64(&block[lane * 8..]));
        }
    }

    /// The hash of everything written so far.
    ///
    /// Takes `&self`, not `self`: the caller of a state checksum
    /// usually wants the number *and* to keep hashing (a per-subsystem
    /// digest followed by the whole-state digest, say). A consuming
    /// `finish` would force a clone at every section boundary.
    pub fn finish(&self) -> u64 {
        let mut h = if self.total_len >= 32 {
            let mut h = self.acc[0]
                .rotate_left(1)
                .wrapping_add(self.acc[1].rotate_left(7))
                .wrapping_add(self.acc[2].rotate_left(12))
                .wrapping_add(self.acc[3].rotate_left(18));
            for lane in 0..4 {
                h = merge(h, self.acc[lane]);
            }
            h
        } else {
            self.seed.wrapping_add(PRIME5)
        };

        h = h.wrapping_add(self.total_len);

        let tail = &self.buf[..self.buf_len];
        let mut i = 0;
        while i + 8 <= tail.len() {
            h ^= round(0, read_u64(&tail[i..]));
            h = h.rotate_left(27).wrapping_mul(PRIME1).wrapping_add(PRIME4);
            i += 8;
        }
        if i + 4 <= tail.len() {
            h ^= (read_u32(&tail[i..]) as u64).wrapping_mul(PRIME1);
            h = h.rotate_left(23).wrapping_mul(PRIME2).wrapping_add(PRIME3);
            i += 4;
        }
        while i < tail.len() {
            h ^= (tail[i] as u64).wrapping_mul(PRIME5);
            h = h.rotate_left(11).wrapping_mul(PRIME1);
            i += 1;
        }

        avalanche(h)
    }
}

#[inline]
fn round(acc: u64, input: u64) -> u64 {
    acc.wrapping_add(input.wrapping_mul(PRIME2)).rotate_left(31).wrapping_mul(PRIME1)
}

#[inline]
fn merge(acc: u64, lane: u64) -> u64 {
    (acc ^ round(0, lane)).wrapping_mul(PRIME1).wrapping_add(PRIME4)
}

#[inline]
fn avalanche(mut h: u64) -> u64 {
    h ^= h >> 33;
    h = h.wrapping_mul(PRIME2);
    h ^= h >> 29;
    h = h.wrapping_mul(PRIME3);
    h ^= h >> 32;
    h
}

/// Explicit little-endian reads. `from_le_bytes` rather than a
/// transmute, so the result is the same on a big-endian machine — the
/// specification says XXH64 is byte-order independent and a pointer
/// cast would quietly make that false.
#[inline]
fn read_u64(bytes: &[u8]) -> u64 {
    let mut b = [0u8; 8];
    b.copy_from_slice(&bytes[..8]);
    u64::from_le_bytes(b)
}

#[inline]
fn read_u32(bytes: &[u8]) -> u32 {
    let mut b = [0u8; 4];
    b.copy_from_slice(&bytes[..4]);
    u32::from_le_bytes(b)
}
