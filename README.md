# nsc

A compiler for fully homomorphic encryption whose thesis is a **noise budget
type system**: a program that cannot be statically proved to decrypt correctly
is a *compile error*, not a runtime surprise.

**Status: phase 0.** The cryptographic layer exists; the compiler does not yet.
What phase 0 delivers is the bug — see below.

---

## The problem

Every FHE ciphertext carries noise. Fresh ciphertexts have little; addition adds
it, multiplication multiplies it. Past a bound that depends on the parameters,
decryption **does not fail**. It returns garbage.

No exception. No error. No sentinel. A perfectly ordinary plaintext that is not
the answer:

```
$ cargo run --release --bin depth-cliff

  parameters   N = 256, q = 288230376151736833 (59 bits), t = 256
  budget       49.0 bits

  the program  encrypt 3, then multiply by a fresh encryption of 3
               at depth d the answer is 3^(d+1) mod 256

  depth   noise   headroom   expected   decrypted
  ─────   ─────   ────────   ────────   ─────────
      1    17.9       31.1          9           9
      2    31.6       17.4         27          27
      3    46.3        2.7         81          81
      4    57.0       -8.0        243         235   ← wrong
```

At depth 4 the computation returns **235** where the answer is **243**. Close
enough to look plausible. Your tests, which are small, all pass. Production,
which is not small, is wrong.

## The thesis

That failure should be a compile error, and the diagnostic should be a
counterexample rather than a warning — the path that overflows, with the bound
after every step:

```
error: 'tally' exceeds its noise budget before decryption
  --> tally.nsc:14
     = the budget is 49 bits at these parameters; this path spends 57
     = the path, with the bound after each step:
     =     ct_in                    3 bits
     =     mul(ct_in, weights)     18 bits   (+15, multiplicative depth 1)
     =     mul(., threshold)       32 bits   (+14, depth 2)
     =     mul(., scale)           46 bits   (+14, depth 3)
     =     mul(., bias)            57 bits   (+11, depth 4 — over budget)
help: raise the parameters, or insert a bootstrap before the final multiplication
```

That message does not exist yet. Everything in the roadmap is in service of
making it real.

## Two invariants

Held at every phase, and *demonstrated* rather than asserted:

1. **The Core IR is scheme-agnostic** — falsifiable by lowering the same IR to
   both BFV and CKKS, with the noise analysis parameterised over the scheme
   rather than special-cased.
2. **The noise analysis is a separate artefact from the evaluator** — the static
   bound must never be computed by running the thing. Falsifiable by the
   differential harness: analysis on one side, instrumented evaluation on the
   other, never sharing code.

Invariant 2 is stated aggressively on purpose. From phase 5 the project's main
empirical result is the *gap* between predicted and measured noise, and two
implementations that share a helper drift together — staying mutually consistent
while both leave the truth. `backend/nsc-core/src/noise.rs` carries the warning
where someone would be tempted.

## Phases

| Phase | Content | Status |
|---|---|---|
| 0 | Foundation spike: ring, NTT, RLWE, textbook BFV, and the depth cliff | **done** |
| 1 | Walking skeleton: source → IR → evaluate → decrypt, plus RNS | |
| 2 | The noise budget type system — the thesis phase | |
| 3 | CKKS: the second scheme, and the test of invariant 1 | |
| 4 | Bootstrapping and placement (4a placement, 4b the real thing) | |
| 5 | The differential harness: predicted versus measured | |
| 6 | Tooling: LSP with budgets in hover, profiler, kernel library | |
| 7 | Optional widening — TFHE, packing, circuit privacy | |

See `docs/ROADMAP.md` for the reasoning, and `docs/README_phase0.md` for what
phase 0 found — including an `i128` overflow in the rescaling step whose symptom
would have been a quietly incorrect decryption.

## Build

```sh
cd backend
cargo test --all              # debug: integer overflow panics
cargo test --release --all    # release: the reference paths are quadratic
cargo run --release --bin depth-cliff
```

No dependencies. Not even `rand` — every experiment has to be reproducible from
a seed, because phase 5's result is a comparison of measured numbers.

## Layout

```
backend/nsc-core/       the cryptographic layer
  src/ring.rs           R_q = Z_q[X]/(X^N+1)
  src/ntt.rs            negacyclic NTT, checked against schoolbook multiplication
  src/bfv.rs            textbook BFV
  src/noise.rs          INSTRUMENTATION ONLY — read the module docs first
  src/demo.rs           the depth cliff, defined once
docs/                   roadmap, per-phase reports, design decisions
```

## What this is not

- **Not a proof-assistant development.** Rigour means executable specifications,
  differential testing against instrumented evaluation, and mutation harnesses.
- **The analysis will be sound, not complete.** It proves, or it says it could
  not. *Unknown* is never silently *fine*.
- **Not fast.** A from-scratch NTT will lose to SEAL and OpenFHE by a wide
  margin. Speed is not the claim.
- **Not audited, not production.** `N = 256` at phase 0 is nowhere near a real
  security level, and is documented as such.

## Prior art

Microsoft **EVA** (scale and noise management for CKKS), **HECO** (layout
optimisation), **Porcupine** (kernel synthesis), **Concrete** (TFHE, with a
parameter optimiser). This is not virgin ground and the roadmap says so. The bet
is on the framing: existing tools *manage* noise; here the budget is a **type**
— proved, or the program does not compile.

## License

See `LICENSE`.
