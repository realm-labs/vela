# Agent Instructions

This repository implements a Hot Reload First dynamic scripting language in Rust for game server logic. Agents should treat [docs/goal.md](docs/goal.md) as the product and milestone target, and [docs/architecture.md](docs/architecture.md) as the technical contract.

## Start Of Each Turn

1. Read `docs/goal.md`.
2. Read `docs/architecture.md`.
3. Read `docs/progress.md` if it exists.
4. Inspect the current git diff before editing.
5. Run or inspect the most relevant failing test.
6. Choose the smallest verifiable task that advances the current milestone.

## End Of Each Turn

1. Run the relevant tests for the changed area.
2. Run formatting checks when practical.
3. Update `docs/progress.md` if the milestone status changed.
4. Update `docs/decisions.md` when a new design decision is made.
5. Update `docs/blocked.md` when progress is blocked by an external decision.
6. Commit at appropriate verified checkpoints using Conventional Commits, keeping each commit small and coherent.

## Hard Constraints

- Do not introduce script-language generics.
- Do not expose real Rust `&mut T` references to scripts.
- Host mutation must be represented through `HostRef`, `HostPath`, and `PatchTx`.
- Reflection may query metadata and perform controlled value reads, writes, and calls, but it must not mutate type structure at runtime.
- Do not build a monkey-patching system.
- Do not implement JIT, async/coroutine hot reload, moving GC, or a full LSP in the MVP.
- Do not delete tests to make a failure pass.
- Do not add unbudgeted infinite execution paths.
- Do not place Rust host state under the script GC.

## Engineering Priorities

Correctness comes first, followed by testability, hot reload semantics, host boundary safety, performance, and syntax sugar.

Prefer a runnable vertical slice over a large incomplete subsystem. The most important early loop is:

```text
script source -> bytecode -> VM -> HostRef/HostPath/PatchTx -> host apply
```

Keep implementation structure modular. Do not pile large unrelated logic into a
single file such as `lib.rs`; split code by responsibility into focused modules
that match the crate boundary and architecture documents. For example, syntax
work should separate lexer, tokens, parser, AST/CST, diagnostics, and tests when
those pieces become non-trivial. Add a new module when it clarifies ownership,
keeps files reviewable, or prevents unrelated concepts from sharing one file.

## Validation Commands

Use these as the default full validation target:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Later milestones may also require:

```bash
cargo run -p vela_cli -- examples/game_server_demo/scripts/level_up.lang
cargo bench --workspace
cargo fuzz run parser
```

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
feat(host): add PatchTx overlay reads
fix(vm): preserve old CodeObject for active frames
test(reflect): cover read-only host field errors
refactor(common): extract stable id newtypes
feat(reload)!: reject incompatible event ABI changes
```

## Task Template

When writing or following a task, keep it concrete:

```text
Task: Implement X.
Context: X belongs to milestone M?, and the relevant files are ...
Expected behavior: ...
Tests: ...
Do not change: ...
Validation: cargo test -p ...
```

Example:

```text
Task: Implement the minimal HostPath and PatchTx overlay model.
Context: This belongs to M3. Scripts must not directly mutate Rust host objects.
Expected behavior:
  - write_path(player.level, 10) records a Set patch.
  - read_path(player.level) returns overlay value 10 in the same transaction.
  - player.level += 1 records an Add patch.
Tests:
  - vela_host::tests::write_then_read_overlay
  - vela_host::tests::add_patch_records_rmw
Do not change:
  - Do not change the VM instruction set.
  - Do not expose real &mut Player to the script layer.
Validation:
  cargo test -p vela_host
```
