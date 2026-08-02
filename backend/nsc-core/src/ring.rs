//! The ring `R_q = Z_q[X]/(X^N + 1)` — the object everything else is built on.
//!
//! Two representations of the same polynomial live here, and keeping them
//! distinct matters:
//!
//! * **Coefficient form** (`Poly`), coefficients reduced mod `q`. This is what
//!   ciphertexts are stored as.
//! * **Exact integer form** (`PolyI128`), coefficients *not* reduced. BFV
//!   multiplication needs the tensor product computed over the integers before
//!   scaling by `t/q`; reducing mod `q` first would destroy exactly the
//!   information the scaling recovers.
//!
//! `X^N + 1` makes the convolution *negacyclic*: a term that wraps past degree
//! `N` comes back with a minus sign, because `X^N = -1`. Every multiplication
//! routine here has to honour that, and getting it wrong is the classic silent
//! bug in an RLWE implementation — the arithmetic stays plausible and the
//! decryptions stay *almost* right.

use crate::modulus::Modulus;

/// A polynomial in `R_q`, in coefficient form.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Poly {
    pub coeffs: Vec<u64>,
}

/// A polynomial with exact (unreduced) signed coefficients.
///
/// Used only for the BFV tensor product, where the whole point is to *not*
/// reduce. The coefficients of a product of two polynomials with coefficients
/// below `q` are bounded by `N·q²`, so the caller is responsible for choosing
/// parameters where that fits in an `i128` — [`crate::params::Params::validate`]
/// checks it rather than leaving it to hope.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PolyI128 {
    pub coeffs: Vec<i128>,
}

impl Poly {
    pub fn zero(n: usize) -> Self {
        Poly { coeffs: vec![0; n] }
    }

    pub fn from_coeffs(coeffs: Vec<u64>) -> Self {
        Poly { coeffs }
    }

    pub fn len(&self) -> usize {
        self.coeffs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.coeffs.is_empty()
    }

    pub fn add(&self, other: &Poly, q: &Modulus) -> Poly {
        debug_assert_eq!(self.len(), other.len());
        Poly {
            coeffs: self
                .coeffs
                .iter()
                .zip(&other.coeffs)
                .map(|(a, b)| q.add(*a, *b))
                .collect(),
        }
    }

    pub fn sub(&self, other: &Poly, q: &Modulus) -> Poly {
        debug_assert_eq!(self.len(), other.len());
        Poly {
            coeffs: self
                .coeffs
                .iter()
                .zip(&other.coeffs)
                .map(|(a, b)| q.sub(*a, *b))
                .collect(),
        }
    }

    pub fn neg(&self, q: &Modulus) -> Poly {
        Poly {
            coeffs: self.coeffs.iter().map(|a| q.sub(0, *a)).collect(),
        }
    }

    /// Multiply by a scalar, mod `q`.
    pub fn scalar_mul(&self, scalar: u64, q: &Modulus) -> Poly {
        Poly {
            coeffs: self.coeffs.iter().map(|a| q.mul(*a, scalar)).collect(),
        }
    }

    /// Schoolbook negacyclic multiplication mod `q`.
    ///
    /// Quadratic, and kept deliberately: it is the reference the NTT is checked
    /// against (`ntt_tests`). Two implementations of the same operation are
    /// worth having when one of them is obviously correct and slow — that is a
    /// differential test, not duplication.
    pub fn mul_schoolbook(&self, other: &Poly, q: &Modulus) -> Poly {
        let n = self.len();
        debug_assert_eq!(n, other.len());
        let mut acc = vec![0u64; n];
        for (i, a) in self.coeffs.iter().enumerate() {
            if *a == 0 {
                continue;
            }
            for (j, b) in other.coeffs.iter().enumerate() {
                let prod = q.mul(*a, *b);
                let k = i + j;
                if k < n {
                    acc[k] = q.add(acc[k], prod);
                } else {
                    // X^N = -1: everything past the top wraps with a sign flip.
                    acc[k - n] = q.sub(acc[k - n], prod);
                }
            }
        }
        Poly { coeffs: acc }
    }

