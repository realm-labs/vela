# Rust/Vela Interop Hard-Switch Implementation

Status: active execution plan.

This document implements the
[final Rust/Vela interop contract](rust-vela-interop-final-shape-hard-switch-plan.md).
That contract owns user-visible semantics, supported and rejected shapes, and
code examples. This plan owns the deletion map, phase order, verification
matrix, and closing gate.

The switch is intentionally incompatible. It adds no feature flag,
compatibility alias, dual opcode interpretation, legacy artifact loader, or
second Service dispatch path.

## 1. Typed `base` Dispatch

### 1.1 Current failure to delete

The current Service macro accepts some non-`'static` call-scoped Host
signatures but emits a `base call for a call-scoped opaque Host parameter`
runtime error. The hard switch deletes this partial-admission branch.

### 1.2 Final call path

```text
generated Rust Service caller
  -> pins one service generation
  -> creates root CallArgs and HostRef slots
  -> installs generated typed default thunks
  -> enters selected Vela method
       -> service::base::method(...)
       -> ServiceId + MethodId lookup
       -> validate exact argument ABI and complete lease set
       -> typed root-local reborrow
       -> call authored Rust default
```

Generation-global routing selects the target. Root-local typed invocation
authority supplies the concrete call-scoped Host reborrow.

### 1.3 Required runtime facts

The root execution retains:

- stable InteropTypeId and exact HostTypeId;
- compact HostRef slot and generation;
- shared/exclusive root capability;
- canonical object identity and borrow group;
- generated concrete reborrow vtable;
- source and parameter origin for diagnostics; and
- the pinned service generation.

No copied Vela alias carries these facts independently.

### 1.4 Unsafe boundary

