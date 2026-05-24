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
/goal Treat docs/goal.md as the authoritative product roadmap, docs/architecture.md as the technical contract, and docs/progress.md as the current implementation status. Continue implementing Vela from the completed M0-M6 runnable prototype into a complete Hot Reload First dynamic scripting language for game server logic. Complete means the full planned language surface in docs/grammar.ebnf can be resolved, analyzed, compiled, and executed; script heap values are managed by a budgeted non-moving GC; scripts mutate host state only through HostRef, HostPath, PathProxy, and PatchTx; TypeRegistry and reflection cover types, modules, functions, fields, methods, traits, variants, attributes, and permissions; Rust hosts can register schemas and native functions through a stable Engine API and derive macros; hot reload performs function, schema, and effect ABI checks at safe points; the standard library covers collections, Option/Result-style propagation, math, time/context, and gameplay helpers; and examples/game_server_demo proves level-up, monster-kill rewards, quest progress, reflection, and hot reload workflows. Maintain these constraints throughout implementation: the script language has no generics; scripts never hold real Rust &mut references; host mutation must enter PatchTx; reflection can only query and perform controlled reads/writes/calls and cannot monkey patch type structure; the first complete interpreter does not implement JIT, script async/coroutines, moving GC, or a full LSP. Every milestone must be runnable, tested, documented in docs/progress.md, and validated by the relevant subset of cargo fmt --all -- --check, cargo clippy --workspace --all-targets -- -D warnings, cargo test --workspace, demo script runs, and benchmark/fuzz targets once those exist. Commit appropriate verified checkpoints using Conventional Commit messages.
```

## Milestones

These milestones start after the completed M0-M6 prototype. The completed
history remains in [progress.md](progress.md); the plan below only tracks the
remaining work needed for the complete non-JIT, non-async interpreter.

### M7: Runtime Safety, Budgets, And GC

Goal: script execution is bounded, and script heap objects are reclaimed
without moving references or owning host state.

Scope:

```text
ExecutionBudget for instruction count, memory bytes, call depth, patch count
budget charging in VM dispatch, native calls, reflection, and host patching
script heap with stable GcRef handles
non-moving mark-sweep collector
root stack and call frame roots
tracing for string, array, map, set, record, enum, closure, and upvalue objects
step_gc pacing and full collection
host refs treated as external handles, not owned GC objects
```

Acceptance:

```text
recursive scripts stop at max_call_depth
infinite loops stop at instruction budget once loops exist
patch floods stop at max_patches
live script objects survive GC
cyclic script objects are reclaimed
host refs are never traced as Rust-owned objects
```

### M8: Resolver, HIR, And Module Graph

Goal: parsed source lowers into a stable semantic representation shared by the
compiler, diagnostics, hot reload, and future tooling.

Scope:

```text
vela_hir crate
module graph and use/import resolution
declaration index for functions, structs, enums, traits, impls, consts
SymbolTable and BindingMap
stable node IDs and expression IDs
type hints parsed into metadata without script generics
top-level side-effect restrictions
HIR lowering from AST with source spans preserved
bytecode compiler consuming HIR instead of raw syntax AST
```

Acceptance:

```text
imports resolve across multiple files
unresolved names report candidate suggestions
duplicate declarations are diagnosed with both spans
compiler output remains equivalent for existing examples
module top-level host mutation is rejected before bytecode generation
```

### M9: Complete Executable Language Surface

Goal: every non-deferred language construct in the grammar can compile and run
with correct dynamic semantics.

Scope:

```text
unary operators and logical short-circuiting
local assignment and compound assignment
index reads and writes
for-in loops
break and continue
method calls on script values, host paths, and stdlib values
lambda and closure values with captured upvalues
block, if, and match expression values
match guards, literal patterns, binding patterns, tuple variants
default parameter values and named call arguments
return behavior through nested blocks and closures
```

Acceptance:

```text
grammar executable conformance tests pass for all supported constructs
lambda closures retain captured values after outer frames return
for-in loops support arrays, maps, and host-provided iterables
break/continue work through nested control-flow blocks
unsupported grammar remains explicitly diagnosed, not silently miscompiled
```

### M10: Script Types, Shapes, Traits, And Dispatch

Goal: script-defined records, enums, and traits use stable runtime metadata
instead of syntactic heuristics.

Scope:

```text
script struct declarations lower into TypeRegistry entries
script enum declarations lower into TypeRegistry entries
ShapeId and slot-based ObjRecord layout
ObjEnum with stable VariantId and field slots
schema hash generation for script types
trait declarations with default methods
impl blocks for script types and host types
dynamic trait/protocol implements checks
method dispatch through MethodId and fallback dynamic lookup
```

Acceptance:

```text
field slot access replaces named-map record access
schema hashes stay stable across field reordering
trait default method tests pass
host and script types can both satisfy a script trait
enum variant additions are represented with stable VariantId values
```

### M11: Complete Host Bridge And Patch Transactions

Goal: natural script syntax can read, call, and mutate nested host state through
controlled paths and transactions.

Scope:

```text
PathProxy value category
nested HostPath lowering for fields, indexes, keys, and variant fields
GET_HOST_PATH, SET_HOST_PATH, RMW_HOST_PATH, CALL_HOST_METHOD lowering
HostValue conversion for arrays, maps, records, enums, host refs, and nullables
PatchTx overlay for Set, Add, Sub, Remove, Push, and method-call return effects
patch validation, rollback-safe apply, and conflict reporting
host access policies for read/write/call permissions
source-span propagation into patches and host errors
```

Acceptance:

```text
player.inventory.items[item_id].count += 1 records a nested RMW patch
reads after nested writes observe overlay values
read-only and permission-denied host paths fail before apply
failed apply leaves adapter state unchanged
host method calls can return script-visible copied values without exposing &mut
```

### M12: Complete Reflection And Permissions

Goal: reflection is useful for admin/debug tooling while remaining bounded,
permissioned, and schema-safe.

Scope:

```text
TypeRegistry modules, functions, fields, methods, variants, traits, attrs
TypeHint, TypeKind, FieldAccess, MethodAccess, EffectSet, DeclOrigin, DocString
reflect.name, kind, field, fields, has_field
reflect.get and reflect.set for host refs and script records
reflect.methods, has_method, call
reflect.variant and variant_is
reflect.traits and implements
reflect.module and exports
reflection permission checks and lookup budgets
candidate hints for unknown fields, methods, variants, modules, and functions
```

Acceptance:

```text
reflection cannot mutate type structure at runtime
gameplay permissions allow approved field reads and method calls only
GM/admin permissions can inspect configured host paths
unknown-name diagnostics include ranked candidates and related schema spans
reflective calls respect EffectSet and MethodAccess
```

### M13: Standard Library And Language Conveniences

Goal: common game-server logic is compact, readable, deterministic, and
permission-aware.

Scope:

```text
array.len/is_empty/push/pop/map/filter/find/any/all/count/sum/group_by/sort_by
map.len/has/get/get_or/set/remove/keys/values/entries/map_values/filter
set APIs
string APIs needed for gameplay scripts and diagnostics
Option and Result as dynamic enums
? operator lowering for Option/Result propagation
math.max/min/clamp/floor/ceil/abs
controlled random through permissions or context
ctx.now, ctx.tick, logging, event emit helpers
stdlib metadata for TypeFacts without user-visible generics
```

Acceptance:

```text
collection methods work with lambdas and preserve dynamic values
? propagates None and Err through script functions
random and wall-clock APIs require explicit permissions
monster kill reward script is readable without custom native glue
stdlib methods expose analysis facts for lambda parameter hints
```

### M14: Engine, Native Functions, And Rust Host Macros

Goal: Rust applications can embed Vela with stable schemas, explicit effects,
and minimal boilerplate.

Scope:

```text
Engine and EngineBuilder
compile_file and compile_dir
Runtime::call with CallOptions
args!/host! convenience APIs
NativeFunctionDesc and FunctionDesc
NativeCallContext with runtime, state adapter, PatchTx, permissions, budget
native function and native method registration with stable IDs
Rust signature conversion rules
vela_macros crate
#[derive(ScriptHost, ScriptReflect)]
#[script_methods] and #[script_method]
generated schema hashes, field accessors, method dispatch, and docs/origin data
```

Acceptance:

```text
sample Rust host registers Player, Monster, Inventory, and config types
derive macro output matches explicit hand-written TypeRegistry metadata
duplicate stable IDs are rejected at registration or compile time
native calls consume budgets and enforce permissions
scripts never receive real Rust references from native APIs
```

### M15: Production Hot Reload Semantics

Goal: hot reload is safe across function, module, type, reflection, and host
schema boundaries.

Scope:

```text
Runtime current ProgramVersion with registry, modules, functions, and code objects
active version epochs and old-version lifetime tracking
safe points at event end, tick boundary, and before/after patch apply
compile_update for changed files and module dependency invalidation
ABI diff for exported functions, event handlers, native descriptors, effects
schema diff for structs, enums, fields, variants, methods, traits
default value construction for compatible schema additions
top-level side-effect rejection during reload
hot reload reports with accepted/rejected changes and repair hints
```

Acceptance:

```text
old call frames continue on old code without seeing partial updates
new calls enter updated code after a safe point
event ABI parameter removals, reordering, and effect expansion are rejected
new private helpers and compatible schema additions are accepted
module top-level side effects are not re-executed during reload
```

### M16: Diagnostics, Error Reporting, And Tooling Foundation

Goal: errors are actionable for script authors, and the core data structures are
ready for editor tooling without requiring a full LSP.

Scope:

```text
lossless CST or equivalent token tree with comments, newlines, and spans
diagnostics with primary span, related labels, call stack, candidates, hints
semantic diagnostics for unresolved names, fields, methods, variants, effects
runtime diagnostics mapped back to source spans and function stack frames
TypeFact inference for locals, host refs, arrays, maps, enums, and null checks
flow narrowing for if, match, and Option/Result-style checks
completion data for bindings, modules, fields, methods, variants, stdlib APIs
snapshot tests for diagnostic rendering
```

Acceptance:

```text
misspelled host fields report candidates and read/write access hints
runtime host errors include script call stack and source span
match exhaustiveness hints are available when enum facts are known
completion fixtures can suggest fields and methods from TypeRegistry
diagnostics degrade cleanly to Any at dynamic boundaries
```

### M17: Game Server Demo And Conformance Suite

Goal: the language is proven by realistic gameplay workflows and reusable
conformance fixtures.

Scope:

```text
examples/game_server_demo host world
level_up script
monster_kill_reward script
quest_progress script
reflect_debug script
hot_reload_function_swap script
tests/fixtures source programs
parser, compiler, VM, host, reflect, hot reload integration tests
negative tests for permissions, ABI mismatch, stale host refs, bad schemas
parser fuzz target once grammar stabilizes
demo CLI commands documented in docs/validation.md
```

Acceptance:

```text
all game_server_demo scripts run through Engine and Runtime APIs
monster kill updates player exp, level, inventory, and quest progress via PatchTx
reflect debug script can inspect allowed fields but cannot mutate schema
hot reload demo proves old frames and new calls observe correct code versions
conformance suite guards every supported grammar feature
```

### M18: Performance Foundation And Release Hardening

Goal: the interpreter has measured performance, first-round optimizations, and
clear release-quality behavior.

Scope:

```text
criterion benchmark suite
field access benchmark
host patch benchmark
array map/filter benchmark
hot reload benchmark
GC pacing benchmark
shape and field slot optimization
inline cache prototype for fields and method dispatch
specialized host field read/write fast paths
peephole optimization
bytecode cache
runtime configuration docs
public API docs and examples
```

Acceptance:

```text
benchmarks run reliably and compare before/after optimization reports
optimized paths preserve all conformance behavior
GC pacing keeps configured pause budgets in benchmark scenarios
public API docs compile
final validation passes fmt, clippy, tests, demos, and benchmarks
```

## Remaining Task List

1. Implement `ExecutionBudget` and charge the current VM dispatch loop.
2. Move script-owned compound values behind a non-moving heap and `GcRef`.
3. Add mark-sweep GC roots for VM frames, native calls, and temporary values.
4. Create `vela_hir` for module resolution, bindings, HIR, and stable node IDs.
5. Move bytecode lowering from syntax AST to HIR.
6. Complete bytecode and VM support for loops, indexes, lambdas, closures, and
   full match patterns.
7. Replace named-map records/enums with shape, slot, and stable schema metadata.
8. Implement nested HostPath/PathProxy operations and rollback-safe patch apply.
9. Expand TypeRegistry and reflection permissions to the full metadata surface.
10. Build `vela_std` with collections, Option/Result, `?`, math, and context APIs.
11. Add `Engine`, `Runtime`, `CallOptions`, native descriptors, and permissioned
    native dispatch.
12. Add `vela_macros` for host schema and method registration.
13. Strengthen hot reload ABI/schema/effect checks and safe-point integration.
14. Add diagnostic rendering, call stacks, TypeFacts, and tooling fixtures.
15. Expand `examples/game_server_demo` into the final acceptance demo suite.
16. Add benchmarks, inline caches, peephole optimization, and bytecode caching.

## Roadmap Maintenance Files

`docs/progress.md`:

````md
# Progress

## Current Milestone

M7 - Runtime safety, budgets, and GC

## Completed

- M0-M6 runnable prototype loop complete.

## Next

- [ ] Implement `ExecutionBudget`
- [ ] Add GC heap and roots
- [ ] Validate with focused runtime tests

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
The complete interpreter excludes script generics, JIT, script async, and
script macros.
Rust host derive macros are allowed only to reduce embedding boilerplate.
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
