# Design decisions

Kept in the order the decisions were made, with the reasoning and — where there
was one — the option that was rejected. A decision without its alternative is
just a description of the code.

---

## Phase 0

### The failure mode goes in the library, not in a test

The depth cliff is defined in `src/demo.rs` and exported, rather than living
inside a test file. It is referred to from the tests, from the demo binary, from
CI, and from every later phase that has to say what it does about it. There
should be exactly one definition of what "the depth cliff" means.

`CLIFF_DEPTH` and `LAST_GOOD_DEPTH` are constants for the same reason: a change
to the arithmetic that moves the cliff should fail a test rather than quietly
move the goalposts.

### No dependencies, not even `rand`

*Rejected:* `rand` with `StdRng` and explicit seeding, which would have been
less code.

The deciding argument is not dependency hygiene, it is reproducibility. Phase 5's
result is a comparison between predicted and measured noise; a measurement that
cannot be replayed exactly is not evidence. With an in-tree xoshiro256\*\*, the
seed is the whole state, seeding is explicit at every call site, and a figure in
a document can be regenerated years later.

The cost is stated where it matters: `rng.rs` says plainly that it is not
cryptographically secure and must not generate real keys.

### Two implementations of polynomial multiplication

`Poly::mul_schoolbook` is quadratic and obviously correct. `NttTables::mul` is
`O(N log N)` and full of bit-reversal and twiddle detail that is easy to get
subtly wrong. Both are kept, and `ntt_tests` checks the second against the first
on random inputs.

*Rejected:* test vectors. A table of expected outputs can only ever encode what
the implementation produced the first time it was run, which is worthless when
that implementation is the thing under test.

Note that this is the opposite discipline from invariant 2, and deliberately so.
Here two implementations *should* be compared because one is independently
trustworthy. There, two implementations must not share code because neither is.

### A ciphertext carries no noise estimate

*Rejected:* a `noise_bound` field on `Ciphertext`, updated by every operation.
It would have been convenient, it is what several real libraries do, and it
would have quietly destroyed the project.

A ciphertext genuinely does not know its noise. Tracking an estimate inside the
evaluator means the phase-2 analysis and the phase-0 evaluator share an
implementation — and then phase 5's differential harness compares a thing to
itself. Measurement lives in `noise.rs`, requires the secret key, and is labelled
as a laboratory instrument.

This is invariant 2 in its most concrete form, which is why it is written into
the module docs rather than only here.

### `measure` takes the expected plaintext as an argument

It cannot be inferred, and the reason is the trap the whole project is about.
Deriving the expectation by decrypting would measure the noise of whatever the
ciphertext happens to say — which is bounded by `Δ/2` by construction, no matter
how badly the budget was blown. The measurement is only meaningful against the
value the computation *should* have produced.

`bfv_tests::the_measurement_needs_the_expected_plaintext` asserts both halves of
this, so the trap is documented as an executable fact rather than a comment.

### Rescaling is split by Euclidean division

Found while choosing parameters. The obvious `(c·t + q/2)/q` overflows `i128`:
tensor coefficients reach `N·(q/2)² ≈ 2^123`, and `×256` needs `2^131`. In
release that wraps silently and yields a plausible wrong plaintext — the exact
failure mode this project exists to attack.

Splitting `c = quo·q + rem` keeps both products in range and is *exact*, not
approximate. See `docs/README_phase0.md` for the full account.

The process lesson mattered more than the fix: it was found by reasoning about
parameter bounds, not by a test. CI now runs the suite in debug as well as
release, so that overflow panics rather than wraps.

### Rounding is to nearest, not truncating

Truncation biases every coefficient in the same direction, and a systematic bias
accumulates over a multiplication chain in a way that random noise does not. The
`rem_euclid` split also makes the rounding uniform across signs, removing a
branch that would otherwise be easy to get asymmetrically wrong.

### Parameter validity is checked at construction and refuses

Every condition in `Params::validate` — degree a power of two, `q` prime,
`q ≡ 1 (mod 2N)`, `t` far below `q`, the tensor product fitting in `i128` — is a
bug that would otherwise surface as *quietly wrong plaintexts* rather than as an
error.

*Rejected:* warnings. A project whose subject is silent failure should not have a
category of "probably fine" parameters.

### The depth cliff was moved from 3→8 to 3→4

The roadmap promised garbage at depth 8. Not reachable without RNS: noise grows
10–14 bits per multiplication, so depth 8 needs `q ≈ 2^100`, and the integer
tensor product caps `q` near 59 bits.

The roadmap was corrected rather than the target quietly reinterpreted, and the
correction is marked in place. Two reasons to keep the marking: the ceiling *is*
the argument for RNS in phase 1, stated as an arithmetic fact rather than as
exposition; and the revised cliff is sharper, with a single multiplication
separating a correct program from a wrong one.

---

## To be decided (phases 1–2)

Restated from `docs/ROADMAP.md` §9 so that they are not lost:

1. What is the bound lattice — plain intervals, or something carrying
   correlation? Independent-noise assumptions are where worst-case bounds get
   loose, so this decides how interesting phase 5 is.
2. Is the noise bound a type or an effect? It flows along dataflow edges (type),
   but is *checked* at decryption (effect discharged at a boundary).
3. How much of parameter selection is inferred versus declared?
4. Is there an analogue of an SMT escalation, or is interval propagation the
   whole story?
5. Does the IR carry plaintext operations, giving the noise model a second axis?

One more, added by phase 0:

6. **Where does the saturation ceiling go in the phase-5 methodology?** Measured
   noise is only comparable against a predicted bound while the budget holds;
   past the cliff it pins to `q/2` and wanders. The harness needs to exclude
   blown runs rather than average over them, and that rule should be written
   down before any numbers are collected.