Rust's standard `Any` cannot recover a non-`'static` concrete type. A private
unsafe erased-borrow module may reconstruct an invocation-scoped typed reborrow
only after:

- exact stable type identity validation;
- root and generation validation;
- atomic complete alias preflight;
- active shared or exclusive lease validation;
- generated thunk identity validation; and
- lifetime containment by the root call or awaited native future.

The pointer, erased token, and reconstructed reference never enter Vela Value,
GC storage, reflection data, persistent state, portable artifacts, or public
HostRef payloads.

## 2. Deletion And Modification Map

| Area | Required change |
|---|---|
| `docs` | Replace hybrid release claims; document strict/try release and typed `service::base` totality with examples. |
| `vela_hir` | Bind compiler-owned Service paths; delete contextual capabilities; preserve strict `host::release`; add `host::try_release -> bool`; tag scoped producer results. |
| `vela_analysis` | Expose View/MutView/iterator facts; remove “released after last use” facts and hints. |
| `vela_mir` | Delete release-liveness analysis; retain strict release and add authored `TryReleaseBorrowLease` returning Bool. |
| `vela_bytecode` | Delete release insertion; encode only authored strict/idempotent release; bump artifact semantics. |
| `vela_vm` | Execute both explicit release modes; check the full active resource table at every await. |
| `vela_host` | Retain group invalidation and parent/child checks; make try-release suppress only known expiry; add reviewed typed reborrow. |
| `vela_engine` | Register both release intrinsics; retain root teardown/origins; provide root-local typed Service invocation authority. |
| `vela_macros` | Generate complete typed `service::base` thunks; delete placeholder arms; reject incomplete signatures. |
| `vela_reflect` | Preserve scoped return facts; do not expose Service compiler paths or typed reborrow internals. |
| language service/LSP | Show scoped types, await errors, and explicit strict/try release actions without inventing `host::is_live`. |
| examples/tests/benches | Rewrite implicit-release dependencies; prove totality and no implicit release. |
| portable artifacts | Reject artifacts produced under old implicit-release semantics. |

The current deletion inventory is concrete:

- `crates/vela_mir/src/borrow_release.rs` computes the last-use and edge
  schedule, while `verifier/mod.rs` and `liveness.rs` carry it;
- `crates/vela_bytecode/src/compiler/mir_backend/core.rs` emits releases after
  statements, and `core/physical.rs` emits the scheduled physical/edge
  releases;
- `crates/vela_mir/src/builder/calls.rs` is retained because it lowers authored
  `host::release`; it must also lower `host::try_release` to a distinct
  Bool-producing operation;
- `crates/vela_hir/src/binding/syntax_binding.rs` and `scopes.rs` currently
  recognize contextual `base` / `services` receivers and reserve those local
  names; the hard switch replaces them with compiler-owned `service::*` paths;
- `crates/vela_bytecode/src/compiler/semantic_input/placements/service_calls.rs`
  currently diagnoses and lowers the old receiver shapes and must accept only
  the namespaced static paths;
- `crates/vela_engine/src/runtime/lifetime.rs` currently derives async-suspend
  safety by walking reachable live values and must instead query every active
  scoped group in the ExecutionHost; and
- `crates/vela_macros/src/service/dispatch.rs` contains
  `requires_opaque_host_dispatch` and both sync/async runtime placeholder arms
  that E4 deletes.

`ReleaseBorrowLease` remains the strict explicit MIR/bytecode/VM operation.
`TryReleaseBorrowLease` is added for authored `host::try_release`; it returns
`false` only when `expired_scoped_hosts` proves that the same root already
released the scoped group. The switch deletes automatic producers, not either
authored operation.

## 3. Implementation Phases

### E0 — Freeze the final contract

Deliverables:

- make the final contract and this plan active;
- reopen the Rust/Vela interop checkpoint in `progress.md`;
- record explicit release and typed `base` totality in decisions;
- inventory automatic-release producers and opaque-Host placeholders; and
- freeze focused baseline tests before deletion.

Gate:

```text
active docs do not claim that the known opaque-Host base path is complete
release categories and the root-cleanup exception are unambiguous
the deletion inventory names every automatic-release producer
```

### E1 — Delete compiler-driven early release

Deliverables:

- delete MIR release scheduling;
- delete bytecode statement and edge insertion;
- retain strict `host::release` lowering and VM execution;
- add `host::try_release -> bool` through HIR, MIR, bytecode, VM, Engine, and
  ExecutionHost;
- rewrite ordinary export tests to use explicit release; and
- prove dead locals and scope exit no longer unfreeze parents.

Gate:

```text
source audit finds no borrow-release schedule or automatic-release emitter
only authored host::release produces ReleaseBorrowLease
only authored host::try_release produces TryReleaseBorrowLease
root teardown still releases every retained group on every exit mode
```

### E2 — Make explicit resource facts complete

Deliverables:

- expose scoped View, MutView, and scoped iterator facts;
- reject discarded and unnameable scoped producer results;
- attach creation origin and parent information to diagnostics;
- update hover, signature help, completion, inlay hints, and code actions for
  both release operations without selecting one from inferred liveness; and
- document parent/child release order and alias-group invalidation.

Gate:

```text
authors can identify every value that requires release
discarded scoped results fail before execution
tooling never claims an implicit last-use release
tooling never inserts a separate liveness guard
```

### E3 — Make await deterministic

Deliverables:

- validate the complete active scoped-resource table before every await;
- remove proven-dead release exceptions;
- retain invocation-future and root Host lease behavior;
- report every blocking origin with an explicit release hint; and
- prove ready and pending futures obey the same pre-await rule.

Gate:

```text
an unreleased dead local still blocks await
explicit release permits await
root Host async calls and cancellation-safe RAII remain valid
```

### E4 — Complete typed `base`

Deliverables:

- add root-local generated typed default thunks;
- add the quarantined non-`'static` reborrow boundary;
- bind `service::base::method(...)` and
  `service::pinned::service_name::method(...)` as
  compiler-owned non-value paths;
- delete contextual `base.method` and `services.service.method` bindings without
  aliases;
- route sync and async `service::base` through typed thunks;
- route pinned cross-Service Rust defaults through the same path;
- delete `requires_opaque_host_dispatch` and placeholder errors; and
- reject incomplete methods during macro expansion or schema sealing.

Gate:

```text
non-static non-Sync call-scoped Host patches call sync and async service::base
patch -> service::pinned::other -> target patch -> target service::base works
alias, effect, capability, generation, and cancellation checks remain
production contains no admitted runtime unsupported Service branch
```

### E5 — Artifact, fixture, and repository acceptance

Deliverables:

- bump artifact semantics and reject old implicit-release artifacts;
- update the representative Service fixture and ordinary interop example;
- add release and base-dispatch benchmark rows;
- run focused, workspace, example, documentation, and structural gates;
- update active architecture and usage documents to current truth; and
- record one acceptance report.

Gate:

```text
the supported and rejected matrix has executable proof
old artifacts reject before activation
all repository validation gates pass
progress.md marks interop complete only after E0-E5
```

## 4. Required Test Matrix

