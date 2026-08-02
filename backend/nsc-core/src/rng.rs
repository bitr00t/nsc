//! A small deterministic RNG, and the distributions RLWE needs.
//!
//! Deliberately not `rand`. Two reasons, and the second is the real one:
//!
//! 1. No dependencies keeps CI trivial and the build reproducible.
//! 2. **Every experiment in this project must be reproducible from a seed.**
//!    Phase 5 compares predicted noise against measured noise; a measurement
//!    that cannot be replayed exactly is not evidence of anything. Seeding is
//!    therefore explicit at every call site rather than defaulted.
//!
//! This is **not** cryptographically secure and must never be used to generate
//! real keys. Phase 0 is about correctness of the arithmetic, not about
//! deployable key material — see `docs/ROADMAP.md` §8.

/// xoshiro256** — small, fast, good statistical quality, easy to verify.
#[derive(Clone, Debug)]
pub struct Rng {
    state: [u64; 4],
}

impl Rng {
    pub fn from_seed(seed: u64) -> Self {
        // SplitMix64 to spread one word across the whole state; seeding all
        // four words from the same value would correlate the early outputs.
        let mut z = seed;
        let mut next = || {
            z = z.wrapping_add(0x9E3779B97F4A7C15);
            let mut x = z;
            x = (x ^ (x >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
            x = (x ^ (x >> 27)).wrapping_mul(0x94D049BB133111EB);
            x ^ (x >> 31)
        };
        Rng {
            state: [next(), next(), next(), next()],
        }
    }

    pub fn next_u64(&mut self) -> u64 {
        let result = self.state[1].wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let t = self.state[1] << 17;
        self.state[2] ^= self.state[0];
        self.state[3] ^= self.state[1];
        self.state[1] ^= self.state[2];
        self.state[0] ^= self.state[3];
        self.state[2] ^= t;
        self.state[3] = self.state[3].rotate_left(45);
        result
    }

    /// Uniform in `[0, bound)`, rejection-sampled so the distribution is exact.
    pub fn next_below(&mut self, bound: u64) -> u64 {
        assert!(bound > 0);
        let zone = u64::MAX - (u64::MAX % bound);
        loop {
            let candidate = self.next_u64();
            if candidate < zone {
                return candidate % bound;
            }
        }
    }

    /// A ternary secret coefficient: -1, 0 or +1, uniform over the three.
    pub fn ternary(&mut self) -> i64 {
        match self.next_below(3) {
            0 => -1,
            1 => 0,
            _ => 1,
        }
    }

    /// A discrete-Gaussian-ish error coefficient.
    ///
    /// A sum of uniform bits (an Irwin–Hall approximation), *not* a real
    /// discrete Gaussian. Adequate for phase 0, where what matters is that the
    /// error is small and centered; a proper sampler is a prerequisite for any
    /// security claim and is deliberately out of scope here. The bound is what
    /// the noise analysis will reason about, and it is exact: `|e| <= 19`.
    pub fn error(&mut self) -> i64 {
        let bits = self.next_u64();
        let mut acc = 0i64;
        for i in 0..38 {
            acc += ((bits >> i) & 1) as i64;
        }
        acc - 19
    }
}

/// The exact bound on [`Rng::error`] output. The analysis in phase 2 will need
/// a number here, so it is stated once and asserted in the tests rather than
/// left as folklore.
pub const ERROR_BOUND: i64 = 19;
