# Progress

This file records current implementation truth, the active checkpoint, and the
remaining gaps. It is not a changelog.

Detailed progress before the 2026-06-01 compaction lives in
[archive/progress-full-2026-06-01.md](archive/progress-full-2026-06-01.md).
Completed execution plans and acceptance reports live under
[archive](archive/); routine implementation history belongs in Git.

## Current Focus

The active architecture focus is the
[Rust/Vela unified service hard switch](rust-vela-service-hard-switch-plan.md).
Rust hotfixing has one target model: generated Rust service contracts and
defaults, sparse Vela service implementations, and atomic publication of one
complete immutable service generation.

Phase status:

- S0 accepted: the migration inventory, executable fixture, and boundary
  baselines are frozen.
- S1 accepted: the callable-level replacement model is deleted without aliases
  or a compatibility path.
- S2 accepted: one sealed `TypeBinding` registry, compact root-local `HostRef`
  slots, prepared typed thunks, and allocation-free common-arity preflight are
  validated.
- S3 active: standard Rust type bindings, borrowed collection views, collection
  protocols, and prepared host operations are being completed.
- S4-S7 have not started. No generated service-generation API is accepted yet.

S3 already provides recursive standard bindings; exact owned/shared/exclusive
View and MutView facts; scoped reborrow for borrowed collections; prepared
field, index, and key access; read-only collection projections; and immediate
write-through for the implemented Array, Map, and Set mutations. The remaining
S3 exit work is:

- complex-element borrowed views and their identity/lifetime proof;
- remaining element/key methods and live or resumable traversal operations;
- remaining transactional bulk write-through operations;
- richer user-defined collection adapters; and
- prepared element-method, grouping, and traversal paths without runtime name
  or reflection lookup.

S4 begins only after the complete S3 gate is green. Its first accepted slice is
the generated Rust-only service generation; it must create no `HostRef` and
perform no VM entry when a method selects the Rust default.

## Milestone Snapshot

| Milestone | Status | Current note |
|---|---|---|
| M0-M7 | Complete | Source-to-VM-to-HostAccess-to-reload vertical slice, budgets, managed heap, and GC roots are validated. |
| M8-M18 | Complete enough | Language, HIR, host bridge, reflection, stdlib, embedding, reload, diagnostics, examples, and benchmark foundations satisfy their checkpoints. |
| M19 | Complete enough | Remaining interpreter costs belong to later cache, layout, or backend work. |
| M19.5 | Complete enough | Cache-ready IDs, linked bytecode, profile ownership, and prepared host paths are validated. |
| M20 | Complete enough | Actor Runtime/cache ownership, lifetime, reload, and concurrency gates are accepted. |
| M20.5 | Queued | Resume editor-visible work after the service hard switch. |
| Rust/Vela service interop | S2 accepted; S3 active | Close the collection-view and prepared-operation gate before service dispatch. |
| M21 | Not started | Debugger runtime hooks and DAP integration. |
| M22 | Not started | Cranelift JIT after interpreter, cache, and debugger contracts stabilize. |
| M23 | Not started | Release hardening, public documentation, validation, and performance targets. |

## Current Baseline

### Language And Runtime

- Vela uses lossless syntax, HIR, analysis facts, verified MIR, linked bytecode,
  and one production interpreter route.
- Functions, closures, records, enums, traits, pattern matching, loops,
  iterators, parameterized collections, Option/Result, and controlled
  reflection have executable coverage.
- Execution, memory, call-depth, and collection-growth budgets are enforced.
  Script objects use non-moving managed storage; Rust host state stays outside
  the script GC.
- `LinkedArtifact` is the sole production executable generation. Sync and async
  execution share one explicit frame driver, and old generations remain pinned
  across active or suspended calls.

### Host Boundary And Embedding

- Scripts mutate Rust-owned state only through `HostRef`, `HostPath`,
  `PathProxy`, `HostTargetPlan`, and call-scoped `HostAccess`.
- Host reads, writes, compound mutations, methods, permissions, generations,
  lease conflicts, retained borrows, and same-session re-entry are covered.
- One sealed `TypeBinding` model supplies stable identity, ABI, codecs,
  constructors, methods, fields, protocols, and owned/shared/exclusive
  representation facts to runtime, reflection, compiler analysis, and LSP.
- The former callable-level replacement implementation is absent. Until S4
  lands, Rust/Vela integration exposes ordinary exports and generated typed
  bindings but no Rust-logic hotfix API.

### Standard Library, Tooling, And Proof

