# Executable Generation Phase 0 Inventory

Captured from base commit `9390cffc7` for the executable-generation plan.
This is migration evidence, not a second architecture contract.

## Compile And Construction Front Doors

- `vela_bytecode::compiler::compile_mir_roots` built and verified one
  `MirProgram` per root, emitted bytecode, then discarded MIR.
- Engine source/module/reload front doors returned `UnlinkedProgram` and called
  `Engine::link_program` independently.
- `ProgramVersion::{from_linked_program,from_linked_program_with_abi}` rebuilt
  `ProgramImage` and `ProgramProfile` while accepting an independently linked
  `LinkedProgram`.
- `RuntimeImage::{new,try_new}` independently built `ProgramImage`, linked, and
  rebased caches. `RuntimeImage::from_program_version` cloned both layouts and
  rebased again.
- Direct `ProgramImage::{from_program,from_parts}` and `LinkedProgram::new`
  constructors remain widespread in bytecode/VM tests and benchmark fixtures;
  production authority is concentrated in Engine runtime/reload and
  `ProgramVersion` constructors.

## Cache And Profile Exhaustiveness

`CacheSiteKind` contains global read/write, record read/write, method call,
HostPath read/write/mutate/remove/call, and native call. `GlobalWrite` is
reserved and has no production emitter. Every emitted family must be covered
by the unlinked cache attachment classifier, linker conversion, linked and
unlinked bytecode verifiers, `ProgramImage` flattening/rewrite, and runtime
sidecar dispatch. The pre-migration manual RuntimeImage rewrite is incomplete
by construction because it is separate from instruction linking and relies on
debug function names; nested lambdas are absent from that name index.

Immutable layout today is split among per-code `CacheSiteLayout`, flattened
`ProgramImage` cache descriptors, `LinkedProgram`, and `ProgramProfile`.
Mutable state is already mostly runtime-local in `RuntimeState`:
`InlineCaches`, `RuntimeBytecodeProfile`, globals, script globals, and heap
execution state. The migration must make the linker the sole immutable layout
authority and key mutable cache/profile/hotness/tier sidecars by generation.

## Closure And Frame Execution

`ClosureCode::Linked` stores only `ScriptFunctionHandle`. Linked closure calls,
nested calls, iterator callbacks, method callbacks, and `CallFrame` execution
receive the current `LinkedProgram` separately. `RuntimeImage` can replace that
program while retaining the script heap, so a retained closure handle can be
resolved against unrelated new code. The target representation pairs every
dense handle and active frame with an immutable executable-generation owner.

## Frozen Regression Fixtures

- CFG shape join: `Left { x }` versus `Right { a, x }`, followed by `.x`.
- CFG immediate join: branch constants `2` and `100`, consumed in a loop.
- Callable forwarding: a dynamic `Function` parameter forwards a lambda into
  `Array::map`; exact linked closure arity has pass/fail cases.
- Nested cache sites: two lambdas repeatedly read distinct globals.
- Retained closure reload: host-retained old closure plus changed top-level and
  lambda handle layout; old and new entry results differ intentionally.
- Malformed MIR: dynamic typed operand, duplicate safepoint, invalid
  guard-success refinement, and an unsupported backend peephole precondition.
- Diagnostics: register overflow and unsupported record-pattern failures retain
  stable source spans.

## Phase 0 Measurement

The tracked release command is:

```bash
cargo bench -p vela_vm --bench external_compare -- \
  --runtime vela --iterations 10000 --repeats 3 --warmup 1 scalar
```

On 2026-07-11 with Rust 1.96.0 on macOS/aarch64 it produced
`scalar_branch_loop per_iter_mean_ns=8312`, `p95_ns=83937292`, and checksum
`3828494456532927350`. The final gate repeats the identical row and adds
compile/peak-memory, ProgramVersion, lambda-heavy, shared-runtime, hot-reload,
and retained-generation measurements.

The clean release `cargo check -p vela_bytecode --release` compile baseline on
the same machine was `5.14s` wall time with `277,463,040` bytes maximum resident
set size (`/usr/bin/time -l`).
