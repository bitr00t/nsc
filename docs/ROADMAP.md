# nsc — Roadmap

*A from-scratch compiler for fully homomorphic encryption whose thesis is a
**noise budget type system**: a program that cannot be statically proved to
decrypt correctly is a compile error.*

Working name `nsc` (noise-safe compiler) — change it when something better
turns up; it appears in enough places that renaming later is a chore.

---

## 1. The thesis

Every FHE ciphertext carries noise. Fresh ciphertexts have little; addition adds
it roughly, multiplication multiplies it. Past a bound that depends on the
parameters, decryption **does not fail** — it returns garbage. No exception, no
error, no signal. The plaintext is simply wrong.

This is the same shape of bug as zkc's under-constrained circuit, in different
clothing:

- It does not show up in tests, because tests are small. Three multiplications
  work; twelve do not.
- It is silent. Nothing in the type system, the API, or the runtime says a word.
- The failure is total, not degraded. A wrong plaintext is not an approximation
  of the right one; in BFV it is uniformly meaningless.

So the thesis is the same move zkc made:

> A program whose noise budget cannot be statically proved to hold at every
> decryption point is a **compile error**, not a runtime surprise.

And the diagnostic should be the same kind of thing zkc's was — not a warning,
but a counterexample. The concrete operation sequence that overflows, with the
bound after every step and the step at which it crosses:

```
error: 'tally' exceeds its noise budget before decryption
  --> tally.nsc:14
     |
  14 |     decrypt(acc)
     |
     = the budget is 62 bits at these parameters; this path spends 71
     = the path, with the bound after each step:
     =     ct_in                    12 bits
     =     mul(ct_in, weights)      27 bits   (+15, multiplicative depth 1)
     =     add over 8 terms         30 bits   (+3)
     =     mul(., threshold)        45 bits   (+15, multiplicative depth 2)
     =     mul(., scale)            71 bits   (+26, depth 3 — over budget)
help: raise the parameters (N = 2^14 buys 43 bits, at 2.1x the ciphertext size),
      or insert a bootstrap before the final multiplication
```

Everything else in this document follows from wanting that error message to be
real.

---

## 2. What makes this a project and not an exercise

Four things, in decreasing order of how sure I am about them.

**The gap between the static bound and the actual noise is measurable.** This is
the big one, and it is why FHE beats the alternatives I considered. Noise bounds
are worst-case and famously loose. Unlike zkc — where determinacy either holds
or does not — here I can *instrument the real thing*: decrypt with the secret
key, measure the actual noise, and compare it against what the analysis
predicted. Every gap is a finding waiting to be explained. "My bound is 4x loose
on this circuit shape, and here is the reason" is exactly the kind of result
that made zkc worth writing about.

**There is a real optimization problem.** When the budget does not suffice, the
answer is to bootstrap — and bootstrapping is expensive, often by orders of
magnitude. *Where* to place bootstraps to minimise their number subject to the
budget holding everywhere is a genuine compiler problem, the analogue of zkc's
constraint fusion, and it has a shape (a min-cut-like placement on a DAG) rather
than being a bag of heuristics.

**Two schemes that genuinely disagree.** BFV (exact, integer) and CKKS
(approximate, fixed-point) have different noise behaviour, different rescaling
disciplines, and different failure modes. That is the FHE analogue of zkc's
R1CS-versus-Plonkish disagreement — the thing that makes a neutral IR *earn* its
existence rather than being an architecture diagram.

**The arithmetic is adjacent to what I already know.** RLWE lives in polynomial
rings mod q; NTT is the multiplication primitive; RNS/CRT decomposition handles
big moduli. That is the same corner of the world as Goldilocks and FFT, with
different constraints. Enough transfer to move fast, enough difference to learn
something.

---

## 3. The invariants

Two, to be held at every phase and *demonstrated* rather than asserted. zkc's
worked because each had a concrete falsifiable test attached, so these get the
same treatment up front.

**Invariant 1 — the Core IR is scheme-agnostic.**
It is a typed dataflow graph over encrypted values, not a BFV circuit in
disguise. *Falsifiable by:* lowering the same IR to both BFV and CKKS and
getting the same plaintext results, with the noise analysis parameterised over
the scheme rather than special-cased.

