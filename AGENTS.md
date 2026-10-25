# Agent Instructions

This repository implements a synchronous, interpreted, Hot Reload First dynamic
scripting language in Rust for domain-neutral host business logic. Treat
[docs/goal.md](docs/goal.md) as the product roadmap,
[docs/architecture.md](docs/architecture.md) as the technical contract, and
[docs/runtime-simplification-plan.md](docs/runtime-simplification-plan.md) as the
removal-only checkpoint plan. These supersede conflicting old subsystem plans,
milestone targets, and agent examples.

The current pass removes Service/Provider, script async/tasks, caller-borrowed
Rust roots/arguments/returns/views, transparent host paths/leases, dependent
bindings, and JIT-only preparation. It does not implement replacement opaque
objects, new conversion APIs, or a new re-entry model. Missing interop features
are a separate follow-up, not removal prerequisites. Checkpoint A documentation
does not establish completed runtime removal; existing code/tests and historical
acceptance records may still describe the old implementation.

## Start Of Each Turn

1. Read `docs/goal.md`.
2. Read `docs/architecture.md`.
3. Read `docs/runtime-simplification-plan.md` and relevant `docs/progress.md`.
4. Inspect the current git diff before editing; preserve parent/user changes.
5. Run or inspect the most relevant failing test, or existing retained-baseline
   test when no failure is known.
6. Choose the smallest verifiable task advancing the authorized checkpoint;
   respect the current request's file ownership and commit limits.

## End Of Each Turn

1. Run the relevant tests for the changed area.
2. Run formatting checks when practical.
3. Update `docs/progress.md` if the milestone status changed.
4. Update `docs/decisions.md` when a new design decision is made.
5. Update `docs/blocked.md` when progress is blocked by an external decision.
6. Commit small, coherent verified checkpoints using Conventional Commits only
   when authorized by the current request. Do not commit a no-commit handoff.

Status/design/blocked updates above apply only within assigned ownership; report
needed updates to the owner instead of editing excluded files.

## Hard Constraints

- Do not introduce script-language generics.
- Do not inject shared or mutable caller-borrowed Rust data into scripts,
  directly or through handles/wrappers. Borrowed arguments, returns, views,
  iterators, retained leases, and script-managed borrow release are removal scope.
- Do not preserve transparent HostRef/HostPath/PathProxy/HostTargetPlan adapter
  access through aliases, reflection, or replacement calls. Remove dependent
  registrations and extern bindings instead of inventing a new binding model.
- Retain existing independent synchronous native calls and value conversion.
  Serialized data is script-owned and has no implicit Rust write-back. Retain an
  existing opaque surface only if it meets the reduced boundary without new
  machinery; keep type, identity, invalidation, lifetime, and permission checks.
  Rust may borrow internally for one native invocation, never as a script value.
- Do not restore Service hotfixing, Provider entrypoints, script async/await,
  detached tasks/continuations, JIT, or JIT-only preparation. Do not add a
  replacement object system, conversion API, or re-entry model in this pass.
- Reflection may inspect script metadata and perform permitted value operations,
  but must not mutate type structure or recover opaque layout, borrowed children,
  or host paths. Do not build a monkey-patching system.
- Preserve ordinary modules/packages/traits, script collections/iterators,
  script reflection, GC/budgets, interpreter optimizations, CLI/browser execution,
  native LSP/editor workers, and planned debugger/DAP support.
- Preserve compatible script state, stable ABI checks, and exact old executable
  generations for active calls and retained closures, including nested dispatch,
  state ownership, caches, and plans. Never rebind old closures by stable name.
- Do not implement moving GC or structural state migration.
- Do not delete tests to make a failure pass. Update removed-feature success
  expectations with their owning checkpoint; retain safety/rejection coverage.
- Reject incompatible executable artifact formats before activation with the
  owning format change. Do not retain a legacy runtime or compatibility wrapper.
