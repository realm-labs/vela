# Verified-MIR Interpreter Final Acceptance — 2026-08-28

The
[Verified-MIR superinstruction and basic-block interpreter plan](../verified-mir-superinstruction-basic-block-interpreter-plan.md)
is complete through Batches A-G. The production runtime remains one
generation-pinned register VM: verified MIR may select a bounded
superinstruction, scalar block, or scalar range-loop region, and every
ineligible operation continues through the canonical ordinary instruction
path in the same frame driver.

Batch G accepts the physical model established by Batches B-F. It removes no
semantic fallback and adds no production optimization switch, bytecode-
adjacency selector, second interpreter, JIT, compatibility reader, global plan
authority, or benchmark-specific production branch. Portable selected plans
use artifact version 5 because version 4 was already assigned before this
track; ordinary program, Service artifact, and Service deployment readers all
hard-reject versions 1-4.

## Release decision

The selected families are retained in production:

- verified-MIR i64 compare-immediate conditional branches;
- complete eligible Bool/i64 scalar blocks with bounded operation, exit,
  source, and charged-target tables; and
- proven-i64, single-entry, single-latch scalar range regions executed by the
  same focused scalar executor.

Selection is deterministic and consumes only the sealed `MirBackendHandoff`.
An independent verifier re-derives complete MIR statement, terminator, edge,
budget, source, liveness, root/safepoint, and CFG-exit coverage before physical
emission. Portable load uses only the bounded version 5 plan and never needs
MIR or reconstructs a plan from emitted instruction adjacency.

## Semantic acceptance matrix

| Area | Accepted proof |
|---|---|
| Selection and verification | Deterministic plans; exact one-to-one operation, terminator, edge, source, and budget coverage; malformed, duplicate, reordered, cross-function, invalid-exit, and unknown-fact cases reject or remain ordinary. The selector has no HIR/source-text or emitted-bytecode-adjacency query. |
| Scalar arithmetic and control | Immediate/register add, sub, mul, rem, comparisons, true/false exits, fallthrough, break, continue, return staging, empty/one-iteration ranges, later-iteration overflow, and malformed typed entry match ordinary execution. Earlier writes survive a later trap and later writes do not execute. |
| Budgets, traps, and limits | Entry, internal operation, terminator, selected edge, loop header, and backedge charges preserve canonical order and exact `MirBudgetSite` ownership. Exhaustion at zero, iteration N, and the final edge matches ordinary execution, including error kind, consumed units, and source span. Unbounded execution has no active per-operation budget branch. |
| GC and Host boundary | Selected units contain no allocation, safepoint, HostAccess, reflection, lease, or borrowed-view operation. Untouched live heap registers survive selected execution and later collection. Host mutation remains exclusively `HostRef`/`HostPath`/`PathProxy`/`HostAccess`; script GC never owns Rust host state. |
| Sources, debugger readiness, and profiling | Every compact operation and exit retains a logical source subpoint. Traps identify the same logical operation and frame as ordinary execution. Opt-in exact-generation profiles report ordinary hits, superinstruction hits/eliminated dispatches, block entries/logical operations, and loop entries/iterations/exits/charged backedges; counters never select or rewrite plans. |
| Hot reload and closures | Old active frames and retained closures finish on their old immutable artifact and selected plans. New roots use the newly published generation. Rejected or staged reloads do not change active plan authority or profile ownership. |
| Async and scoped tasks | Ready/pending roots, providers, detached workers, continuations, Runtime pooling, cancellation, and deadline boundaries retain the origin artifact and generation. Suspension remains outside selected regions, and reset clears mutable counters/state without copying plans. |
| Service generations | Snapshot, Delta, fold, rollback, portable activation, and nested `service::base`/`service::pinned` calls retain one complete generation. Selected plans add no Service dispatch or mutation authority. |
| Portability and corruption | Version 5 source-linked, portable, Service, and deployment round trips preserve selected plans and checksums without MIR. Versions 1-4 reject. Count/size/depth, handle, register, constant, target, charge, exit, source, profile, feature, and coverage corruption rejects transactionally. The `portable_plan` fuzz target completed 10,000 seeded executions. |
| Memory and ownership | Selected plans are immutable `LinkedArtifact` content shared across Runtime images and 10,000 actors. Block entry and 10,000 additional loop iterations allocate zero incremental bytes after warmup. Plans and profiles never become Runtime-global mutable authority. |
| Ordinary fallback | Dynamic or unknown facts, calls, heap work, HostAccess, reflection, state, tasks, await, try control, safepoints, multi-exit/multi-latch regions, and every unsupported operation execute as ordinary instructions through the same VM. |

## Final performance retention gate

The clean final candidate capture is
`perf-results/external_compare/20260827T161755Z-verified-mir-batch-g-final.txt`
at runtime checkpoint `65f51a3775c6166306925dd872579d662ad2decd`.
It used Rust 1.97.1 on an Apple M1 Max, 500,000 workload iterations, five
measured processes, and two warmups. The later acceptance-head changes only
repair example registration fixtures and the source unsafe-audit allowlist;
they do not change production interpreter code.

| Workload | Batch A mean ns | Final Vela mean ns | Improvement | Lua 5.4 mean ns | Vela/Lua |
|---|---:|---:|---:|---:|---:|
| `scalar_branch_loop` | 10,980,632,575 | 6,848,224,266 | 37.634% | 1,211,593,166 | 5.652x |
| `range_iteration` | 33,108,354,483 | 8,013,425,800 | 75.796% | 6,012,117,816 | 1.333x |
| `function_calls` | 36,475,609,558 | 36,283,115,224 | 0.528% | 5,347,262,042 | 6.785x |
| `recursive_countdown` | 11,251,672,383 | 11,571,068,525 | -2.839% | 2,011,980,999 | 5.751x |
| `float_math_loop` | 14,056,844,083 | 13,027,657,850 | 7.322% | 1,910,048,200 | 6.821x |
| Five-row geometric mean | 18,379,533,498 | 12,458,567,638 | **32.215%** | 2,722,931,984 | **4.575x** |