**Invariant 2 — the noise analysis is a separate artefact from the evaluator.**
The static bound must never be computed by running the thing. *Falsifiable by:*
the differential harness — analysis on one side, instrumented evaluation on the
other, never sharing code. If they ever share a helper, the comparison is
worthless, and the temptation to share one will be strong.

A note on why invariant 2 is stated so aggressively: in zkc the equivalent
mistake was writing `deep_batch` twice, once for the prover and once for the
verifier. Two agreeing implementations of the same idea drift together and stay
self-consistent while both leave the protocol. Here the failure mode is the
mirror image — the analysis and the evaluator must *not* share code, precisely
so that their disagreement means something.

---

## 4. The recurring artefact

zkc had the phase-0 forgery: `IsZero` with `inv = 0`, `x = 5`, `out = 1`. One
concrete object, carried through every phase, and every new layer was tested to
reject it. It was worth more than any amount of prose about soundness.

The equivalent here: **the depth-cliff program.** A program that computes
correctly at multiplicative depth 3 and silently returns garbage at depth 4,
under fixed parameters. Something small and legible — a chain of homomorphic
multiplications.

*(Revised during phase 0. The original text said "garbage at depth 8". Textbook
BFV without RNS cannot reach depth 8: the tensor product is computed over the
integers, its coefficients are bounded by `N·(q/2)²`, and `i128` therefore caps
`q` at around 59 bits — which buys about three multiplications. That ceiling is
not a disappointment, it is the phase-1 motivation stated as an arithmetic fact.
The cliff is also sharper this way: one multiplication separates a correct
program from a wrong one.)*

It gets built in phase 0, before anything else, and it is the thing the compiler
must refuse. Every phase is tested against it:

- Phase 0: it exists, and the garbage is demonstrable. **Done** — at depth 4 it
  returns 235 where the answer is 243, with no error raised anywhere.
- Phase 2: the analysis refuses it at depth 4 and accepts it at depth 3.
- Phase 3: the CKKS lowering refuses it too, for its own reasons.
- Phase 4: the bootstrap placer turns the depth-4 version into a program that
  compiles *and* runs correctly.
- Phase 5: the differential harness shows how loose the bound was on it.

If a phase cannot say what it does to the depth cliff, the phase is not
finished.

---

## 5. Architecture

Language choice mirrors zkc for the same reasons: the analysis is a tree- and
graph-manipulation problem that Haskell is good at, and the arithmetic is a
tight-loop problem that Rust is good at. It also means the boundary is a
serialised IR again, which turned out to be the single best structural decision
in zkc — it forced the IR to be a real artefact with a spec instead of an
in-memory convention.

```
  .nsc source
      │  parse, elaborate
      ▼
  Core IR ─────────── noise analysis (interval arithmetic over the bound lattice)
   (typed dataflow           │
    over encrypted           ├─ proved: every decrypt is within budget
    values)                  ├─ refuted: the overflowing path, with per-step bounds
      │                      └─ unknown: say so, never silently approve
      │
      ├─ bootstrap placement (only if refuted and bootstrapping is enabled)
      │
      ├──► BFV lowering  ─────┐
      └──► CKKS lowering ─────┴──► RNS/NTT engine ──► evaluation
                                        │
                                        └──► instrumented: actual noise measured
                                             against the static bound
```

### Crate and module layout

```
compiler/                     Haskell frontend
  src/Nsc/
    Syntax/                   lexer, parser, AST
    Core/                     elaboration, the typed IR
    Analysis/Noise.hs         the budget analysis — the heart of the project
    Analysis/Params.hs        parameter selection and the security lookup
    Placement/                bootstrap placement (phase 4)
    Emit/Json.hs              IR emission
    Diagnose.hs               the counterexample paths
  tests/Spec.hs

backend/
  nsc-core/                   crypto: rings, NTT, RNS, RLWE, BFV, CKKS
    src/ring.rs               R_q = Z_q[X]/(X^N + 1)
    src/ntt.rs, rns.rs
    src/rlwe.rs               keygen, encrypt, decrypt
    src/bfv.rs, ckks.rs       the two schemes behind one trait
    src/noise.rs              INSTRUMENTATION ONLY — measures actual noise.
                              Never imported by the analysis. See invariant 2.
  nsc-eval/                   the IR evaluator and the CLI tools

docs/
  ROADMAP.md                  this file
  DESIGN_DECISIONS.md         the arguments, including the lost ones
  CHECKPOINT.md               the resume-from-here snapshot
```

