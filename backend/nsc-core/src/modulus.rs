//! Arithmetic modulo a single machine-word prime.
//!
//! Nothing clever — no Montgomery, no Barrett. Phase 0 is about being obviously
//! correct; the roadmap is explicit that speed is not the claim (`docs/ROADMAP.md`
//! §8). `u128` intermediates keep every product exact, and when this becomes the
//! bottleneck it can be replaced behind an unchanged interface.

/// A prime modulus below `2^62`, with the arithmetic that goes with it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Modulus {
    q: u64,
}

impl Modulus {
    /// # Panics
    /// If `q` is not in `(1, 2^62)`. The upper bound keeps `a + b` inside `u64`
    /// and every product inside `u128`.
    pub fn new(q: u64) -> Self {
        assert!(q > 1, "modulus must exceed 1");
        assert!(q < (1u64 << 62), "modulus must be below 2^62");
        Modulus { q }
    }

    pub fn value(&self) -> u64 {
        self.q
    }

    pub fn reduce(&self, a: u64) -> u64 {
        a % self.q
    }

    pub fn add(&self, a: u64, b: u64) -> u64 {
        let s = a + b;
        if s >= self.q {
            s - self.q
        } else {
            s
        }
    }

    pub fn sub(&self, a: u64, b: u64) -> u64 {
        if a >= b {
            a - b
        } else {
            a + self.q - b
        }
    }

    pub fn mul(&self, a: u64, b: u64) -> u64 {
        ((a as u128 * b as u128) % self.q as u128) as u64
    }

    pub fn pow(&self, base: u64, mut exponent: u64) -> u64 {
        let mut acc = 1u64;
        let mut b = base % self.q;
        while exponent > 0 {
            if exponent & 1 == 1 {
                acc = self.mul(acc, b);
            }
            b = self.mul(b, b);
            exponent >>= 1;
        }
        acc
    }

    /// Inverse via Fermat. Valid because `q` is prime; `None` for zero.
    pub fn inverse(&self, a: u64) -> Option<u64> {
        if a % self.q == 0 {
            return None;
        }
        Some(self.pow(a, self.q - 2))
    }

    /// Signed representative in `(-q/2, q/2]`.
    pub fn center(&self, a: u64) -> i128 {
        let a = a as i128;
        let q = self.q as i128;
        if a > q / 2 {
            a - q
        } else {
            a
        }
    }
}
