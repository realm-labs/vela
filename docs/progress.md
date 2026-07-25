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
- S3 accepted: standard Rust type bindings, borrowed collection views,
  collection protocols, prepared host operations, and the phase-wide gate are
  complete.
- S4 accepted: generated Rust-only service contracts publish and pin one
  complete immutable generation with direct zero-VM Rust defaults.
- S5 accepted: sparse Vela implementations, exact-base Delta inheritance,
  lexical `base`, pinned cross-service calls, custom Values, host-backed
  collections, scoped borrowed returns, and atomic nested reborrow are
  validated in one mixed Rust/Vela generation.
- S6 active: authored async adapters and lifecycle/lease proof are complete;
  deployment metadata, handler migration, CLI/LSP service tooling, examples,
  benchmarks, and the phase-wide gate remain.

S3 provides recursive standard bindings; exact owned/shared/exclusive
View and MutView facts; scoped reborrow for borrowed collections; prepared
field, index, and key access; call-scoped Array, Map, and Set iterators with
frozen traversal structure and live prepared reads; terminal iterator fold and
collection; prepared Array searches; live read-only Array, Map, and Set
callback traversal, including Array and Map grouping; bounded collection
projections; complex child views with
exact nested identity and lifetime enforcement; and immediate write-through
for the implemented Array, Map, and Set mutations. User-defined Sequence,
MapLike, and SetLike adapters reuse the same protocol, traversal, callback,
budget, and mutation paths. Bulk clear/extend/retain operations preflight
budgets, conversions, and stale snapshots before mutation. The explicit
standard collection matrix covers owned round trips, shared reads and mutation
rejection, fixed mutable replacement and growth rejection, growable mutable
write-through, Bytes views, and distinct BTree/Hash ABI. The S3 exit
proof covers the complete element/key method surface, resumable traversal,
dense typed element methods, lease-aware dynamic caches, and target resolution
independent of element count. The generated Rust-only service generation
creates no `HostRef`, performs no VM entry, and allocates nothing after root
pinning when a method selects the Rust default.

S5 adds explicit internal service-call targets that are invisible to ordinary
source registration, one immutable dispatcher per published generation, and
same-session re-entry for `base` and pinned `services` calls. The acceptance
fixture constructs a custom `PatchCommand` Value in Vela, preserves a mutable
Vec identity through Rust defaults and Vela selections, proves immediate
write-through and old-root isolation, routes a Vela-selected scoped borrowed
return into another Rust service, and rejects duplicate exclusive aliases
before business Rust executes.

S6 now preserves ordinary authored Rust async traits while generating a hidden
object-safe dispatcher returning `Send` service futures. One actor-owned,
mutex-free Runtime slot is removed from its host context for the duration of a
Vela-selected call and restored on completion, cancellation, drop, or unwind.
The pinned dispatcher/artifact and complete host lease set survive suspension;
the fixture proves direct host write-through, awaited Rust `base`, isolated
actors, old/new-root generation behavior, and non-rollback of effects already
performed before cancellation or panic.

## Milestone Snapshot

| Milestone | Status | Current note |
|---|---|---|
| M0-M7 | Complete | Source-to-VM-to-HostAccess-to-reload vertical slice, budgets, managed heap, and GC roots are validated. |
| M8-M18 | Complete enough | Language, HIR, host bridge, reflection, stdlib, embedding, reload, diagnostics, examples, and benchmark foundations satisfy their checkpoints. |
| M19 | Complete enough | Remaining interpreter costs belong to later cache, layout, or backend work. |
| M19.5 | Complete enough | Cache-ready IDs, linked bytecode, profile ownership, and prepared host paths are validated. |
| M20 | Complete enough | Actor Runtime/cache ownership, lifetime, reload, and concurrency gates are accepted. |
| M20.5 | Queued | Resume editor-visible work after the service hard switch. |
| Rust/Vela service interop | S5 accepted; S6 active | Async adapters and lifecycle proof are complete; add deployment metadata, handler migration, tooling, examples, and benchmarks. |
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
- The former callable-level replacement implementation is absent. Generated
  `#[service]` and `#[service_set]` contracts provide sealed schemas, direct
  Rust defaults, whole-generation staging/publication, root pinning, and
  conditional rollback. Sparse Vela methods compile to stable hidden targets,
  bind to one verified artifact, and execute through generated Snapshot and
  exact-base Delta adapters with explicit Runtime authority. Delta inheritance
  rebinds all Vela targets to one artifact; explicit `RustDefault`, stale-base
  rejection, effect ceilings, failure-without-fallback, and rollback are
  covered.

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
| S3 | Accepted | Complete collection protocols, prepared traversal/method paths, and the phase-wide gate are green. |
| S4 | Accepted | Sealed schemas, complete signature closure, generated service sets, whole-generation publication, fixture migration, and the zero-VM/zero-HostRef/zero-allocation Rust branch are green. |
| S5 | Accepted | Snapshot/Delta, lexical `base`/`services`, mixed custom values and collections, and scoped reborrow are green. |
| S6 | Active | Async adapters and lifecycle proof are green; deployment, handler migration, tooling, examples, benchmarks, and the phase-wide gate remain. |
| S7 | Pending | Requires S6 deployment/tooling integration. |

S6 must now expose immutable bundle and dry-run deployment metadata, migrate
handler/rule/event examples entirely onto service contracts, feed service and
TypeBinding metadata into CLI/LSP surfaces, and replace the remaining examples
and benchmark rows before its full gate. A shorter Runtime-owned host
reclamation policy remains a post-S2 follow-up and is not a service
hard-switch blocker.

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

1. Add immutable Snapshot/Delta bundle metadata, exact checksums, load/build,
   dry-run staging reports, and deployment diagnostics.
2. Model representative handlers/rules/events only as generated service
   contracts and remove any remaining domain-specific replacement surface.
3. Expose service schemas and complete TypeBinding metadata through CLI/LSP,
   then replace service examples and benchmark rows.
4. Run the full S6 gate before starting S7 host-framework acceptance.

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
