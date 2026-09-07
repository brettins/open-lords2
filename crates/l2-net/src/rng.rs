//! The simulation's random number generator, frozen in-tree.
//!
//! # The algorithm
//!
//! PCG-XSH-RR 64/32 — O'Neill's `pcg32`, the "minimal C implementation"
//! variant. 64 bits of state, 32 bits of output, an odd per-stream
//! increment, and a permutation on the output rather than on the state:
//!
//! ```text
//! old    = state
//! state  = old * 6364136223846793005 + increment      (mod 2^64)
//! xorsh  = ((old >> 18) ^ old) >> 27                  (32 bits)
//! rot    = old >> 59                                  (5 bits)
//! output = xorsh rotated right by rot
//! ```
//!
//! Two properties matter here and neither is about statistical quality.
//! The output is a function of the state *before* the advance, so a
//! generator that has been stepped `n` times is fully described by
//! `(state, increment)` and nothing else — it serialises into a snapshot
//! in sixteen bytes. And the increment selects one of 2^63 distinct
//! streams, so a subsystem that wants its own sequence can have one
//! without a second seed and without any chance of walking into another
//! subsystem's numbers.
//!
//! PCG32 is chosen over xoshiro256++ only because its state is half the
//! size and its published reference vectors are trivially reproducible
//! (see below). Statistically either would be far more than a 1996
//! turn-based game needs.
//!
//! # Why it is written out here rather than depended upon
//!
//! `docs/netcode.md` D-3 makes the argument and this crate's Cargo.toml
//! repeats it: the value stream must be frozen *forever*, and the
//! rust-random project explicitly permits value-breaking changes in
//! minor versions. A changed stream is not a bug that shows up as a
//! failing test. It shows up as one player desyncing from another, an
//! hour into a game, or as a replay that no longer reproduces the
//! recording it was made from.
//!
//! # This file is frozen
//!
//! Every function below defines part of a value stream that is baked
//! into saved replays and into other people's expectations of the game.
//! `tests/rng.rs` pins all of it: the raw output against O'Neill's
//! published demo, and every derived helper against our own recorded
//! vectors. **If you change anything here and a test in `tests/rng.rs`
//! fails, the test is right.** Adding a new helper is fine; changing
//! what an existing one returns is a breaking change to the game.
//!
//! # Rules this obeys
//!
//! No entropy source, no clock, no thread-local state, no global — a
//! [`Pcg32`] is a plain value that lives in the simulation state and is
//! advanced only by simulation code (D-3). It is `Clone`, which is what
//! makes a speculative "what would this roll be" query possible without
//! disturbing the real stream, and `PartialEq`, which is what lets a
//! desync dump say "the generators differ" rather than "something
//! differs".

/// The multiplier from the reference implementation. LCG parameters are
/// not interchangeable; this constant is part of the algorithm.
const MULTIPLIER: u64 = 6_364_136_223_846_793_005;

/// A seeded PCG32 generator.
///
/// ```
/// # use l2_net::Pcg32;
/// let mut rng = Pcg32::new(42, 54);
/// assert_eq!(rng.next_u32(), 0xa15c_02b7);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Pcg32 {
    state: u64,
    /// Always odd. The type cannot enforce that, so every constructor
    /// does.
    increment: u64,
}

impl Pcg32 {
    /// Seed the generator, exactly as the reference `pcg32_srandom_r`
    /// does: increment first, then two advances with the seed folded in
    /// between them.
    ///
    /// The dance looks arbitrary and is not. Setting `state = seed`
    /// directly would make `Pcg32::new(0, s)` and `Pcg32::new(1, s)`
    /// produce first outputs that differ in only a couple of bits,
    /// because one LCG step does not mix. Advancing before and after
    /// folding the seed in costs two multiplies once and removes that
    /// whole class of "the two players' seeds were adjacent integers"
    /// surprise.
    ///
    /// `stream` selects one of 2^63 independent sequences. Different
    /// streams from the same seed never produce the same sequence of
    /// states, so subsystems can be given their own stream instead of
    /// sharing one generator and depending on the order in which they
    /// happen to draw.
    pub fn new(seed: u64, stream: u64) -> Pcg32 {
        let mut rng = Pcg32 { state: 0, increment: (stream << 1) | 1 };
        rng.step();
        rng.state = rng.state.wrapping_add(seed);
        rng.step();
        rng
    }

    /// The stream every part of the simulation uses unless it has a
    /// reason not to.
    pub fn from_seed(seed: u64) -> Pcg32 {
        Pcg32::new(seed, DEFAULT_STREAM)
    }

    /// Rebuild a generator from a serialised `(state, increment)` pair.
    ///
    /// The increment is forced odd. A snapshot with an even increment is
    /// either corrupt or hand-written; silently repairing it is better
    /// than a generator whose period collapses, and it keeps the type's
    /// invariant true by construction rather than by hope.
    pub fn from_parts(state: u64, increment: u64) -> Pcg32 {
        Pcg32 { state, increment: increment | 1 }
    }

    /// The serialisable state. Sixteen bytes, and they are the whole
    /// generator.
    pub fn parts(&self) -> (u64, u64) {
        (self.state, self.increment)
    }

    #[inline]
    fn step(&mut self) {
        self.state = self.state.wrapping_mul(MULTIPLIER).wrapping_add(self.increment);
    }

