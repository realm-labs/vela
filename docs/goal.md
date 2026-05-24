# Goal

Build a dynamic scripting language in Rust for game server logic. The language is Hot Reload First, embeds deeply into Rust host state, and lets gameplay scripts mutate host objects through safe patch transactions instead of direct mutable references.

The language is not "dynamic Rust" and is not a Lua rewrite. It is a scripting language designed around Rust game server state models, controlled reflection, host patching, and reliable function-level hot reload.

## Product Goals

The language should provide:

1. Better gameplay expression than Lua: structs, enums, `match`, method calls, rich array/map APIs, and Option/Result-style error handling.
2. Deep Rust host integration: scripts can naturally read and write host state with syntax such as `player.level += 1`.
3. Safe mutable state boundaries: scripts never hold Rust `&mut T`; they produce `HostPath` operations inside `PatchTx`, and the host applies them at safe points.
4. Hot Reload First semantics: hot reload replaces function-level or module-level code objects. Existing call frames continue on old code, and new calls enter new code.
5. Controlled reflection: scripts can inspect types, fields, methods, variants, traits, modules, and functions, and can perform controlled dynamic reads, writes, and calls. Runtime schema mutation is not allowed.
6. Embeddability: Rust hosts can register types, native functions, permissions, execution budgets, state adapters, and hot reload policies.
7. Practical performance: the MVP should focus on a high-quality bytecode VM, stable IDs, field specialization, inline-cache-ready dispatch, native standard library functions, and GC pacing. JIT is not part of the MVP.

## Non-Goals

The first phase does not include:

- Script-language generics.
- A Rust-style borrow checker in the script language.
- Real Rust references exposed to script code.
- Arbitrary monkey patching of types or methods.
- Arbitrary `eval` or runtime execution of generated source strings.
- JIT compilation.
- Script-level threads or shared-memory concurrency.
- Complex async or coroutine hot reload.
- A full IDE or LSP implementation.
- Performance that exceeds LuaJIT at the outset.

## Design Principles

```text
Dynamic typing, but not unconstrained dynamism.
Comfortable syntax, but controlled runtime boundaries.
Strong reflection for queries, weak reflection for mutation.
Host state can be changed, but Rust &mut is never exposed.
Hot reload is a primary design axis, not an afterthought.
Performance comes from architecture, bytecode, caching, and batch boundaries before JIT.
```

Engineering principles:

1. Every feature must have tests: parser, compiler, VM, host bridge, reflection, and hot reload.
2. Every milestone must be runnable.
3. Close the vertical loop before optimizing.
4. Build the interpreter before considering JIT.
5. Every schema item needs a stable ID: fields, methods, variants, traits, and functions.
6. Hot reload compatibility is bounded by ABI checks.
7. Host permissions must be configurable: execution budget, memory budget, reflection permissions, and host write permissions.
8. Implementations must stay modular: split logic by crate and module responsibility instead of piling unrelated code into one large file.

## Long-Term Codex Goal

The following goal can be used as a persistent implementation target:

```text
/goal Continue implementing a Hot Reload First dynamic scripting language prototype in the current Rust repository until the runnable loop from M0 through M6 is complete: script source can be parsed, compiled into register bytecode, executed in the VM, read and write Rust host state through HostRef/HostPath/PatchTx, use TypeRegistry and reflect APIs for controlled reflection, and replace function-level CodeObject values through hot reload. Completion must be validated by cargo fmt --all -- --check, cargo clippy --workspace --all-targets -- -D warnings, cargo test --workspace, and at least one examples/game_server_demo script run. Maintain these constraints throughout implementation: the script language has no generics; scripts never hold real Rust &mut references; host mutation must enter PatchTx; reflection can only query and perform controlled reads/writes/calls and cannot monkey patch type structure; the first phase does not implement JIT, async, moving GC, or a complex LSP; implementation must be split into focused modules that match crate responsibilities instead of accumulating large unrelated code in one file. Add tests and update docs/progress.md after every verifiable step. Commit at appropriate verified checkpoints using Conventional Commit messages, keeping commits small and coherent so the working tree does not accumulate large unrelated changes. At the start of each iteration, read docs/goal.md, docs/architecture.md, docs/progress.md, and the current failing tests, then pick the smallest verifiable task. If tests fail, prioritize the failures. If the design is unclear, record the assumption and rationale in docs/decisions.md and continue. If progress is blocked under the current constraints, stop and document the blocker, evidence, attempted approaches, and required user decision in docs/blocked.md.
```

