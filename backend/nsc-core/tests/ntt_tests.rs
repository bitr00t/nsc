//! The NTT, checked differentially against schoolbook multiplication.
//!
//! There are two implementations of negacyclic multiplication in this crate on
//! purpose. One is quadratic and obviously correct; the other is `O(N log N)`
//! and full of bit-reversal and twiddle-factor detail that is easy to get
//! subtly wrong. Checking the fast one against the slow one on random inputs
//! is worth more than any fixed test vector, because a test vector can only
//! ever encode what the implementation already does.

use nsc_core::modulus::Modulus;
use nsc_core::ntt::NttTables;
use nsc_core::ring::Poly;
use nsc_core::Rng;

/// An NTT-friendly prime: `q ≡ 1 (mod 2N)` for every `N` used here.
const Q: u64 = 1_073_750_017;

fn random_poly(n: usize, q: u64, rng: &mut Rng) -> Poly {
    Poly::from_coeffs((0..n).map(|_| rng.next_below(q)).collect())
}

#[test]
fn forward_then_inverse_is_the_identity() {
    let q = Modulus::new(Q);
    for log_n in 3..=10 {
        let n = 1usize << log_n;
        let tables = NttTables::new(n, q);
        let mut rng = Rng::from_seed(100 + log_n as u64);
        let original = random_poly(n, Q, &mut rng);

        let mut values = original.coeffs.clone();
        tables.forward(&mut values);
        tables.inverse(&mut values);
        assert_eq!(values, original.coeffs, "roundtrip failed at N = {n}");
    }
}

#[test]
fn ntt_multiplication_matches_schoolbook() {
    let q = Modulus::new(Q);
    for log_n in 3..=9 {
        let n = 1usize << log_n;
        let tables = NttTables::new(n, q);
        let mut rng = Rng::from_seed(200 + log_n as u64);

        for _ in 0..4 {
            let a = random_poly(n, Q, &mut rng);
            let b = random_poly(n, Q, &mut rng);
            assert_eq!(
                tables.mul(&a, &b),
                a.mul_schoolbook(&b, &q),
                "NTT and schoolbook disagree at N = {n}"
            );
        }
    }
}

#[test]
fn ntt_respects_the_negacyclic_wrap() {
    // The property the twiddle weighting exists to provide. If the ψ-weighting
    // were dropped, the transform would compute a cyclic convolution and this
    // would come out +1 instead of -1 — and every other test above would still
    // pass, because both implementations would be... no: schoolbook would
    // disagree. Which is precisely why both exist.
    let n = 16;
    let q = Modulus::new(Q);
    let tables = NttTables::new(n, q);

    let mut top = Poly::zero(n);
    top.coeffs[n - 1] = 1;
    let mut x = Poly::zero(n);
    x.coeffs[1] = 1;

    let mut expected = Poly::zero(n);
    expected.coeffs[0] = q.sub(0, 1);
    assert_eq!(tables.mul(&top, &x), expected);
}

#[test]
fn multiplication_by_a_constant_scales_every_coefficient() {
    let n = 32;
    let q = Modulus::new(Q);
    let tables = NttTables::new(n, q);
    let mut rng = Rng::from_seed(300);

    let a = random_poly(n, Q, &mut rng);
    let scalar = 12345u64;
    let mut constant = Poly::zero(n);
    constant.coeffs[0] = scalar;

    assert_eq!(tables.mul(&a, &constant), a.scalar_mul(scalar, &q));
}

#[test]
#[should_panic(expected = "negacyclic NTT")]
fn a_modulus_without_the_right_roots_is_refused() {
    // 97 is prime but 97 ≢ 1 (mod 2·64), so no primitive 128th root of unity
    // exists. Constructing the tables anyway would produce a transform that
    // computes something, just not a convolution.
    NttTables::new(64, Modulus::new(97));
}