The `noise.rs` comment is not decoration. That file is the one place where the
project could quietly destroy its own main result, and the comment is there to
be seen by whoever is tempted.

---

## 6. Phases

Deliberately shaped like zkc's, because that shape worked: a spike that proves
the problem is real, a walking skeleton end to end, then the thesis, then
widening.

### Phase 0 — Foundation spike

Own ring arithmetic (`R_q = Z_q[X]/(X^N + 1)`), NTT, RLWE keygen/encrypt/decrypt,
textbook BFV without RNS. No compiler at all.

**Exit criterion:** the depth-cliff program exists and is demonstrably broken —
correct at depth 3, garbage at depth 4, same parameters, no error raised.
**Met.** See `docs/README_phase0.md`.

The point of doing this first is the point zkc's phase 0 made: build the bug
before building the thing that prevents it. It also front-loads the risk. If the
arithmetic is harder than expected, that is better known in week two.

### Phase 1 — Walking skeleton

Source → IR → BFV evaluation → decrypt, end to end, on something trivial. Naive
parameters, no analysis, no optimisation. A serialised IR with a schema
document, because that is the artefact everything else hangs off.

**Exit criterion:** `nsc run add.nsc` encrypts, evaluates, decrypts, prints the
right number.

### Phase 2 — The noise budget type system

**The thesis phase.** Everything before this is scaffolding.

Static noise bounds per operation, propagated through the IR as interval
arithmetic over a bound lattice. Three outcomes, never two: *proved*, *refuted
with a path*, *unknown*. The refutation carries the per-step bounds shown in §1
— that diagnostic is the deliverable, not a nicety.

Parameter selection belongs here too: given a program, choose `N` and the
modulus chain that make it fit, subject to a security level. That inverts the
usual relationship — the program constrains the parameters rather than the
parameters constraining the program.

**Exit criterion:** the depth-cliff program is refused at depth 4, accepted at
depth 3, and the refusal names the multiplication that crosses the line.

**Known risk:** *unknown* must not become a wastebasket. In zkc the decidable
core settled most cases and SMT took the tail. Here the analogue is unclear yet,
and if *unknown* turns out to be the common case, the phase has failed even if
it compiles. Watch this from the first week — the ratio of proved to unknown on
the example set is the metric that matters.

### Phase 3 — CKKS: the second scheme

Invariant 1 has not been tested until there are two lowerings. CKKS brings a
second budget — *precision*, since it is approximate — which the analysis must
carry alongside noise rather than as a special case.

This is where the neutral-IR claim either survives or does not. If CKKS forces
changes upstream of the lowering, the IR was never scheme-agnostic and the
document should say so.

**Exit criterion:** the same source, lowered both ways, agrees on results within
CKKS's declared precision; the analysis is parameterised over the scheme.

### Phase 4 — Bootstrapping and placement

The hard phase, and the one to be honest about. Split it:

**4a — placement, assuming bootstrapping exists.** Given a program that exceeds
its budget, find where to insert bootstraps to minimise their count subject to
the budget holding everywhere. Implementable and testable against a *stub* that
resets noise without doing the cryptography.

**4b — actual bootstrapping.** Substantially harder than anything in zkc.

Plan 4a as the real deliverable and 4b as a **marked boundary** in the sense
phase 5 of zkc used: documented explicitly, with an argument for why it does not
invalidate the other results. If 4b lands, excellent. If it does not, the
project is still coherent — and the marked boundary is what makes it so.

**Exit criterion (4a):** the depth-4 program compiles with bootstraps placed and
evaluates correctly against the stub.

### Phase 5 — The differential harness

The phase that produces the findings. Instrumented evaluation measures actual
noise; the static bound predicted something; the gap gets explained.

Expected outputs: a table of predicted-versus-actual across circuit shapes, and
at least two written-up explanations of *why* a bound is loose where it is.
Deliberately after phase 3, so the comparison covers both schemes.

**Exit criterion:** a `docs/noise-gap.md` with numbers and an argument, not just
numbers.

### Phase 6 — Tooling