- Do not add unbudgeted infinite execution paths.
- Do not place Rust host state under the script GC.

## Engineering Priorities

Correctness comes first, followed by testability, hot reload semantics, host boundary safety, performance, and syntax sugar.

Prefer a runnable retained-language slice over a large incomplete subsystem:

```text
source -> HIR/MIR -> linked bytecode -> synchronous VM -> script-owned result
```

Keep existing independent native/value calls and script hot reload runnable.
Transparent Rust write-through and Rust Service hotfixing are no longer targets.
Review shared crates by responsibility rather than deleting whole host, package,
reflection, or reload crates by name. Keep interpreter-used MIR/root/budget/cache
analyses and physical plans even when JIT-only interfaces are removed.

Keep implementation structure modular. Do not pile large unrelated logic into a
single file such as `lib.rs`; split code by responsibility into focused modules
that match the crate boundary and architecture documents. For example, syntax
work should separate lexer, tokens, parser, AST/CST, diagnostics, and tests when
those pieces become non-trivial. Add a new module when it clarifies ownership,
keeps files reviewable, or prevents unrelated concepts from sharing one file.

## Validation Commands

Use `docs/validation.md` as the validation command source of truth, including:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features --no-fail-fast
```

Use affected CLI/browser, package, portable-artifact, editor/site, benchmark,
and fuzz gates from `docs/validation.md` when relevant. Existing retained
baseline test entrypoints are listed in `docs/architecture.md`. A baseline pass
alone does not prove removed APIs are unavailable; add boundary rejection and
export/dispatch/artifact audits with the owning removal checkpoint.

## Commit Message Convention

Use Conventional Commits for all commits:

```text
<type>(optional-scope): <description>
```

Allowed types:

```text
feat      user-facing feature or milestone capability
fix       bug fix or behavioral correction
docs      documentation-only change
test      tests or test fixtures
refactor  code change without intended behavior change
perf      performance improvement
build     build system, dependency, or workspace change
ci        CI configuration change
chore     maintenance that does not fit another type
```

Rules:

- Keep the description imperative, lowercase unless it names an identifier, and under 72 characters when practical.
- Use a scope when it clarifies ownership, such as `syntax`, `vm`, `host`, `reflect`, `reload`, `gc`, `stdlib`, or `docs`.
- Mark breaking changes with `!` after the type or scope, and explain the impact in the body.
- Include a short body when the reason, migration path, or validation is not obvious from the subject.
- Do not mix unrelated work in one commit.

Examples:

```text
docs: split project goal and architecture docs
fix(engine): preserve synchronous native value calls
fix(vm): preserve old CodeObject for active frames
test(reflect): cover script value permissions
refactor(common): extract stable id newtypes
feat(reload)!: reject incompatible event ABI changes
```

## Task Template

When writing or following a task, keep it concrete:

```text
Task: Implement X.
Context: X belongs to removal checkpoint A-E or a retained track; relevant files are ...
Expected behavior: ...
Tests: ...
Do not change: ...
Validation: cargo test -p ...
```

Example:

```text
Task: Preserve exact old closure generations while removing obsolete metadata.
Context: This belongs to the simplification pass; Engine reload tests own proof.
Expected behavior:
  - Retained closures resolve nested calls against their creation generation.
  - Old closures can read state removed from the active generation.
  - Rejected reloads leave native dispatch and the active owner unchanged.
Tests in crates/vela_engine/tests/hot_reload.rs:
  - retained_closure_pins_old_generation_across_handle_layout_reload
  - retained_old_closure_reads_state_removed_from_active_generation
  - retained_old_closure_keeps_native_dispatch_and_rejected_reload_owner
Do not change:
  - Do not add object storage, conversion, or re-entry APIs.
  - Do not preserve deleted host bindings through wrappers.
Validation:
  cargo test -p vela_engine --test integration hot_reload::retained_
```
