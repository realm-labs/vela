# Tooling, Performance, Security, And Testing

This is the retained synchronous interpreter contract after A–E. Removal
implementation and software gates are complete; [current progress](../progress.md)
and the [acceptance report](../archive/runtime-simplification-acceptance.md)
record 4,333 workspace tests passed and 27 installed checks passed in
same-provenance partitions. The aggregate npm run timed out; strict/native
Input/Render coverage remains incomplete, not certified by those partitions.

## Executable-Generation Regression Matrix

Exercise new entries on new code and retained closures on exact old code,
including nested native/script dispatch, old-only state, cache/profile slots and
physical interpreter plans. Rejected/stale reloads leave active ownership
unchanged. Compatible state survives without rerunning initializers; reclamation
must not leave self-retaining old state generations. See [hot reload](hot-reload.md).

## Diagnostics

Parser, resolver, analyzer, verifier, linker and runtime errors retain source
spans and structured codes. Diagnostics should identify the failed operation,
expected/actual contract, and call stack when available. Source maps and debug
names are generation-owned side tables, not runtime string dispatch keys.
Permission and budget failures must remain observable on optimized paths.

## IDE And LSP Readiness

Use lossless syntax, deterministic semantic identities, reusable analysis facts,
static native/value schemas and immutable workspace snapshots. The native LSP
server owns transport/workers; `vela_language_service` owns editor-neutral
queries. Thin editor clients must not execute programs for hints. Worker pools,
cancellation and background indexing are retained tooling—not script async.

Keep modules/packages, script fields/methods/traits, collections, reflection,
source overlays, Unicode ranges, formatting and degraded-query behavior usable.
Removed syntax and obsolete schema versions need rejection/no-advertising
coverage. See [LSP](lsp.md) and the [test strategy](../lsp-test-strategy.md).

## Debugger Architecture

Runtime debugger hooks and DAP remain a **planned** track. Share source points,
frame/local availability and schema display data through explicit structures,
not live LSP execution. Breakpoints, stepping and inspection must respect
finite budgets, GC roots, permissions and exact-generation owners. Selected
physical plans need logical source locations and exits back to ordinary
instructions. Debugging does not require a JIT or suspended script futures.
This document does not claim debugger implementation or acceptance.

## Performance Architecture Contract

Measure parsing/compilation/linking/loading separately from loaded repeated
execution and reload activation. Retain interpreter-used verified MIR, guard
refinements, liveness/root and logical budget analyses, linked operands,
inline caches, scalar blocks, superinstructions and physical profiles.

Optimizations preserve generic-path results, checked errors, effect ordering,
source spans, roots, budget charges and old-generation lifetime. Contract guards
trap; specialization/cache guards fall back to equivalent ordinary behavior.
Use measured workloads and checksums, not removed Rust-object/path workloads.
There is no Cranelift/JIT milestone or JIT-only preparation requirement.

The retained optimization loop is measurement, a focused interpreter change,
regression proof and before/after measurement. Execution-lane caches or new
specializations require evidence; do not duplicate semantic/runtime layers for
speculative performance. Live harnesses and report policy are in
[performance.md](../performance.md).

## Security And Sandbox

Native host-read/write effects, I/O, event, reflection, random and time capabilities remain
meaningful even when arguments/results are owned values. Engine grants, call
policy, package requirements and reflection authorization are independent checks.
Do not silently erase requested capabilities or bypass checks through dynamic
calls, generated bindings, reflection or synchronous native re-entry.

Script-owned snapshots have no implicit Rust write-back. Rust state is outside
script GC; no caller borrow, host path, lease, opaque replacement or extern-state
binding may cross the boundary. Filesystem natives remain explicit opt-in with
sandbox-relative paths and escape rejection.

Use finite execution-unit, memory, call-depth and collection-growth limits for
untrusted programs. Charge before associated effects and account runtime-sized
iterator/callback/guard work. Cancellation/deadlines constrain synchronous
execution; native Rust remains trusted and must bound its own blocking work.

## Testing Strategy

- Unit tests cover syntax, resolution, contracts, MIR/bytecode verification,
  identity, conversion, collection semantics, reflection policy and budgets.
- Integration tests exercise the complete linked synchronous slice, ordinary
  package/dependency reload, compatible state and exact old closures.
- Negative tests prove removed syntax/APIs/registrations and obsolete executable,
  binding and tooling formats cannot execute; checksums do not replace validation.
- CLI/browser and runnable owned-value examples cover actual supported entrypoints.
- LSP tests distinguish service, protocol, installed-provider, command, input and
  rendered-output evidence, and stale/cancelled generations.
- Bench/fuzz builds and selected stress runs cover physical plans, GC and codecs.

Do not delete tests just to obtain green gates. Move obsolete success expectations
with their removal while retaining safety/rejection coverage. Run commands from
[validation.md](../validation.md) and record exactly what ran. The
[acceptance report](../archive/runtime-simplification-acceptance.md) records
completed A–E software gates; [current progress](../progress.md) owns the separate
strict/native Input/Render gaps.