All workload checksums match Batch A and embedded Lua 5.4. The 32.215%
geometric-mean improvement exceeds the 25% suite gate; scalar and range exceed
their individual 35% gates. The final values are slightly slower than Batch
E's checkpoint but remain decisively inside the predeclared retention gates.
No benchmark result changes the semantic selector or creates a benchmark-only
runtime path.

The retained physical inventory is unchanged from Batch E: the scalar workload
has 38 static / 3,801 profiled outer dispatches; the range workload has 39 / 171
and two scalar range plans. The range profile still reports 18 loop entries,
2,176 internal iterations, 18 exits, and 2,176 charged backedges. Relative to
Batch A, profiled range dispatch is down 99.29% while the eliminated logical
work remains visible through generation-owned profile events.

The final compile/resource capture is
`perf-results/commands/20260827T170627Z-verified-mir-batch-g-compile-resources.txt`.
Against Batch A, five-row compile means change by -3.28%, -0.38%, +1.46%,
+0.26%, and -0.49%; peak RSS changes by +0.33%. Version 5 portable sizes are
2,387, 2,424, 2,331, 2,079, and 2,021 bytes, changes of +6.61%, +5.94%,
+1.30%, +0.68%, and -0.10%. The bounded growth is selected-plan, source,
coverage, exit, and profile-layout metadata; decoder count and payload limits
remain enforced.

## Guardrails and memory

The final VM guardrail capture is
`perf-results/commands/20260827T162958Z-verified-mir-batch-g-vm-guardrails.txt`.
Non-target interpreter rows remain within 1.7% of Batch A: script calls change
by +0.76%, direct closures by +1.61%, managed direct closures by -2.32%, host
access by -1.75%, host-field access by +0.73%, managed materialization by
+0.55%, and GC by +1.01%. Checksums and profile-count contracts match.

Final Engine captures cover interop, Service boundaries, async execution,
scoped tasks, actor memory, and actor concurrency. Stable comparable rows have
no unexplained regression above 5%. The quiet no-recompile async rerun is
`perf-results/commands/20260827T170653Z-verified-mir-batch-g-async-rerun.txt`;
sync, ready, pending-resume, scalar-reentry, and provider-async change by
+0.92%, +1.94%, +1.39%, +2.05%, and +0.20% from Batch A. The preceding quick
sample's isolated +5.32% ready-entry result did not repeat. The selected plan
cannot contain suspension or change the async boundary.

The 10,000-actor memory rows remain generation-shared: selection off measured
116,981,760 bytes versus 117,080,064 at Batch A (-0.08%); selection on measured
118,407,168 versus 117,522,432 (+0.75%). Ten-worker hot/cold concurrency rows
improve by 3.09%/1.71%. Scalar block entry and scalar loop iteration retain the
zero-incremental-allocation proofs from Batches D and E.

## Structural and safety audit

Repository searches and architecture tests find one production frame driver,
one scalar executor, no production selection toggle, no bytecode-adjacency
peephole, no global plan registry, no selector HIR/source query, no legacy
artifact reader, and no benchmark-specific production branch. The Batch G
module split keeps selection, portable plans, artifact I/O, and verification
ownership reviewable without changing behavior.

The scalar executor's only new unsafe boundary is its two private unchecked
fixed-register-slot helpers. Unlinked, portable, linked, source-link, and
physical-reference verification prove every index before entry; runtime value
tags and checked arithmetic remain canonical. The repository unsafe-boundary
audit includes this module and passes. `cargo-miri` is not installed for the
available `nightly-2026-07-27` toolchain, so Miri could not run; the explicit
unavailability is accepted alongside malformed-plan/type tests and all verifier
layers.

## Full repository validation

The acceptance head passes:

```bash
cargo fmt --all -- --check
cargo fmt --manifest-path examples/Cargo.toml -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo clippy --manifest-path examples/Cargo.toml \
  --all-targets --all-features -- -D warnings
cargo test --workspace --all-features --no-fail-fast
cargo test --manifest-path examples/Cargo.toml \
  --all-features --no-fail-fast
cargo test -p vela_package --test architecture --all-features
cargo test -p vela_host --test unsafe_boundaries --all-features
cargo doc --workspace --all-features --no-deps
cargo bench --workspace --no-run
cargo check --manifest-path fuzz/Cargo.toml --bins
node editors/vscode/scripts/validate-package.js
(cd editors/tree-sitter-vela && \
  npx --yes tree-sitter-cli@0.25.10 generate)
git diff --exit-code -- editors/tree-sitter-vela/src
npm --prefix site run test:syntax
npm --prefix site run test:docs
npm --prefix site run build
```

The final workspace test rerun and all 35 runnable example tests pass. The
portable-plan nightly fuzz smoke completed 10,000 executions without a finding.
Documentation generation, all benchmark compilation, extension packaging,
grammar regeneration, syntax/docs tests, and the site production build pass.

## Checkpoints

The track's implementation and verification were kept as small Conventional
Commit checkpoints. Batch G closes with the physical-plan module split,
restored recursive example registration fixtures, formatting repair, unsafe-
boundary audit update, this acceptance report, and the durable status/decision
checkpoint. Earlier Batch A-F acceptance reports retain detailed per-family
history, rejected-shape rationale, and capture provenance.
