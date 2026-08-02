//! Ring arithmetic: the negacyclic wrap, centering, and the scaling step.
//!
//! Everything here is checked against a property or an independently obvious
//! computation, never against a table of expected outputs. A table only records
//! what the first implementation happened to produce, which is worthless when
//! the first implementation is the thing under test.

use nsc_core::modulus::Modulus;
use nsc_core::ring::{Poly, PolyI128};
use nsc_core::Rng;

fn q() -> Modulus {
    Modulus::new(1_073_750_017) // 2^30-ish, NTT-friendly for N ≤ 2^12
}

#[test]
fn x_to_the_n_is_minus_one() {
    // The defining relation of the ring, tested at its most direct: X^(N-1)
    // times X should be -1, not +1. Get this backwards and every product is
    // subtly wrong in a way that still looks like a polynomial.
    let n = 8;
    let q = q();
    let mut top = Poly::zero(n);
    top.coeffs[n - 1] = 1; // X^(N-1)
    let mut x = Poly::zero(n);
    x.coeffs[1] = 1; // X

    let product = top.mul_schoolbook(&x, &q);
    let mut expected = Poly::zero(n);
    expected.coeffs[0] = q.sub(0, 1); // -1
    assert_eq!(product, expected, "X^(N-1) · X must be -1, not +1");
}

#[test]
fn multiplication_by_one_is_the_identity() {
    let n = 16;
    let q = q();
    let mut rng = Rng::from_seed(11);
    let a = Poly::from_coeffs((0..n).map(|_| rng.next_below(q.value())).collect());
    let mut one = Poly::zero(n);
    one.coeffs[0] = 1;
    assert_eq!(a.mul_schoolbook(&one, &q), a);
}

#[test]
fn multiplication_is_commutative_and_distributes() {
    let n = 16;
    let q = q();
    let mut rng = Rng::from_seed(12);
    let random =
        |rng: &mut Rng| Poly::from_coeffs((0..n).map(|_| rng.next_below(q.value())).collect());
    let a = random(&mut rng);
    let b = random(&mut rng);
    let c = random(&mut rng);

    assert_eq!(a.mul_schoolbook(&b, &q), b.mul_schoolbook(&a, &q));
    assert_eq!(
        a.mul_schoolbook(&b.add(&c, &q), &q),
        a.mul_schoolbook(&b, &q).add(&a.mul_schoolbook(&c, &q), &q),
        "multiplication must distribute over addition"
    );
}

#[test]
fn centering_reports_small_negatives_as_small() {
    // The reason this matters: noise is measured by magnitude. Read as an
    // unsigned residue, a noise of -3 reports as q-3 ≈ 2^30, turning a tiny
    // noise into an enormous one and making every measurement meaningless.
    let q = q();
    let modulus = q.value();
    let poly = Poly::from_coeffs(vec![0, 1, modulus - 1, modulus - 3, modulus / 2 + 1]);
    let centered = poly.center(&q);
    assert_eq!(centered.coeffs[0], 0);
    assert_eq!(centered.coeffs[1], 1);
    assert_eq!(centered.coeffs[2], -1);
    assert_eq!(centered.coeffs[3], -3);
    assert!(centered.coeffs[4] < 0, "just above q/2 must land negative");
    assert_eq!(centered.inf_norm(), (modulus / 2) as i128);
}

#[test]
fn center_then_reduce_is_the_identity() {
    let n = 32;
    let q = q();
    let mut rng = Rng::from_seed(13);
    let a = Poly::from_coeffs((0..n).map(|_| rng.next_below(q.value())).collect());
    assert_eq!(a.center(&q).reduce(&q), a);
}

#[test]
fn exact_multiplication_agrees_with_modular_multiplication() {
    // mul_exact is used for the BFV tensor product, where reduction must not
    // happen. Reducing its output afterwards must land where the modular
    // version does — otherwise the two paths disagree about the ring itself.
    let n = 16;
    let q = q();
    let mut rng = Rng::from_seed(14);
    let a = Poly::from_coeffs((0..n).map(|_| rng.next_below(q.value())).collect());
    let b = Poly::from_coeffs((0..n).map(|_| rng.next_below(q.value())).collect());

    let exact = a.center(&q).mul_exact(&b.center(&q)).reduce(&q);
    assert_eq!(exact, a.mul_schoolbook(&b, &q));
}

#[test]
fn scale_round_rounds_to_nearest_in_both_directions() {
    let q = 1000;
    let t = 1;
    let poly = PolyI128 {
        coeffs: vec![0, 499, 500, 501, -499, -500, -501, 1499, -1499],
    };
    let scaled = poly.scale_round(t, q);
    // Ties round in a fixed direction; what matters is that every result is
    // within half a unit of the true quotient, on both sides of zero.
    for (raw, got) in poly.coeffs.iter().zip(&scaled.coeffs) {
        let exact = *raw as f64 / q as f64;
        assert!(
            (exact - *got as f64).abs() <= 0.5 + 1e-9,
            "scale_round({raw}) = {got}, exact {exact}"
        );
    }
}

#[test]
fn scale_round_does_not_overflow_at_tensor_scale() {
    // The regression test for the bug found while choosing the phase-0
    // parameters. A tensor coefficient is bounded by N·(q/2)²; the naive
    // `c·t` would need ~2^131 and wrap silently in release builds, yielding a
    // plausible wrong plaintext. The values below are of that magnitude.
    let q = 288_230_376_151_736_833u64;
    let t = 256u64;
    let huge = (256i128 * (q as i128 / 2)) * (q as i128 / 2); // ≈ 2^123
    let poly = PolyI128 {
        coeffs: vec![huge, -huge, huge / 3, -huge / 7],
    };
    let scaled = poly.scale_round(t, q);

    for (raw, got) in poly.coeffs.iter().zip(&scaled.coeffs) {
        // Check against the mathematically exact quotient computed in a way
        // that cannot overflow: split the multiplication the same way the
        // implementation does, but derive the split independently.
        let quotient = raw.div_euclid(q as i128);
        let remainder = raw.rem_euclid(q as i128);
        let exact_low = (remainder * t as i128) as f64 / q as f64;
        let expected = quotient * t as i128 + exact_low.round() as i128;
        assert!(
            (expected - got).abs() <= 1,
            "scale_round overflowed or drifted: raw={raw} got={got} expected≈{expected}"
        );
        assert!(got.signum() == raw.signum(), "sign must survive scaling");
    }
}
