# Working on nsc

Notes to self, mostly, but written as though for someone else — which is the
only way they stay honest.

## Branches and pull requests

`main` is protected. Nothing lands on it directly; everything arrives through a
pull request with green CI.

Branch names carry the phase, because this project is organised by phase and it
should be obvious from a branch list what stage a piece of work belongs to:

```
phase0/ring-and-ntt
phase0/bfv-textbook
phase2/noise-lattice
fix/scale-round-overflow
docs/roadmap-revision
```

One idea per pull request. The temptation with a solo project is to let a branch
accumulate everything done that week; resist it, because the PR description is
where the reasoning gets written down, and a PR that does four things has a
description that explains none of them.

## Commit messages

Subject line in the imperative, under ~70 characters, no trailing period. Then a
blank line, then prose — what changed, and more importantly *why*, including the
options that were rejected.

The body is worth more than the subject. A year from now the diff will still be
readable; the reason will not be, unless it was written down.

```
Fix i128 overflow in the BFV rescaling step

scale_round computed (c·t + q/2)/q directly. A tensor coefficient is
bounded by N·(q/2)², which at the phase-0 parameters is ≈2^123; times
t = 256 that needs 2^131 and wraps silently in release builds.

Splitting c = quo·q + rem by Euclidean division keeps both products in
range and is exact rather than approximate.
```

Reference an issue when there is one. Do not reference a PR number in the commit
body — it is not known until the PR exists, and rewriting history to add it is
not worth the trouble.

## Pull request descriptions

The template in `.github/pull_request_template.md` asks for four things, and
the third is the one that matters:

1. **What** — a sentence or two.
2. **Why** — the reasoning, including alternatives rejected.
3. **What this does to the depth cliff** — every substantive PR should be able
   to answer this. If the answer is "nothing", say so explicitly; it is a
   perfectly good answer and it forces the question to be asked.
4. **How it was verified** — actual commands, actual numbers.

## CI

Five jobs, all required: `rustfmt`, `clippy`, tests in **both** debug and
release, the depth-cliff demo, and `rustdoc`.

Debug *and* release is not belt-and-braces. Debug panics on integer overflow;
release wraps silently. This project is about arithmetic that goes wrong without
saying so, so a suite that only ran under one of those profiles would be blind
to precisely the interesting failure. The overflow in `scale_round` was found by
reasoning about parameter bounds rather than by a test — the debug job is there
so that next time it is found by a machine.

Warnings are errors, everywhere. A repository that tolerates warnings
accumulates them until nobody reads the output.

## Before opening a pull request

```sh
cd backend
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test --all              # debug: catches overflow
cargo test --release --all    # release: needed, the reference paths are quadratic
cargo run --release --bin depth-cliff
```

## Things that are not negotiable

**The noise analysis must never share code with the noise instrumentation.**
`backend/nsc-core/src/noise.rs` says this at length and it is the one
architectural rule worth being rigid about. From phase 5 the project's main
empirical result is the gap between predicted and measured noise; two
implementations that share a helper drift together, stay mutually consistent,
and make the comparison meaningless. See `docs/ROADMAP.md` §3, invariant 2.

**Tests assert properties, not recorded outputs.** A test that compares against
a table of values only encodes whatever the implementation produced the first
time it ran. Where a fast implementation exists, check it against a slow obvious
one (the NTT against schoolbook multiplication). Where a property holds, assert
the property.

**Numbers in prose are numbers that were run.** Every figure in the docs should
be reproducible by a command in the same document.
