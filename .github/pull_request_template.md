## What

<!-- One or two sentences. -->

## Why

<!-- The reasoning, including what was considered and rejected. This is the part
     that will still be useful in a year — the diff explains itself, the reason
     does not. -->

## What this does to the depth cliff

<!-- The project's recurring artefact (docs/ROADMAP.md §4). Every substantive
     change should be able to answer this. "Nothing" is a fine answer; write it
     down rather than deleting the section, so that the question was asked. -->

## How it was verified

<!-- Actual commands and actual numbers, not "tests pass". -->

```
cd backend
cargo test --all && cargo test --release --all
```

## Checklist

- [ ] `cargo fmt --all` clean
- [ ] `cargo clippy --all-targets -- -D warnings` clean
- [ ] Tests pass in **both** debug and release
- [ ] Docs updated if behaviour or reasoning changed
- [ ] No new coupling between the noise analysis and the noise instrumentation
      (`docs/ROADMAP.md` §3, invariant 2)