### 4.1 Explicit release

| ID | Proof |
|---|---|
| ER-01 | proven last use does not release |
| ER-02 | lexical scope exit does not release |
| ER-03 | branch convergence does not release |
| ER-04 | register overwrite does not release |
| ER-05 | discarded scoped result is rejected |
| ER-06 | unnameable scoped chaining is rejected |
| ER-07 | explicit release invalidates every alias |
| ER-08 | distinct siblings release independently |
| ER-09 | parent release fails while a child is live |
| ER-10 | double release reports expired borrow |
| ER-11 | ordinary root Host release is rejected |
| ER-12 | root success cleans unreleased groups |
| ER-13 | root error and panic clean unreleased groups |
| ER-14 | cancellation and future drop clean unreleased groups |
| ER-15 | try-release of a live group releases it and returns true |
| ER-16 | try-release of an expired alias group is a false no-op |
| ER-17 | try-release preserves NotScopedBorrow |
| ER-18 | try-release preserves BorrowStillInUse and does not release children |
| ER-19 | strict release remains an error after try-release closed the group |
| ER-20 | a path-dependent early release converges through one try-release |
| ER-21 | try-release preserves invalid, forged, stale, and cross-root errors |

### 4.2 Await

| ID | Proof |
|---|---|
| AW-01 | live scoped View blocks await |
| AW-02 | live scoped MutView blocks await |
| AW-03 | dead-but-unreleased scoped group blocks await |
| AW-04 | strict or try release permits await |
| AW-05 | ready future does not bypass the check |
| AW-06 | pending future observes the same check |
| AW-07 | root Host argument survives awaited Rust call |
| AW-08 | cancellation releases awaited invocation leases |

### 4.3 Ordinary interop

| ID | Proof |
|---|---|
| OI-01 | owned structural Value round trip |
| OI-02 | shared Value temporary borrow |
| OI-03 | shared and exclusive Host argument preflight |
| OI-04 | field/path/index write-through |
| OI-05 | direct shared/exclusive borrowed return |
| OI-06 | Option/Result borrowed return |
| OI-07 | owned and borrowed collection conversion |
| OI-08 | dynamic/reflected scoped return uses the same explicit lifetime |
| OI-09 | generated Rust binding observes reload ABI |

### 4.4 Service totality

| ID | Proof |
|---|---|
| ST-01 | direct Rust default uses no VM |
| ST-02 | ordinary Vela selection executes |
| ST-03 | Vela selection calls same-Service Rust base |
| ST-04 | non-`'static` opaque Host calls sync base |
| ST-05 | non-`'static` opaque Host calls async base |
| ST-06 | pinned Rust Service call uses typed dispatch |
| ST-07 | pinned Vela Service enters target patch |
| ST-08 | target patch calls its own Rust base |
| ST-09 | exact borrowed terminal return restores Rust borrow |
| ST-10 | unsupported signature fails at macro/schema time |
| ST-11 | old root stays on old generation across await |
| ST-12 | cancellation and panic release Runtime and Host leases |
| ST-13 | locals named `base` and `services` work inside a Service patch |
| ST-14 | `service::base` and `service::pinned` cannot become values |
| ST-15 | old contextual receiver spellings fail during binding |

### 4.5 Structural audits

```text
no MirBorrowReleaseSchedule
no emit_automatic_release
no non-authored TryReleaseBorrowLease
no host::is_live intrinsic
no requires_opaque_host_dispatch
no "base call for a call-scoped opaque Host parameter"
no accepted base.method(...) contextual capability
no accepted services.service.method(...) contextual capability
no compatibility release mode
no old portable artifact acceptance
```

## 5. Final Review

Before closing the hard switch, all answers must be yes:

1. Can an author identify every scoped value requiring strict or try release?
2. Can no compiler phase release a scoped Host based on liveness or scope?
3. Does every await check the complete active scoped-resource set?
4. Does every root exit mode clean remaining resources?
5. Does every admitted Service method support Rust default, Vela selection,
   `service::base`, `service::pinned`, async completion, and result restoration?
6. Can a non-`'static`, non-`Sync` Host reach Rust `service::base` without `Any`
   or a script-visible reference?
7. Do ordinary exports and Services share TypeBinding and conversion rules?
8. Are unsupported shapes rejected before Engine construction or activation?
9. Are old artifacts rejected without compatibility interpretation?
10. Do examples release every retained borrow on ordinary success paths?