Following zkc: an LSP surfacing the budget in hover text (*"63 of 71 bits spent
at this point"* is the kind of feedback that changes how one writes), a cost
profiler attributing runtime to source lines, and a standard library of common
kernels with their budgets proved.

### Phase 7 — Optional widening

Candidates, in rough order of appeal: **TFHE** as a third scheme with a
completely different noise model (programmable bootstrapping, so it stresses
invariant 1 hardest); **SIMD packing** as an automatic transformation with the
rotation cost in the model; **circuit-privacy analysis** — noise flooding as a
second, independent budget.

Do not plan this phase in detail now. zkc's later phases were better for being
shaped by what the earlier ones found.

---

## 7. Prior art, and the honest position on it

This is not virgin ground, and pretending otherwise would be the fastest way to
write something worthless.

- **Microsoft EVA** does automatic scale and noise management for CKKS. It is
  the closest prior work and should be read *before* phase 2, not after.
- **HECO** does layout/packing optimisation.
- **Porcupine** does synthesis for homomorphic kernels.
- **Concrete (Zama)** is the TFHE toolchain, with a crypto-parameter optimiser.

zkc was not virgin ground either — Picus and Ecne look for under-constrained
circuits, circom and halo2 long predate it. The value was in the from-scratch
treatment and in the *framing*: determinacy as a type system with
counterexample diagnostics, rather than a linter bolted on afterwards.

The same bet applies here, and the angle is worth stating precisely, because it
is what the write-up will be about:

> Existing tools **manage** noise — they insert rescales and choose parameters
> so that things work out. The framing here is that the budget is a **type**: it
> is proved or the program does not compile, the refusal is a counterexample,
> and the analysis is separate from and checkable against the evaluator.

Read EVA early enough to know where the angle actually differs. If after reading
it the answer is "it does not", that is worth knowing in month one rather than
month nine.

---

## 8. What this project is not

Written now, while it is cheap to be honest, because the equivalent section in
zkc aged well.

- **Not a proof assistant development.** Rigour means executable specifications,
  differential testing against instrumented evaluation, and mutation harnesses —
  not Coq or Lean. What gets verified is that *this* analysis bounds *this*
  evaluator's noise.
- **The analysis will be sound, not complete.** It proves, or it says it could
  not. *Unknown* is never silently *fine*. Same discipline as zkc, same reason:
  a checker that waves things through when unsure is worse than no checker.
- **Not fast, and not competing on speed.** A from-scratch NTT will lose to SEAL
  and OpenFHE by a wide margin. Speed is not the claim; the claim is the type
  system. Benchmarks should be honest about this rather than quietly choosing
  favourable shapes.
- **Not audited, not production.** The RLWE hardness assumption is the
  literature's; the engineering here is the compiler around it.

---

## 9. Open questions to settle in phase 0–1

Answers not needed yet. What is needed is that they are written down, so that
phase 2 does not silently pick one and forget it made a choice.

1. **What is the bound lattice?** Plain intervals, or something carrying
   correlation? Independent-noise assumptions are where worst-case bounds get
   loose, and loose bounds are the phase-5 material — so the choice here decides
   how interesting phase 5 is.
2. **Is the noise bound a type or an effect?** It flows along dataflow edges,
   which suggests a type; but decryption is where it is *checked*, which
   suggests an effect discharged at a boundary. This decides what the source
   language looks like.
3. **How much of the parameter choice is inferred versus declared?** Full
   inference is elegant and may produce parameters nobody can justify to a
   security reviewer. A declared security level with inferred everything-else is
   probably the honest middle.
4. **What is the analogue of zkc's SMT escalation?** Is there a residual
   question worth handing to a solver, or is interval propagation the whole
   story? If the latter, phase 2 is smaller than zkc's phase 3 and the schedule
   should say so.
5. **Does the IR carry plaintext operations at all?** Mixed plaintext-ciphertext
   arithmetic is common and much cheaper. If the IR knows which operands are
   encrypted, the noise model gets sharper — and the type system gets a second
   axis it must track.

---

## 10. First moves

1. Read the BFV paper and the CKKS paper. Write down the noise growth formulas
   by hand, in `docs/DESIGN_DECISIONS.md`, before writing code — they are the
   specification the analysis implements.
2. Read EVA (§7).
3. Build the ring, the NTT, and RLWE encrypt/decrypt. Test decrypt(encrypt(m))
   over random plaintexts and random keys.
4. Build BFV multiplication, naively.
5. **Build the depth cliff.** Make it break. Commit it.
6. Only then start on the compiler.
