
const MULTIPLIER: u64 = 6_364_136_223_846_793_005;

/// ```
/// # use l2_net::Pcg32;
/// let mut rng = Pcg32::new(42, 54);
/// assert_eq!(rng.next_u32(), 0xa15c_02b7);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Pcg32 {
    state: u64,
    increment: u64,
}

impl Pcg32 {
    pub fn new(seed: u64, stream: u64) -> Pcg32 {
        let mut rng = Pcg32 { state: 0, increment: (stream << 1) | 1 };
        rng.step();
        rng.state = rng.state.wrapping_add(seed);
        rng.step();
        rng
    }

    pub fn from_seed(seed: u64) -> Pcg32 {
        Pcg32::new(seed, DEFAULT_STREAM)
    }

    pub fn from_parts(state: u64, increment: u64) -> Pcg32 {
        Pcg32 { state, increment: increment | 1 }
    }

    pub fn parts(&self) -> (u64, u64) {
        (self.state, self.increment)
    }

    #[inline]
    fn step(&mut self) {
        self.state = self.state.wrapping_mul(MULTIPLIER).wrapping_add(self.increment);
    }

    pub fn next_u32(&mut self) -> u32 {
        let old = self.state;
        self.step();
        let xorshifted = (((old >> 18) ^ old) >> 27) as u32;
        let rot = (old >> 59) as u32;
        xorshifted.rotate_right(rot)
    }

    pub fn next_u64(&mut self) -> u64 {
        let hi = self.next_u32() as u64;
        let lo = self.next_u32() as u64;
        (hi << 32) | lo
    }

    pub fn below(&mut self, bound: u32) -> u32 {
        assert!(bound != 0, "Pcg32::below(0) has no value to return");
        let threshold = bound.wrapping_neg() % bound; // 2^32 mod bound
        loop {
            let r = self.next_u32();
            if r >= threshold {
                return r % bound;
            }
        }
    }

    pub fn range(&mut self, low: i32, high: i32) -> i32 {
        assert!(low <= high, "Pcg32::range({low}, {high}) is empty");
        let span = (high as i64 - low as i64 + 1) as u64;
        if span > u32::MAX as u64 {
            let r = self.next_u64() % span;
            return (low as i64 + r as i64) as i32;
        }
        low + self.below(span as u32) as i32
    }

    pub fn chance(&mut self, numerator: u32, denominator: u32) -> bool {
        assert!(denominator != 0, "Pcg32::chance with a zero denominator");
        self.below(denominator) < numerator
    }

    pub fn one_in(&mut self, n: u32) -> bool {
        self.chance(1, n)
    }

    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        if items.len() < 2 {
            return;
        }
        let mut i = items.len() - 1;
        while i > 0 {
            let j = self.below(i as u32 + 1) as usize;
            items.swap(i, j);
            i -= 1;
        }
    }

    pub fn index(&mut self, len: usize) -> Option<usize> {
        if len == 0 {
            return None;
        }
        assert!(len <= u32::MAX as usize, "Pcg32::index on {len} items");
        Some(self.below(len as u32) as usize)
    }

    pub fn advance(&mut self, n: u64) {
        let mut acc_mult: u64 = 1;
        let mut acc_plus: u64 = 0;
        let mut cur_mult = MULTIPLIER;
        let mut cur_plus = self.increment;
        let mut delta = n;
        while delta > 0 {
            if delta & 1 == 1 {
                acc_mult = acc_mult.wrapping_mul(cur_mult);
                acc_plus = acc_plus.wrapping_mul(cur_mult).wrapping_add(cur_plus);
            }
            cur_plus = cur_mult.wrapping_add(1).wrapping_mul(cur_plus);
            cur_mult = cur_mult.wrapping_mul(cur_mult);
            delta >>= 1;
        }
        self.state = acc_mult.wrapping_mul(self.state).wrapping_add(acc_plus);
    }

    pub fn fork(&mut self) -> Pcg32 {
        let seed = self.next_u64();
        let stream = self.next_u64();
        Pcg32::new(seed, stream)
    }
}

pub const DEFAULT_STREAM: u64 = 0x853c_49e6_748f_ea9b;