    /// One 32-bit output. Every other method here is built from this
    /// one, so this is the only place the stream is defined.
    pub fn next_u32(&mut self) -> u32 {
        let old = self.state;
        self.step();
        let xorshifted = (((old >> 18) ^ old) >> 27) as u32;
        let rot = (old >> 59) as u32;
        xorshifted.rotate_right(rot)
    }

    /// Two draws, **high word first**.
    ///
    /// The order is arbitrary and therefore has to be written down: it
    /// is part of the value stream, and swapping it changes every
    /// 64-bit number the game has ever generated.
    pub fn next_u64(&mut self) -> u64 {
        let hi = self.next_u32() as u64;
        let lo = self.next_u32() as u64;
        (hi << 32) | lo
    }

    /// A uniform value in `0..bound`, with no modulo bias.
    ///
    /// Panics if `bound` is zero, because there is no value to return
    /// and a silent zero would be a bug that reached the player as a
    /// unit that never moves.
    ///
    /// The debiasing is the reference implementation's `bounded_rand`:
    /// reject the first `2^32 mod bound` outputs, which are exactly the
    /// ones that would make the low residues more likely, then take the
    /// remainder. Lemire's multiply-shift method is faster and would
    /// produce a *different stream*; the cost here is a rejection
    /// roughly `bound / 2^32` of the time, which for the numbers this
    /// game deals in is never.
    ///
    /// The loop is unbounded in principle. In practice, for a bound
    /// under a million, the chance of even one rejection is under
    /// 0.03%.
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

    /// A uniform value in `low..=high`, inclusive at both ends.
    ///
    /// Inclusive because game rules are written that way — "1 to 6
    /// damage" means six outcomes — and an off-by-one in a range helper
    /// is a bug that hides for months.
    ///
    /// Panics if `low > high`.
    pub fn range(&mut self, low: i32, high: i32) -> i32 {
        assert!(low <= high, "Pcg32::range({low}, {high}) is empty");
        // i64 throughout: `high - low` overflows i32 for a full-width
        // range, and `range(i32::MIN, i32::MAX)` is a legal thing to ask
        // for.
        let span = (high as i64 - low as i64 + 1) as u64;
        if span > u32::MAX as u64 {
            // Only reachable for spans wider than 2^32. `below` works in
            // 32 bits, so fall back to a full 32-bit draw plus a second
            // for the top bits.
            let r = self.next_u64() % span;
            return (low as i64 + r as i64) as i32;
        }
        low + self.below(span as u32) as i32
    }

    /// `numerator` chances in `denominator`.
    ///
    /// Panics if `denominator` is zero. `chance(0, n)` is always false
    /// and `chance(n, n)` is always true, and **both still draw**: a
    /// probability of zero that skips the draw would make the stream
    /// depend on the value, which is how a balance change to one number
    /// silently reshuffles every roll made after it.
    pub fn chance(&mut self, numerator: u32, denominator: u32) -> bool {
        assert!(denominator != 0, "Pcg32::chance with a zero denominator");
        self.below(denominator) < numerator
    }

    /// One in `n`.
    pub fn one_in(&mut self, n: u32) -> bool {
        self.chance(1, n)
    }

    /// Fisher-Yates, **downward**: `i` from `len - 1` to `1`, swapping
    /// `i` with `below(i + 1)`.
    ///
    /// The direction is written down because the upward variant is
    /// equally correct, equally uniform, and produces a different
    /// permutation from the same stream. There is one shuffle in this
    /// engine and this is it.
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

    /// Pick an index into a collection of `len` items, or `None` when
    /// it is empty.
    ///
    /// Returns an index rather than a reference so the caller keeps the
    /// borrow — and so the *index* can be recorded in a replay, which a
    /// reference cannot be.
    pub fn index(&mut self, len: usize) -> Option<usize> {
        if len == 0 {
            return None;
        }
        assert!(len <= u32::MAX as usize, "Pcg32::index on {len} items");
        Some(self.below(len as u32) as usize)
    }

    /// Advance the generator `n` times without using the output.
    ///
    /// This is a real jump-ahead in `O(log n)` — the LCG's multiplier
    /// and increment are exponentiated — not a loop, so skipping a
    /// billion draws costs about thirty multiplies. It exists because a
    /// replay that wants to resume at tick 10,000 should not have to
    /// pretend to draw.
    ///
    /// Verified against the loop in `tests/rng.rs`, which is the only
    /// way anyone should believe a closed form like this.
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

    /// A generator for a different stream, derived from this one.
    ///
    /// Draws one 64-bit value to pick the stream, so calling it
    /// *advances the parent* — deliberately. A fork that left the
    /// parent untouched would let "how many subsystems asked for a
    /// generator" become invisible in the state, and two peers that
    /// disagreed about that would produce identical checksums right up
    /// until the fork was used.
    pub fn fork(&mut self) -> Pcg32 {
        let seed = self.next_u64();
        let stream = self.next_u64();
        Pcg32::new(seed, stream)
    }
}

/// The stream [`Pcg32::from_seed`] uses. An arbitrary odd-ish constant;
/// its only job is to not be zero, so that a hand-written `Pcg32::new(s,
/// 0)` in a test is visibly a different stream from the engine's.
pub const DEFAULT_STREAM: u64 = 0x853c_49e6_748f_ea9b;
