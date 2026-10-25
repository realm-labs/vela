# Architecture

## Authority And Transition Status

Vela is a synchronous, interpreted dynamic language with reliable script hot
reload. [goal.md](goal.md) is the product roadmap; the
[runtime simplification plan](runtime-simplification-plan.md) owns removal scope.
This entrypoint and the active subsystem contracts describe the reduced baseline.

**A–D are implemented through D commit `226900e81`.** Service/Provider,
script async/tasks, caller-borrowed Rust data, transparent host paths and dependent
bindings are removed. The Rust boundary is existing synchronous owned-value
native APIs only: **no independent opaque-object runtime survived D**.
Replacement objects, new conversion APIs and a new re-entry model are separately
deferred, not removal prerequisites.

**A-E removal implementation and software acceptance are complete.** The
[acceptance report](archive/runtime-simplification-acceptance.md) records full
workspace/browser/build gates and provenance-checked installed-editor partitions.
Strict/native Input/Render coverage remains separate and is not claimed complete.
[progress.md](progress.md) owns current gaps; historical records do not reinstate
removed support.

## Retained Compile And Runtime Pipeline

```text
.vela source / ordinary package graph
    -> lexer / parser / lossless CST / AST
    -> resolver / HIR / semantic facts
    -> verified MIR / bytecode compiler
    -> linked artifact / immutable executable generation
    -> synchronous VM / call frames / script heap / GC / finite budgets
    -> script-owned results and synchronous owned-value native calls
```

- `vela_syntax` owns parsing. `vela_hir` owns source sets, module graphs and
  resolution. Source-set mode and function selection use the same source set;
  the bytecode compiler consumes built HIR, not source text or `vela_syntax`.
- `vela_engine` orchestrates source/file/directory/package compilation and
  registry-aware linking. `vela_hot_reload` consumes a `LinkedArtifact` and owns
  version construction, ABI/policy comparison and update generation—not parsing
  or HIR compilation. Ingestion/compile/link failures remain structured Engine
  errors; only linked ABI/policy outcomes enter staged reload reports.
- `vela_package` owns manifest/project IO shared by Engine/tooling: deterministic
  sources, package graphs, `PackageId + ModulePath`, ordinary imports/visibility,
  workspaces and path dependencies. No discovery/selection plugin runtime is needed.
- Execution uses one verified linked-bytecode interpreter. Unlinked bytecode is
  a compiler/verifier/linker/fixture format, not an alternate runtime.
- Source files use `.vela`; portable bytecode-only artifacts use `.vbc`.
  Current versions are **portable 8 / Rust binding 7 / tooling 4**. Incompatible
  versions reject before payload use, linking or activation. There is no legacy
  runtime, inferred removed metadata or compatibility wrapper.

## Language And Runtime Ownership

Retain dynamic typing with optional contracts; functions/closures; script
records/enums/shapes; traits and dispatch; patterns/loops; collections/iterators;
and Option/Result. Builtin parameterized hints are not general script generics.
Ordinary modules/packages/traits do not depend on removed Service/Provider APIs.

An immutable generation owns code, linked handles, retained verified MIR,
source maps, cache/profile layouts and interpreter physical plans. Runtime-local
mutable data owns the script heap, roots, VM `state` and active synchronous
execution. Multiple Runtimes may share an artifact without sharing mutable
script state. Script GC is non-moving and never owns Rust host state.

Frames, closures, retained values and live-generation state remain rooted.
Execution-unit, memory, call-depth and collection-growth budgets, native-call
accounting, permissions, cancellation/deadlines and source-spanned errors remain.
Rust hosts own external scheduling; they can finish work and synchronously call
Vela with owned inputs. LSP workers/cancellation are not script scheduling.

## Reduced Rust Boundary

