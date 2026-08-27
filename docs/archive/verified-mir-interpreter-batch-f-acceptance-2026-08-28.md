# Verified-MIR Interpreter Batch F Acceptance — 2026-08-28

Batch F of the
[Verified-MIR superinstruction and basic-block interpreter plan](../verified-mir-superinstruction-basic-block-interpreter-plan.md)
is accepted across checkpoints `c4c629ce0`, `4ab756893`, `41d7cb8fc`,
`87793fb7d`, `91ef72504`, `dd92c09f3`, `ddd236f78`, and the closing
documentation checkpoint. The accepted Batch E physical model is unchanged;
this batch closes exact-generation ownership, reload/async/Service behavior,
portable corruption coverage, profiling, shared memory, and fuzzing.

## Generation and lifecycle proof

- One immutable `LinkedArtifact` owns selected superinstructions, scalar block
  plans, scalar loop metadata, sources, exits, coverage, and profile layout.
  Runtime construction at 1, 100, and 10,000 actors shares those plans instead
  of copying them into actor-local state; the actor-memory harness reports plan,
  source, loop, and profile-counter counts separately.
- Accepted reload publishes a new artifact and independent plan set. Rejected
  and staged updates leave the active owner unchanged. Active frames and
  retained closures finish on their old selected scalar loops while new roots
  use the new generation.
- Ready and pending ordinary async roots, async providers, Service async roots,
  detached Service workers, and safe-point continuations execute selected
  scalar loops through their already isolated Runtime and retain the exact
  artifact selected at admission.
- Service Snapshot, Delta, fold, rollback, portable activation, nested
  `service::base`, and nested `service::pinned` paths retain one complete
  generation. Selected plans add no Service dispatch authority.

## Profiling and portability proof

The immutable artifact profile layout classifies ordinary offsets,
superinstructions, scalar units, logical source subpoints, and scalar loops.
Opt-in generation execution data exposes saturating rows for:

```text
ordinary instruction hits
superinstruction hits and eliminated dispatches
scalar block entries and compact logical operations
scalar loop entries, iterations, exits, and charged backedges
```

Default execution allocates no instruction profile. Multiple Runtime images
share one aggregate profile for the exact generation, reset clears only its
mutable counters, and accepted reload creates a fresh profile. No profile data
rewrites selection.

Version 5 ordinary, Service, and deployment artifacts round-trip selected
scalar loops and checksums without source compilation or portable MIR. Existing
load, stage, and activation gates reject versions 1-4. Decoder and linker tests
reject checksum-valid malformed plan handles, coverage, range metadata,
registers, sources, targets, charges, task metadata, feature combinations, and
count limits transactionally.

`portable_plan` adds seeded libFuzzer mutations for plan handles, operand
ranges, coverage manifests, exits, source points, and payload limits. The
stable build gate compiles every fuzz binary, CI runs the target for five
minutes, and a local nightly smoke run completed 10,000 executions without a
finding.

## Validation

```bash
cargo test -p vela_bytecode --all-features
cargo test -p vela_vm --all-features
cargo test -p vela_hot_reload --all-features
cargo test -p vela_engine --all-features
cargo check --manifest-path fuzz/Cargo.toml --bins
cargo clippy --manifest-path fuzz/Cargo.toml --bins -- -D warnings
cargo +nightly-2026-07-27 fuzz run portable_plan -- -runs=10000
cargo fmt --all -- --check
git diff --check
```

Batch G owns the final structural audit, interleaved performance/guardrail and
embedded Lua 5.4 comparison, complete acceptance matrix, repository validation,
cleanup, and release decision.