- Arrays, maps, sets, strings, bytes, iterators, Option/Result, math, context,
  deterministic time, controlled random, stdio, and sandboxed filesystem
  helpers have runtime and analysis coverage.
- The native language service and LSP cover diagnostics, completion, signature
  help, hover/navigation, symbols, semantic tokens, references, rename, code
  actions, formatting, inlay hints, watching, cancellation, and schema reload.
- Runnable examples, conformance fixtures, fuzz targets, benchmark harnesses,
  and documentation provide end-to-end proof.
- Durable performance rules and current baseline summaries live in
  [performance.md](performance.md); detailed measurements live under
  [archive](archive/).

## Active Gaps

### Rust/Vela Service Hard Switch

The phase gates are authoritative:

| Phase | State | Blocking result |
|---|---|---|
| S0 | Accepted | None |
| S1 | Accepted | None |
| S2 | Accepted | None |
| S3 | Active | Complete complex views, remaining collection operations, richer adapters, and prepared traversal/method paths. |
| S4 | Pending | Requires S3 acceptance. |
| S5 | Pending | Requires the Rust-only generated service generation. |
| S6 | Pending | Requires the synchronous partial-service vertical slice. |
| S7 | Pending | Requires async/deployment/tooling integration. |

Do not start a public service dispatch surface to work around an S3 gap.
Service-signature traversal and service-generation pinning belong to S4-S6.
Compile-time View/MutView enforcement remains dependent on receiver-capable
expression and service-signature facts. A shorter Runtime-owned host
reclamation policy remains a post-S2 follow-up and is not an S3 blocker.

### Parameterized Container Contracts

The runtime supports nested Array/Map/Set/Iterator facts, recursive guards,
budgeted deep checks, value-keyed storage, compiler-owned mutator checks,
macro inference, serde/reflection preservation, ABI comparison, contract
stamps, and lazy iterator item guards. The remaining work is an explicit
acceptance audit against
[container-type-hints-plan.md](container-type-hints-plan.md) and
[value-keyed-map-set-plan.md](value-keyed-map-set-plan.md).

### M20.5 LSP Follow-Up

The native LSP baseline is accepted. Further work must name an editor-visible
failure or missing protocol proof. Known follow-ups are broader method/schema
call-site classification and suppression of future hints across dynamic `Any`
boundaries. This track remains queued behind the service hard switch.

### Deferred Tracks

- M21 debugger/DAP work waits for stable runtime debug contracts.
- M22 Cranelift JIT waits for M20/M21 close-out and consumes the verified
  MIR/linked-artifact contract.
- Typed scalar superinstructions require profile evidence and temporary-register
  liveness.
- Persistent host iterator handles require an explicit lifetime model.
- Persistence, replication, cross-Runtime sharing, structural state migration,
  async-frame migration, and initializer dependency reads remain out of scope.

## Validation

Every implementation commit runs the focused test for its changed behavior.
A phase acceptance checkpoint runs the repository and later-phase gates from
[the hard-switch plan](rust-vela-service-hard-switch-plan.md#5-validation-commands).
Use the relevant subset of [validation.md](validation.md) during implementation.
The phase-closing commit or acceptance report records the commands and final
result; a routine feature commit does not claim phase acceptance from focused
tests alone.

Miri remains unavailable on the installed stable Rust 1.97.1
`x86_64-pc-windows-msvc` toolchain. The erased-borrow boundary relies on its
focused lifecycle, async, lease/re-entry, and source-audit proof until that
changes.

## Next Up

1. Close one named S3 gap with focused behavioral and failure-path tests.
2. Re-run the complete S3 gate and record one acceptance checkpoint.
3. Implement the S4 Rust-only generated service generation.
4. Resume M20.5 only after the service hard switch or a newly prioritized
   editor-visible blocker.

## Update Rules

- Update this file only when current focus, phase status, supported baseline,
  validation expectations, or remaining gaps change.
- Do not append per-commit notes, method-by-method chronology, benchmark logs,
  or rejected candidates.
- Routine implementation commits should not modify this file or the execution
  plan.
- Keep accepted-phase detail in its acceptance report or Git history. Archive
  additional history only when Git is insufficient.
- Keep `Current Focus`, `Active Gaps`, and `Next Up` mutually consistent.
- Use one coherent Conventional Commit per independently verifiable behavior.
  Record focused validation in the commit body when it is not obvious; use one
  explicit checkpoint commit for a phase-wide validation result.
- Fold immediate fixups into their triggering change before shared integration
  when history has not already been published.