| Surface | Supported contract |
|---|---|
| Existing value conversion | Produces script-owned data; serialization need not use JSON text. Script edits do not implicitly mutate Rust sources. Rust explicitly receives/decodes results. |
| Synchronous native call | Existing owned-value arguments/results and typed/context-aware registration remain. Rust may borrow internally for one invocation, never as a script value, view or return. |
| Retained script value | `VelaValue` roots script data in its creating Runtime, with type, identity, lifetime and runtime-affinity checks. It is not a Rust-object handle. |
| Effects and permissions | Native host-read/write effects remain, alongside other retained capabilities and reflection policy. Owned-value transport does not imply purity or weaken authorization. |

No independent opaque-object runtime, Rust receiver methods, host constructors,
caller-borrowed roots/arguments/returns/views, retained leases/release,
transparent HostPath/PathProxy/HostTargetPlan access or dependent `extern state`
bindings remain. Reflection, aliases and wrappers cannot reconstruct them.
Ordinary VM-owned state and script field/collection access stay supported.

`vela_host`, TypeBinding, registry/value macros and conversion crates retain
independent owned-value responsibilities; names alone are not grounds for
whole-crate deletion. Retained type/identity/lifetime/permission checks still
apply. Existing synchronous native re-entry uses its active execution session;
no new re-entry architecture is approved.

