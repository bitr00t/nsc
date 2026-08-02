//! The negacyclic NTT — multiplication in `R_q` in `O(N log N)`.
//!
//! A plain NTT computes a *cyclic* convolution, which is multiplication modulo
//! `X^N - 1`. We need it modulo `X^N + 1`. The standard fix: pick `ψ`, a
//! primitive `2N`-th root of unity mod `q`, and weight the input coefficient `i`
//! by `ψ^i` before transforming (and by `ψ^-i` after the inverse). The weighting
//! is what turns the wrap-around into a sign flip.
//!
//! This is why the modulus has to satisfy `q ≡ 1 (mod 2N)` — without that, no
//! primitive `2N`-th root exists and the whole construction is unavailable.
//! [`crate::params::Params::validate`] checks the congruence rather than letting
//! a bad parameter set produce quietly wrong products.
//!
//! Correctness here is established differentially: `ntt_tests` checks this
//! against [`crate::ring::Poly::mul_schoolbook`] on random inputs. The
//! schoolbook version is obviously right and quadratic; this one is fast and
//! subtle. Neither is checked against a table of expected values, because a
//! table would only ever encode whatever the first implementation happened to
//! produce.

use crate::modulus::Modulus;
use crate::ring::Poly;

/// Precomputed twiddle factors for one `(N, q)` pair.
#[derive(Clone, Debug)]
pub struct NttTables {
    n: usize,
    q: Modulus,
    /// `ψ^i` in bit-reversed order, for the forward transform.
    psi_powers: Vec<u64>,
    /// `ψ^-i` in bit-reversed order, for the inverse.
    inv_psi_powers: Vec<u64>,
    n_inverse: u64,
}

impl NttTables {
    /// # Panics
    /// If `n` is not a power of two, or `q ≢ 1 (mod 2n)`, or no primitive
    /// `2n`-th root of unity can be found.
    pub fn new(n: usize, q: Modulus) -> Self {
        assert!(n.is_power_of_two(), "N must be a power of two");
        let modulus = q.value();
        assert_eq!(
            (modulus - 1) % (2 * n as u64),
            0,
            "q must be 1 mod 2N for a negacyclic NTT to exist"
        );

        let psi = primitive_root_of_unity(2 * n as u64, q)
            .expect("a primitive 2N-th root of unity must exist when q = 1 mod 2N");
        let psi_inv = q.inverse(psi).expect("psi is nonzero");

        let mut psi_powers = vec![0u64; n];
        let mut inv_psi_powers = vec![0u64; n];
        let mut power = 1u64;
        let mut inv_power = 1u64;
        for i in 0..n {
            let rev = bit_reverse(i, n.trailing_zeros());
            psi_powers[rev] = power;
            inv_psi_powers[rev] = inv_power;
            power = q.mul(power, psi);
            inv_power = q.mul(inv_power, psi_inv);
        }

        let n_inverse = q.inverse(n as u64).expect("N is invertible mod q");

        NttTables {
            n,
            q,
            psi_powers,
            inv_psi_powers,
            n_inverse,
        }
    }

    pub fn n(&self) -> usize {
        self.n
    }

    pub fn modulus(&self) -> Modulus {
        self.q
    }

    /// Forward transform, in place. Input in natural order, output bit-reversed.
    pub fn forward(&self, values: &mut [u64]) {
        assert_eq!(values.len(), self.n);
        let q = &self.q;
        let mut t = self.n;
        let mut m = 1;
        while m < self.n {
            t /= 2;
            for i in 0..m {
                let j1 = 2 * i * t;
                let j2 = j1 + t;
                let s = self.psi_powers[m + i];
                for j in j1..j2 {
                    let u = values[j];
                    let v = q.mul(values[j + t], s);
                    values[j] = q.add(u, v);
                    values[j + t] = q.sub(u, v);
                }
            }
            m *= 2;
        }
    }

    /// Inverse transform, in place. Input bit-reversed, output natural order.
    pub fn inverse(&self, values: &mut [u64]) {
        assert_eq!(values.len(), self.n);
        let q = &self.q;
        let mut t = 1;
        let mut m = self.n;
        while m > 1 {
            let mut j1 = 0;
            let h = m / 2;
            for i in 0..h {
                let j2 = j1 + t;
                let s = self.inv_psi_powers[h + i];
                for j in j1..j2 {
                    let u = values[j];
                    let v = values[j + t];
                    values[j] = q.add(u, v);
                    values[j + t] = q.mul(q.sub(u, v), s);
                }
                j1 += 2 * t;
            }
            t *= 2;
            m /= 2;
        }
        for value in values.iter_mut() {
            *value = q.mul(*value, self.n_inverse);
        }
    }

    /// Negacyclic product of two polynomials in `R_q`.
    pub fn mul(&self, a: &Poly, b: &Poly) -> Poly {
        let mut fa = a.coeffs.clone();
        let mut fb = b.coeffs.clone();
        self.forward(&mut fa);
        self.forward(&mut fb);
        for (x, y) in fa.iter_mut().zip(&fb) {
            *x = self.q.mul(*x, *y);
        }
        self.inverse(&mut fa);
        Poly::from_coeffs(fa)
    }
}

fn bit_reverse(mut index: usize, bits: u32) -> usize {
    let mut out = 0;
    for _ in 0..bits {
        out = (out << 1) | (index & 1);
        index >>= 1;
    }
    out
}

/// Find a primitive `order`-th root of unity mod `q`.
///
/// Takes a generator candidate `g`, raises it to `(q-1)/order`, and checks the
/// result really has that order rather than a proper divisor of it — the check
/// that separates a primitive root from an impostor.
fn primitive_root_of_unity(order: u64, q: Modulus) -> Option<u64> {
    let modulus = q.value();
    if (modulus - 1) % order != 0 {
        return None;
    }
    let exponent = (modulus - 1) / order;
    for candidate in 2..1000u64 {
        let root = q.pow(candidate, exponent);
        if root <= 1 {
            continue;
        }
        // Primitive iff root^order = 1 and root^(order/p) != 1 for prime p | order.
        if q.pow(root, order) != 1 {
            continue;
        }
        let mut primitive = true;
        let mut divisor = 2;
        let mut remaining = order;
        while divisor * divisor <= remaining {
            if remaining % divisor == 0 {
                if q.pow(root, order / divisor) == 1 {
                    primitive = false;
                    break;
                }
                while remaining % divisor == 0 {
                    remaining /= divisor;
                }
            }
            divisor += 1;
        }
        if primitive && remaining > 1 && q.pow(root, order / remaining) == 1 {
            primitive = false;
        }
        if primitive {
            return Some(root);
        }
    }
    None
}