## Milestones

### M0: Workspace And Infrastructure

Goal: the repository builds, tests, and formats.

Deliverables:

```text
Cargo workspace
crate skeleton
common ID types
Symbol interner
Span and SourceId
basic Diagnostic structure
CI command documentation
```

Acceptance:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

### M1: Syntax Frontend

Goal: parse the minimal language.

Scope:

```text
let
fn
pub fn
if/else
for-in
return
struct
enum
trait declaration placeholder
field access
method call
array/map literals
lambda
attribute
```

Acceptance:

```text
parser snapshot tests cover core syntax
error recovery reports source spans
```

### M2: Minimal Bytecode VM Loop

Goal: script functions can execute basic logic.

Scope:

```text
Value
CallFrame
register bytecode
arithmetic
comparison
branching
function call
array/map
native function call
```

Acceptance:

```text
basic script calculation tests pass
function call tests pass
mock print/log standard functions can be called
```

### M3: HostRef And PatchTx

Goal: scripts can read and write host state without holding `&mut`.

Scope:

```text
HostRef
HostPath
PathSegment
PatchOp
PatchTx overlay
mock ScriptStateAdapter
GET_HOST_FIELD
SET_HOST_FIELD
ADD_HOST_FIELD / read-modify-write
CALL_HOST_METHOD
```

Acceptance:

```text
player.level = 10 creates a Set patch
player.level += 1 creates an Add patch
reads after writes see overlay values
host generation mismatch reports an error
```

### M4: Reflection System

Goal: TypeRegistry and reflect APIs are usable.

Scope:

```text
TypeKey
TypeDesc
FieldDesc
MethodDesc
TraitDesc
VariantDesc
AttrMap
reflect.type_of
reflect.fields
reflect.get
reflect.set
reflect.call
reflect.implements
```

Acceptance:

```text
reflect.set(host_ref, "level", 10) creates a Patch
reflect.get(record, "field") can read record fields
read-only fields report FieldNotWritable
unknown fields include candidate hints
```

### M5: Struct, Enum, And Match

Goal: script records and enums are first-class values.

Scope:

```text
ObjRecord shape
ObjEnum tag
record constructor
enum constructor
match tag
field destructuring
```

Acceptance:

```text
QuestProgress.Active match tests pass
field slot access tests pass
schema hash generation is stable
```

### M6: Hot Reload First

Goal: function-level code object replacement works.

Scope:

```text
ProgramVersion
FunctionSymbolId
CodeObject indirection
compile_update
apply_hot_update
ABI diff
old version lifetime
safe point
```

Acceptance:

```text
old call frames continue running old code
new calls enter new code
deleted function parameters are rejected
new private helper functions are accepted
```

### M7: GC

Goal: script heap objects are reclaimed automatically.

Scope:

```text
mark-sweep collector
root stack
call frame roots
closure roots
array/map/record/enum tracing
step_gc budget
full collection
```

Acceptance:

```text
live objects survive GC
cyclic script objects can be reclaimed
host refs are not treated as owned Rust objects
```

### M8: Standard Library And Business Example

Goal: common game logic is comfortable to write.

Scope:

```text
array.map/filter/find/any/all/count/sum
map.keys/values/entries/get_or/filter
Option/Result
? operator
example game server demo
```

Acceptance:

```text
monster kill reward example passes
quest progress example passes
level up example passes
```

### M9: Performance Foundation

Goal: establish benchmarks and first-round optimizations.

Scope:

```text
criterion benchmarks
field access benchmark
host patch benchmark
array map/filter benchmark
inline cache prototype
peephole optimization
```

Acceptance:

```text
benchmarks run reliably
optimization reports compare before and after
no correctness regressions
```

## Initial Task List

