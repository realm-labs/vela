# Progress

This file records current implementation truth, the active milestone, and the
remaining gaps. It is not a changelog.

Detailed progress before the 2026-06-01 compaction lives in
[archive/progress-full-2026-06-01.md](archive/progress-full-2026-06-01.md).
Completed execution plans live under [archive](archive/); newer implementation
history belongs in Git.

## Current Focus

The ordinary Rust/Vela interop path is accepted: Rust exports, exact lease
adapters, owner-frozen borrowed returns, generated typed Rust-to-Vela bindings,
and `NativeCallContext` sync/async re-entry use the shared execution path.
Post-implementation review reopened the optional replaceable-dispatch layer:
its current target-owned Runtime starts a second execution with fresh budgets,
can deadlock on same-Runtime nesting, lacks controller-owned generation proof,
performs incomplete ABI/effect validation, narrows returns to `VmResult<T>`,
resolves override targets at staging by string, and still exposes low-level
slot ceremony to business authors. The corrected status and closure tasks live
in [the unified plan](rust-vela-interop-model-plan.md#post-implementation-review-correction--2026-07-17)
and [the post-review report](archive/rust-vela-interop-post-review-2026-07-17.md).

The explicit state-storage hard switch is accepted through Batch G. Exact
qualified embedding types, linked nominal canonicalization, graph-preserving
budgeted reload staging, external-owner generation reclamation, and nested
initializer-call fingerprints have focused and workspace-wide proof. M20 cache
close-out is unblocked; M20.5 remains the next editor follow-up after M20.

The executor-neutral async implementation from Batches A-D is landed: Vela has
one explicit frame driver, scoped `Send` Runtime/native futures, direct typed
host leases, same-session NativeCallContext reentry, generation-pinned reload,
and one `call`/`call_async` target surface for functions, bound methods, and
providers. The 2026-07-13 baseline, zero-hit audit, and original acceptance
result remain recorded under [archive](archive/).

Post-implementation review on 2026-07-14 reopened final acceptance through
Batch E in [async-execution-model-plan.md](async-execution-model-plan.md).
Batch E is complete: dynamic reentry roots, exact shared/exclusive leases,
script-addressable `is_async`, focused VM session/resume/reentry ownership, and
one provider resolver are implemented without compatibility paths. Full
features, examples, benches, Rust docs, fuzz build, site gates, audits, and
performance/memory comparison passed; the result is recorded in
[the Batch E acceptance report](archive/async-execution-batch-e-acceptance-2026-07-14.md).
M20 cache close-out and the M20.5 LSP follow-up remain valid after the accepted
state-storage hard switch.

## Milestone Snapshot

| Milestone | Status | Current note |
|---|---|---|
| M0-M7 | Complete | The source-to-VM-to-HostAccess-to-hot-reload vertical slice, execution budgets, managed heap, and GC roots are validated. |
| M8-M18 | Complete enough | HIR, executable language surface, script metadata, host bridge, reflection, stdlib, embedding, reload, diagnostics, examples, and benchmark foundations satisfy their checkpoints. |
| M19 | Complete enough | The non-JIT interpreter and heap optimization checkpoint is closed; remaining measured costs belong to cache, value-layout, or later backend work. |
| M19.5 | Complete enough | Primitive scalars, bytes, type contracts, guard plans, linked bytecode, runtime profile ownership, and HostTargetPlan/HostAccess preparation are validated. |
| M20 | Ready to resume | State-storage acceptance is complete; resume the cache-family audit. |
| M20.5 | Queued follow-up | Resume concrete editor-visible follow-up after M20 close-out. |
| Rust/Vela interop | Partially complete; replaceable reopened | Ordinary generated interop is accepted. Optional replacement must close same-session execution, generation ownership, exact contract, return mapping, static linking, and business-macro integration gaps. |
| M21 | Not started | Debugger runtime hooks and DAP integration. |
| M22 | Not started | Cranelift JIT after interpreter, cache, debugger, and conformance contracts stabilize. |
| M23 | Not started | Release hardening, public documentation, validation gates, and performance targets. |

## Current Baseline

### Language And Runtime

- `.vela` source uses lossless Rowan syntax, Heavy HIR, analysis facts,
  verified MIR, linked bytecode, and one production interpreter route.
- Functions, closures, records, enums, traits, pattern matching, loops,
  iterators, tuples, unit, structured type hints, Option/Result propagation,
  value-keyed maps/sets, and controlled reflection execute through tested
  runtime paths.
- Execution-unit, memory, call-depth, and collection-growth budgets are
  enforced. Script heap objects use non-moving managed storage and GC roots;
  Rust host state is never placed under script GC.
- `LinkedArtifact` is the sole production executable generation.
  `ProgramVersion` and linked closures retain generation ownership across hot
  reload; no unlinked compatibility interpreter remains.
- Callable asyncness and explicit await/resume control flow are preserved from
  source through verified MIR and linked execution. Sync execution, awaited
  sync targets, and real Rust future suspension use the same explicit
  `ExecutionSession` frame driver. The outer future is scoped and `Send`, and
  registered async futures may borrow invocation state without being `'static`.

### Host Boundary And Embedding

- Scripts mutate Rust-owned state only through `HostRef`, `HostPath`,
  `PathProxy`, `HostTargetPlan`, and call-scoped `HostAccess`; scripts never
  receive real Rust `&mut T` references.
- Nested reads, writes, compound mutations, removals, indexed paths, host
  methods, permission checks, generation checks, and source-spanned failures
  are covered.
- Reflection can inspect registered metadata and perform permissioned
  reads/writes/calls, but cannot mutate runtime type structure or monkey patch
  types.
- Engine registration, native functions, derive macros, capability profiles,
  package graphs, service-provider discovery/selection, serde snapshots,
  runtime value handles, hot reload, and the initial C ABI surface are
  available.
- `PackageId + ModulePath` is the sole script module identity. Package/provider
  compilation and reload use sealed package/HIR snapshots and linked artifact
  metadata rather than parallel package-unaware paths.
- Ordinary Rust/Vela integrations use one deterministic compiler-owned binding
  schema and build-time generated typed Rust surface. Runtime strings and
  boundary wrapper values remain low-level dynamic escape hatches, not the
  primary call workflow.
- Optional `#[replaceable]` entries currently prove stable dense slots, an
  empty-slot fast path, immutable root-pinned selection, partial activation,
  rollback, and no fallback retry. Their target-owned Runtime execution and
  low-level authoring surface are provisional and are not the accepted final
  interop architecture.

### Standard Library, Tooling, And Proof

- Arrays, maps, sets, strings, bytes, iterators, Option/Result, math, context,
  deterministic time, controlled random, opt-in stdio, and sandboxed filesystem
  helpers have runtime and analysis coverage.
- The native LSP uses editor-neutral queries in `vela_language_service`, typed
  `lsp_server::Message` transport and projection in `vela_lsp_server`, and thin
  VS Code/Zed integrations. It covers diagnostics, completion, signature help,
  hover/navigation, symbols, semantic tokens, references, rename, code
  actions, formatting, inlay hints, file watching, cancellation, and schema
  reload.
- Runnable examples, conformance and diagnostic fixtures, a parser fuzz target,
  benchmark harnesses, and the documentation site provide end-to-end proof.
- Current performance rules and baseline summaries live in
  [performance.md](performance.md); detailed historical measurements live in
  [archive/performance-full-2026-06-06.md](archive/performance-full-2026-06-06.md).

## Active Gaps

### Rust/Vela Replaceable Dispatch Post-Review

Ordinary interop remains accepted. Optional replacement must complete
`F-REVIEW-1` through `F-REVIEW-7` and `G-REVIEW-1` through `G-REVIEW-2` in the
[unified plan](rust-vela-interop-model-plan.md#post-implementation-review-correction--2026-07-17).
The closure requires one session-aware invocation authority, inherited runtime
policy and remaining budgets, controller/layout-owned generations, statically
linked override targets, complete callable-contract validation, ordinary
return/error/borrowed-return mapping, and a p9-lattice-shaped business macro
fixture. Do not close it by adding a reentrant or process-global Runtime lock,
replenishing nested budgets, accepting capability projection as full ABI,
fabricating Rust references, or documenting manual slot indices as the final
business surface.

The currently green single-level activation, partial-delta, rollback,
empty-slot benchmark, and runnable examples remain baseline evidence. They do
not prove nested replacement or final acceptance.

### State Storage Acceptance

No state-storage correctness gap remains from Batches A-G. Runtime embedding
resolves canonical qualified types exactly, recursively validates and stamps
linked record/enum identities, and preserves those identities through
`set_state` and `update_state`. Reload copies added-state graphs with one shared
transaction budget while preserving aliases and cycles. Old generations are
pinned only by owners reachable outside inactive state roots and reclaim at an
ordinary safe point. Initializer reports traverse only their reachable script
call graph, including nested closure and parameter-default executables, with
recursive termination. The accepted contract and proof matrix live in
[state-storage-model-plan.md](state-storage-model-plan.md).

### Async Post-Review Closure

Batch E closed `ASYNC-ROOT-1`, `ASYNC-LEASE-1`, `ASYNC-REFLECT-1`,
`ASYNC-VM-MOD-1`, `ASYNC-PROVIDER-1`, and `ASYNC-DOC-1`. No async correctness
or final-acceptance gap remains. Named performance follow-up
`ASYNC-LEASE-PERF-1` belongs to M20: profile the measured owned-guard exclusive
lease cost without weakening exact state, safe Rust, scoped `Send`, or RAII.

Do not solve these with a permanent-root leak, an exclusive lease labeled
shared, reflection aliases, navigation-only source splits, duplicated drivers,
or provider-specific public execution methods.

### M20 Cache Close-Out

Existing cache or measured families include declared state, script record
fields, host access, native calls, linked method dispatch, dynamic method
dispatch, stdlib value methods, callbacks, strings/bytes, Option/Result, and
selected array/map/set paths.

A remaining task is valid only when it names one missing proof:

- coverage: a measured hot path has no cache entry;
- correctness: hit, miss, wrong-guard, fallback, reload, schema, or version
  invalidation proof is missing;
- measurement: interpreter-only, profile-only, and cache-enabled rows cannot be
  compared;
- decision: a flat or slower result has not been accepted, assigned to a named
  follow-up, or deferred.

Close-out requirements:

- Publish the cache-family audit before adding another family.
- Preserve generic fallback behavior, budgets, GC roots, HostAccess policy,
  reflection permissions, hot-reload ownership, schema invalidation, and
  source-spanned diagnostics.
- Compare cache rows against the correct baseline using `measurement_kind`,
  `delta_kind`, `measurement_summary`, and `cache_delta_summary`.
- Keep scalar, collection, string, call/callback, and host-boundary results
  separate. Lua 5.x remains the non-JIT comparison target for representative
  host-boundary workloads.
- Move representation-wide, value-layout, or backend changes to an explicit
  later milestone instead of expanding M20.

The completed executable-generation contract is recorded in
[archive/mir-executable-generation-architecture-plan.md](archive/mir-executable-generation-architecture-plan.md).
Its accepted scalar interpreter cost belongs to a named M20
instruction-selection follow-up or M22, not to a second execution route.

### Parameterized Container Contracts

The current implementation includes nested `Array<T>`, `Map<K, V>`, `Set<T>`,
`Iterator<T>`, Option, and Result facts; recursive runtime guards; budgeted deep
checks; value-keyed map/set storage; compiler-owned mutator checks; macro
inference; serde/reflection preservation; hot-reload ABI comparison; contract
stamps and invalidation; and lazy iterator item guards.

The remaining checkpoint is an explicit acceptance audit against
[container-type-hints-plan.md](container-type-hints-plan.md) and
[value-keyed-map-set-plan.md](value-keyed-map-set-plan.md). Do not reopen
string-only map keys or vector-scan set semantics. Object equality/order is
complete enough for M20: user comparison traits remain separate from
`ValueKey` container identity/equivalence.

### M20.5 LSP Follow-Up

The clean query/context/result/projection boundary, typed main loop, GlobalState
ownership, lifecycle handling, incremental overlays, workspace/schema reload,
authoring-core completion model, formatting, semantic highlighting, and
protocol coverage are the baseline.

Remaining work must name a concrete editor-visible failure or missing protocol
proof. Known follow-up areas are broader method/schema call-site
classification and suppression of future hint families across dynamic `Any`
boundaries. Do not restore raw JSON-RPC handlers, feature-local semantic
scanners, runtime execution, live host-state reads, or editor-owned analysis.

### Deferred Tracks

- M21 debugger/DAP work waits for stable source spans, frame maps, GC roots,
  budgets, HostAccess, reload, tooling, and conformance contracts.
- M22 Cranelift JIT waits for M20/M21 close-out and must consume the verified
  MIR/linked-artifact contract.
- Typed scalar superinstructions remain deferred until profile evidence and
  temporary-register liveness support a specific fused lowering.
- Persistent host iterator handles remain deferred until their lifetime model
  is explicit.

## Validation

Unified Rust/Vela interop passed its complete workspace, examples, docs, site,
benchmark-build, and fuzz-build gates on 2026-07-17. Post-review reruns of root
formatting, all-feature workspace clippy/tests, focused dispatch tests, and the
two runnable replacement examples also passed. Those gates establish a green
baseline, but the optional replaceable acceptance matrix was incomplete; its
original report is historical evidence rather than current completion proof.

State-storage Batch F's focused suites and original full acceptance gates were
green on 2026-07-15, but the second review showed that its regression matrix
was incomplete. The review baseline still passes formatting, workspace clippy,
focused state tests, and the full-feature workspace tests; Batch G must add its
five missing proofs and rerun the complete relevant gates before state-storage
acceptance is restored. The async Batch E acceptance remains recorded for
2026-07-14.

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features --no-fail-fast
cargo clippy --manifest-path examples/Cargo.toml --all-targets --all-features -- -D warnings
cargo test --manifest-path examples/Cargo.toml --all-features --no-fail-fast
cargo bench --workspace --all-features --no-run
cargo doc --workspace --all-features --no-deps
cargo check --manifest-path fuzz/Cargo.toml --bins
cargo bench -p vela_vm --bench baseline -- vm_state_read_write --quick
```

The Miri component is unavailable on the installed stable
`x86_64-pc-windows-msvc` toolchain; focused safe-Rust lease/reentry tests and
the workspace unsafe-code prohibition remain green. Documentation placeholder,
syntax-highlighting, Astro diagnostics, and static-site build gates also pass.

Use the relevant subset of [validation.md](validation.md) for each change.
M20 work also requires focused correctness tests for the touched bytecode,
runtime dispatch, host, or stdlib path and the matching
interpreter-only/profile-only/cache-enabled benchmark rows.

## Next Up

1. Complete state-storage Batch G and restore its final acceptance.
2. Schedule and close the Rust/Vela replaceable-dispatch post-review tasks
   before restoring unified-interop completion.
3. Resume the M20 cache-family audit against the accepted state model.
4. Resume the M20.5 editor-visible follow-up after M20 close-out.
5. Keep persistence, snapshots, replication, cross-Runtime sharing, structural
   state migration, async-frame migration, and initializer dependency reads as
   explicit non-goals.

## Update Rules

- Update this file only when the current focus, milestone status, supported
  baseline, validation expectation, or remaining gaps change.
- Do not append per-commit notes, benchmark logs, implementation chronology, or
  rejected candidates.
- Keep active status concise. Put durable historical detail in
  [archive](archive/) only when Git history is insufficient.
