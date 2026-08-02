//! Parameter sets, and the conditions they have to satisfy.
//!
//! Every one of the checks in [`Params::validate`] is a bug that would
//! otherwise surface as *quietly wrong plaintexts* rather than as an error. That
//! is the failure mode this whole project exists to attack, so the checks run
//! at construction and refuse rather than warn.

use crate::modulus::Modulus;

/// A BFV parameter set.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Params {
    /// Ring degree, a power of two.
    pub n: usize,
    /// Ciphertext modulus. Prime, and `≡ 1 (mod 2N)` so the NTT exists.
    pub q: u64,
    /// Plaintext modulus.
    pub t: u64,
    /// Relinearisation decomposition base.
    pub relin_base: u64,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ParamError {
    DegreeNotPowerOfTwo(usize),
    ModulusNotNttFriendly { q: u64, n: usize },
    ModulusNotPrime(u64),
    PlaintextModulusTooLarge { t: u64, q: u64 },
    TensorOverflow { n: usize, q: u64 },
    RelinBaseTooSmall(u64),
}

impl std::fmt::Display for ParamError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParamError::DegreeNotPowerOfTwo(n) => {
                write!(f, "ring degree {n} is not a power of two")
            }
            ParamError::ModulusNotNttFriendly { q, n } => write!(
                f,
                "q = {q} is not 1 mod 2N for N = {n}, so no negacyclic NTT exists"
            ),
            ParamError::ModulusNotPrime(q) => write!(f, "q = {q} is not prime"),
            ParamError::PlaintextModulusTooLarge { t, q } => {
                write!(f, "plaintext modulus {t} must be far below q = {q}")
            }
            ParamError::TensorOverflow { n, q } => write!(
                f,
                "N·q² overflows i128 for N = {n}, q = {q}: the BFV tensor product \
                 would silently wrap"
            ),
            ParamError::RelinBaseTooSmall(base) => {
                write!(f, "relinearisation base {base} must exceed 1")
            }
        }
    }
}

impl Params {
    pub fn new(n: usize, q: u64, t: u64, relin_base: u64) -> Result<Self, ParamError> {
        let params = Params {
            n,
            q,
            t,
            relin_base,
        };
        params.validate()?;
        Ok(params)
    }

    pub fn validate(&self) -> Result<(), ParamError> {
        if !self.n.is_power_of_two() {
            return Err(ParamError::DegreeNotPowerOfTwo(self.n));
        }
        if !is_prime(self.q) {
            return Err(ParamError::ModulusNotPrime(self.q));
        }
        if (self.q - 1) % (2 * self.n as u64) != 0 {
            return Err(ParamError::ModulusNotNttFriendly {
                q: self.q,
                n: self.n,
            });
        }
        if self.relin_base < 2 {
            return Err(ParamError::RelinBaseTooSmall(self.relin_base));
        }
        // t must leave room for noise: delta = q/t is the scaling factor, and a
        // t anywhere near q leaves no gap for the error to live in.
        if self.t == 0 || self.t.saturating_mul(1 << 10) >= self.q {
            return Err(ParamError::PlaintextModulusTooLarge {
                t: self.t,
                q: self.q,
            });
        }
        // The tensor product is computed over the integers. Its coefficients are
        // bounded by N·q², and that has to fit in i128 with room to spare.
        let q = self.q as u128;
        let bound = (self.n as u128).saturating_mul(q).saturating_mul(q);
        if bound > (i128::MAX as u128) / 4 {
            return Err(ParamError::TensorOverflow {
                n: self.n,
                q: self.q,
            });
        }
        Ok(())
    }

    pub fn modulus(&self) -> Modulus {
        Modulus::new(self.q)
    }

    /// `Δ = ⌊q/t⌋`, the scaling factor a plaintext is lifted by.
    pub fn delta(&self) -> u64 {
        self.q / self.t
    }

    /// The number of base-`relin_base` digits needed to cover `q`.
    pub fn relin_digits(&self) -> usize {
        let mut digits = 0;
        let mut covered = 1u128;
        while covered < self.q as u128 {
            covered *= self.relin_base as u128;
            digits += 1;
        }
        digits.max(1)
    }

    /// The decryption budget in bits.
    ///
    /// Decryption recovers the message iff the accumulated noise stays below
    /// `Δ/2 = q/(2t)`. This number is what phase 2's analysis will be proving
    /// things about; phase 0 only measures against it.
    pub fn budget_bits(&self) -> f64 {
        ((self.q as f64) / (2.0 * self.t as f64)).log2()
    }
}

fn is_prime(n: u64) -> bool {
    if n < 2 {
        return false;
    }
    for small in [2u64, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37] {
        if n == small {
            return true;
        }
        if n % small == 0 {
            return false;
        }
    }
    // Deterministic Miller-Rabin for u64 with this witness set.
    let mut d = n - 1;
    let mut r = 0;
    while d % 2 == 0 {
        d /= 2;
        r += 1;
    }
    'witness: for a in [2u64, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37] {
        let mut x = mod_pow(a, d, n);
        if x == 1 || x == n - 1 {
            continue;
        }
        for _ in 0..r - 1 {
            x = mod_mul(x, x, n);
            if x == n - 1 {
                continue 'witness;
            }
        }
        return false;
    }
    true
}

fn mod_mul(a: u64, b: u64, m: u64) -> u64 {
    ((a as u128 * b as u128) % m as u128) as u64
}

fn mod_pow(mut base: u64, mut exponent: u64, m: u64) -> u64 {
    let mut acc = 1u64;
    base %= m;
    while exponent > 0 {
        if exponent & 1 == 1 {
            acc = mod_mul(acc, base, m);
        }
        base = mod_mul(base, base, m);
        exponent >>= 1;
    }
    acc
}

impl Params {
    
}