Object storage/ownership/destruction, opaque identity/affinity, object reload and
reflection, serialized graph policy and new conversion/re-entry APIs are
[separate follow-up questions](runtime-simplification-plan.md#rust-boundary-and-deferred-follow-up).
Internal graph-copy helpers do not establish those future public contracts.

## Script Hot Reload And Exact Generations

- `ProgramVersion` and its linked artifact identify an immutable executable
  generation. Stable semantic names/IDs compare ABI; dense handles, state indexes,
  cache/profile sites and plan offsets are valid only with their exact owner.
- New public entries use the active generation. Frames pin their entry owner;
  closures retain their creation owner and handle. Old nested script/native
  dispatch uses that owner, never a similarly named active declaration.
- Accepted safe-point activation changes later entries only. It does not rebase
  old closures, bytecode, caches, counters or plans. Rejected/stale candidates
  leave the active image and state unchanged.
- Compatible state cells survive without rerunning initializers or top-level
  side effects. Added state is staged; incompatible contracts reject. Old
  closures can read state removed from the new image through their old maps.
- Code/state/plan reclamation follows live owners. Removed closure-valued state
  must not retain its own generation indefinitely. GC roots and generation
  liveness obey the same ownership rules.

This is script reload, not Rust Service hotfixing, Provider deployment,
structural state migration or host-object migration.

## Reflection, Tooling, And Interpreter Optimization

Reflection retains script metadata and permitted value reads/writes/calls.
Registry structure is immutable: no monkey patching, runtime field addition or
opaque-layout/borrowed-child/path recovery. Metadata/native reflection still
honors effects, capabilities and reflection permissions.

Retain CLI/browser-WASM execution, `vela_analysis`, platform-independent
`vela_language_service`, native `vela_lsp_server` and thin editor clients.
Queries use static schema facts and snapshots, never live script/host execution.
Native workers, request cancellation and incremental analysis remain. Debugger
runtime hooks/DAP are planned retained work, not a completed acceptance claim.

Verified MIR, guard refinements, liveness/root and budget analyses, linked
operands, caches, scalar blocks, superinstructions, source points and physical
profile layouts stay when used by the interpreter. Optimized paths preserve
logical charges, roots, errors, effect ordering and exact-generation ownership.
There is no JIT/Cranelift roadmap or JIT-only preparation requirement.

## Removal Dependencies And Verification

A–D removed the feature families together with dependent declaration,
compilation, dispatch, artifact and registration surfaces. The
[removal checklist](runtime-simplification-plan.md#removal-checklist) owns the
inventory. E completed dead metadata/dependency cleanup and the
software gates, including tooling/site/examples/bench/fuzz build audits; see the
[acceptance report](archive/runtime-simplification-acceptance.md).
Strict/native Input/Render coverage remains incomplete in
[current progress](progress.md). Format rejection belongs with the format
change, not a later documentation claim.

A retained baseline pass is not proof of absent APIs. Boundary rejection tests
and export/dispatch/artifact audits complement retained tests. Never delete
safety tests merely to obtain green gates. Preserve unrelated accepted editor
evidence, but recapture affected evidence before claiming current acceptance.

## Existing Retained Baseline Tests

| Existing source | Retained proof |
|---|---|
| [Engine native compiler tests](../crates/vela_engine/src/tests/native/compiler_options.rs) | `engine_installs_registered_native_functions_into_vm`: synchronous scalar native calls. |
| [Argument conversions](../crates/vela_engine/tests/args.rs) | Exact scalar tags, byte-buffer, tuple, Option/Result, set and map owned conversions. |
| [Ordinary packages](../crates/vela_engine/src/source/package/tests.rs) | `ordinary_package_imports_public_dependency_function`, `ordinary_dependency_body_reload_updates_root_package_calls`, ABI rejection without image advance. |
| [Runtime state](../crates/vela_engine/src/tests/runtime_args/managed_values.rs) | `runtime_state_decl_persists_vm_owned_value_and_rust_updates`. |
| [Engine reload](../crates/vela_engine/tests/hot_reload.rs) | `retained_closure_pins_old_generation_across_handle_layout_reload`, `retained_old_closure_reads_state_removed_from_active_generation`, `retained_old_closure_keeps_native_dispatch_and_rejected_reload_owner`. |
| [VM execution](../crates/vela_vm/src/tests/execution_core.rs) | Execution-unit charging, stop-before-next-instruction and recursive depth limits. |

[validation.md](validation.md) owns focused and checkpoint commands. Report only
commands actually run. [Current progress](progress.md) and the
[acceptance report](archive/runtime-simplification-acceptance.md) record completed
A–E software gates and the remaining strict/native Input/Render limitations.

## Implementation Architecture Hygiene

Keep focused crate/module responsibilities and ordinary source/test files under
1200 lines unless a reviewed exception is listed in
[the ledger](architecture/file-size-exceptions.md). Prefer `mod` boundaries,
`include!` only for generated code, justified `#[path]` exceptions and explicit
production imports. Remove obsolete internal APIs instead of aliases or duplicate
paths that preserve removed callers. This does not weaken ABI or format checks.

## Subsystem References And Old Implementation Contracts

Active reduced contracts:

- [Language](architecture/language.md), [primitive hints/guards](architecture/primitives-type-hints-and-guards.md),
  [dynamic dispatch](architecture/dynamic_method_dispatch.md), [identity](architecture/clean-identity-refactor.md).
- [Runtime/GC/bytecode](architecture/runtime.md), [hot reload](architecture/hot-reload.md),
  [reflection](architecture/reflection.md), [native registration](architecture/host-and-registration.md),
  [stdlib/embedding](architecture/stdlib-and-embedding.md), [ordinary packages](packages-and-providers.md),
  [Rust interop](rust-vela-interop.md).
- [Tooling/security/testing](architecture/tooling-performance-security-testing.md),
  [native LSP](architecture/lsp.md), [performance](performance.md), [validation](validation.md).

Old execution plans and the [Service model](architecture/rust-vela-service-model.md)
are explicitly labeled historical in place to preserve relative links/anchors.
Their old `accepted`, `future`, `/goal` and milestone text is not current support
or permission to resume removed features. Retained collector/language/interpreter
tracks need current scoped authorization rather than replaying mixed old plans.
The [archived goal](archive/goal-before-runtime-simplification.md) and old
subsystem acceptance reports preserve prior evidence only. The current
[removal acceptance report](archive/runtime-simplification-acceptance.md) records
completed A–E software gates; [progress](progress.md) distinguishes remaining
strict/native Input/Render coverage from that acceptance.