    /// Centered lift: coefficients into `(-q/2, q/2]` as signed integers.
    ///
    /// Noise is a *small* quantity, and "small" in `Z_q` means small in
    /// absolute value after centering. Reading a noise coefficient as an
    /// unsigned residue would report `q - 3` where the honest answer is `-3`,
    /// which turns a tiny noise into an enormous one and makes every
    /// measurement in `noise.rs` meaningless.
    pub fn center(&self, q: &Modulus) -> PolyI128 {
        let modulus = q.value() as i128;
        let half = modulus / 2;
        PolyI128 {
            coeffs: self
                .coeffs
                .iter()
                .map(|c| {
                    let c = *c as i128;
                    if c > half {
                        c - modulus
                    } else {
                        c
                    }
                })
                .collect(),
        }
    }
}

impl PolyI128 {
    pub fn zero(n: usize) -> Self {
        PolyI128 { coeffs: vec![0; n] }
    }

    pub fn len(&self) -> usize {
        self.coeffs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.coeffs.is_empty()
    }

    /// The largest coefficient in absolute value — the norm noise is measured in.
    pub fn inf_norm(&self) -> i128 {
        self.coeffs.iter().map(|c| c.abs()).max().unwrap_or(0)
    }

    /// Exact negacyclic product over the integers, no modular reduction.
    ///
    /// This is the step BFV multiplication cannot do mod `q`.
    pub fn mul_exact(&self, other: &PolyI128) -> PolyI128 {
        let n = self.len();
        debug_assert_eq!(n, other.len());
        let mut acc = vec![0i128; n];
        for (i, a) in self.coeffs.iter().enumerate() {
            if *a == 0 {
                continue;
            }
            for (j, b) in other.coeffs.iter().enumerate() {
                let prod = a * b;
                let k = i + j;
                if k < n {
                    acc[k] += prod;
                } else {
                    acc[k - n] -= prod;
                }
            }
        }
        PolyI128 { coeffs: acc }
    }

    pub fn add(&self, other: &PolyI128) -> PolyI128 {
        debug_assert_eq!(self.len(), other.len());
        PolyI128 {
            coeffs: self
                .coeffs
                .iter()
                .zip(&other.coeffs)
                .map(|(a, b)| a + b)
                .collect(),
        }
    }

    /// Multiply every coefficient by `t` and divide by `q`, rounding to nearest.
    ///
    /// The scaling step of BFV multiplication, and the one piece of arithmetic
    /// here with a genuine overflow hazard.
    ///
    /// The obvious implementation is `(c·t + q/2) / q`. It is wrong for the
    /// inputs it will actually see. A tensor coefficient is bounded by
    /// `N·(q/2)²`; at the phase-0 parameters that is around `2^123`, and
    /// multiplying by `t = 256` first would need `2^131` — past `i128`, wrapping
    /// silently in release builds and producing a plausible wrong plaintext.
    /// Which is precisely the failure mode this project exists to attack, so
    /// getting it wrong here would have been a poor start.
    ///
    /// The fix is exact rather than approximate. Split `c = quo·q + rem` by
    /// Euclidean division; then
    ///
    /// ```text
    ///   c·t / q  =  quo·t + rem·t / q
    /// ```
    ///
    /// and both terms are small: `quo ≤ c/q ≈ 2^64` and `rem < q`, so neither
    /// product leaves `i128`. Using `rem_euclid` keeps `rem` non-negative, which
    /// also makes the rounding uniform instead of needing a sign branch.
    ///
    /// Rounding is to *nearest*, not truncating: truncation biases every
    /// coefficient the same way, and a systematic bias accumulates over a
    /// multiplication chain in a way that random noise does not.
    pub fn scale_round(&self, t: u64, q: u64) -> PolyI128 {
        let t = t as i128;
        let q = q as i128;
        PolyI128 {
            coeffs: self
                .coeffs
                .iter()
                .map(|c| {
                    let quotient = c.div_euclid(q);
                    let remainder = c.rem_euclid(q);
                    quotient * t + (remainder * t + q / 2) / q
                })
                .collect(),
        }
    }

    /// Reduce into `R_q`.
    pub fn reduce(&self, q: &Modulus) -> Poly {
        let modulus = q.value() as i128;
        Poly {
            coeffs: self
                .coeffs
                .iter()
                .map(|c| {
                    let r = c.rem_euclid(modulus);
                    r as u64
                })
                .collect(),
        }
    }
}