1. Create the Cargo workspace and crate skeleton.
2. Implement `vela_common`: `Symbol`, `Span`, `SourceId`, ID newtypes, and `Diagnostic`.
3. Implement the `vela_syntax` lexer.
4. Implement the minimal parser: literals, `let`, `fn`, calls, and field access.
5. Add AST snapshot tests.
6. Implement the `vela_bytecode` instruction enum and `CodeObject`.
7. Implement `vela_vm` `Value`, `CallFrame`, and basic arithmetic.
8. Implement native function calls.
9. Implement `vela_host` `HostRef`, `HostPath`, and `PatchTx`.
10. Implement a mock `ScriptStateAdapter`.
11. Connect VM host field reads and writes to `PatchTx`.
12. Implement `vela_reflect` `TypeRegistry`, `TypeDesc`, and `FieldDesc`.
13. Implement `reflect.get` and `reflect.set`.
14. Implement `ProgramVersion` and `FunctionSymbolId`.
15. Implement the function-level hot reload demo.

## Suggested Supporting Files

`docs/progress.md`:

````md
# Progress

## Current Milestone

M0 - Workspace and infrastructure

## Completed

- [ ] Cargo workspace
- [ ] common IDs
- [ ] Symbol interner
- [ ] Span / SourceId
- [ ] Diagnostic skeleton

## Next

- [ ] Implement vela_common crate
- [ ] Add unit tests

## Validation

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
````

`docs/decisions.md`:

```md
# Architecture Decisions

## ADR-0001: Host mutation uses PatchTx, not Rust &mut

Status: Accepted

Context:
Scripts need to mutate Rust host state freely, but Rust borrowing rules cannot be exposed directly to a dynamic VM.

Decision:
Scripts receive HostRef and PathProxy. Mutations produce PatchTx entries. The host applies patches at safe points.

Consequences:
- Multiple script aliases are allowed.
- Runtime can batch, validate, rollback, and log changes.
- Direct Rust mutation is delayed until apply.
```

`docs/blocked.md`:

```md
# Blocked Items

No blockers currently.
```

## Key Risks

### Language Scope Creep

Risk: the language drifts into a mixture of Rust, Python, Lua, and JavaScript.

Control:

```text
The MVP excludes generics, JIT, async, and macros.
Every syntax feature must serve game server logic or the host patch model.
```

### Unclear Host Patch Semantics

Risk: scripts and host state diverge in surprising ways.

Control:

```text
Transaction overlay semantics must be explicit.
Reads after writes must observe transaction values.
Patch apply must be validatable, roll-backable, and loggable.
```

### Premature Hot Reload State Migration

Risk: early full schema migration makes the implementation too complex.

Control:

```text
The first version only supports function-level hot reload.
Long-lived state should primarily live in the Rust host.
Script heap state is not initially guaranteed to migrate across versions.
```

### Uncontrolled Reflection

Risk: reflection becomes monkey patching and breaks hot reload and optimization.

Control:

```text
TypeRegistry is read-only at runtime.
Schema changes happen only through compile/hot reload.
reflect.set writes values only; it never changes schema.
```

### Premature Performance Work

Risk: early NaN boxing, JIT, or moving GC makes the system hard to maintain.

Control:

```text
Close the interpreter loop first.
Optimize only after benchmarks exist.
Prioritize FieldId, shapes, inline caches, and native standard library functions.
```

## Final Acceptance Demo

Script:

```rust
#[event("monster.kill")]
pub fn on_kill(ctx, player, monster) {
    player.exp += monster.exp

    if player.exp >= ctx.config.exp_to_next_level(player.level) {
        player.level += 1
        player.exp = 0
    }

    for reward in ctx.config.kill_rewards.filter(|r| r.monster_id == monster.id) {
        player.inventory.add(reward.item_id, reward.count)
    }
}
```

Rust host test:

```rust
#[test]
fn monster_kill_updates_player_through_patch_tx() {
    let mut world = TestWorld::new();
    let player = world.spawn_player(Player { level: 1, exp: 90, ..Default::default() });
    let monster = world.spawn_monster(Monster { exp: 20, ..Default::default() });

    let mut runtime = compile_demo_runtime();
    let mut tx = PatchTx::new();

    runtime.call(
        "combat.on_kill",
        args![host(player), host(monster)],
        CallOptions::gameplay(),
        &mut world,
        &mut tx,
    ).unwrap();

    world.apply(tx).unwrap();

    assert_eq!(world.player(player).level, 2);
    assert_eq!(world.player(player).exp, 0);
}
```

Hot reload demo:

1. Old function grants 20 exp for a kill.
2. Hot updated function grants 30 exp for a kill.
3. Old call frames still grant 20 exp.
4. New calls grant 30 exp.
5. Module top-level side effects are not re-executed.
