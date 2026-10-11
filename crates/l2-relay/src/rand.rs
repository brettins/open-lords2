//! Codes and tokens from `/dev/urandom`, else the OS-seeded `RandomState`
//! over the clock, a counter and the thread (C254: std only).

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hash, Hasher};
use std::io::Read;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::protocol::ALPHABET;

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn urandom() -> Option<u64> {
    let mut bytes = [0u8; 8];
    std::fs::File::open("/dev/urandom").ok()?.read_exact(&mut bytes).ok()?;
    Some(u64::from_le_bytes(bytes))
}

fn fallback() -> u64 {
    let mut h = RandomState::new().build_hasher();
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0).hash(&mut h);
    COUNTER.fetch_add(1, Ordering::Relaxed).hash(&mut h);
    std::thread::current().id().hash(&mut h);
    h.finish()
}

pub fn word() -> u64 {
    urandom().unwrap_or_else(fallback)
}

/// `len` symbols of [`ALPHABET`], 5 bits each (32 symbols, C254).
pub fn symbols(len: usize) -> String {
    let mut out = String::with_capacity(len);
    let mut bits = word();
    for i in 0..len {
        if i > 0 && i % 12 == 0 {
            bits = word();
        }
        out.push(ALPHABET[(bits & 31) as usize] as char);
        bits >>= 5;
    }
    out
}
