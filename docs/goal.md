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
7. Practical performance: the MVP should keep the bytecode VM, stable IDs,
   field slots, native standard library functions, and GC boundaries ready for
   optimization. After the MVP, the non-JIT interpreter should target
   Lua-comparable performance on representative gameplay workloads before
   post-MVP Cranelift JIT work.

## Non-Goals

The first phase does not include:

- Script-language generics.
- Function overloading by arity, type hint, or native signature.
- A Rust-style borrow checker in the script language.
- Real Rust references exposed to script code.
- Arbitrary monkey patching of types or methods.
- Arbitrary `eval` or runtime execution of generated source strings.
- JIT compilation.
- Script-level threads or shared-memory concurrency.
- Complex async or coroutine hot reload.
- A full IDE or LSP implementation.
- Performance that exceeds LuaJIT at the outset.

These are first-phase non-goals. The post-MVP roadmap includes a debugger
runtime/DAP milestone and a Cranelift JIT milestone after the interpreter,
baseline, and inline-cache work are stable.

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
9. The pre-release implementation should not carry backward-compatibility
   shims for old internal APIs, transitional script behavior, or temporary
   artifacts. Replace obsolete paths and update callers/tests instead. This
   does not weaken the product requirement for hot reload ABI and schema
   compatibility checks.
10. Architecture quality is part of the feature. If adding a feature requires
    awkward call chains, repeated conditional patches, oversized functions,
    or growing parameter lists, adjust the module boundary, dispatch model,
    or data structure first.
11. Keep code reviewable: split large files by responsibility, extract
    cohesive parameter structs or option objects, and replace accumulating
    `if` chains with `match`, enum-driven dispatch, tables, or focused helper
    types when that better expresses the design.
12. Keep roadmap and status docs concise. `docs/goal.md`, `docs/progress.md`,
    and `docs/performance.md` are decision documents, not changelogs. Routine
    implementation notes, long benchmark logs, rejected micro-candidates, and
    per-commit before/after tables belong in commit messages, PR notes, or
    `docs/archive/` when they need to be preserved.

## Long-Term Codex Goal

The following goal can be used as a persistent implementation target:

```text
/goal Treat docs/goal.md as the stable product roadmap, docs/architecture.md as
the technical contract, and docs/progress.md as the rolling source of current
status and remaining milestone gaps. Continue implementing Vela into a complete
Hot Reload First dynamic scripting language for game server logic, always
starting from the active checkpoint in docs/progress.md. Preserve the standing
constraints in this roadmap: no script-language generics, no Rust &mut exposed
to scripts, all host mutation through HostRef, HostPath, PathProxy, and PatchTx,
reflection without runtime type-structure mutation or monkey patching, and no
MVP JIT, script async/coroutines, moving GC, or full LSP. For each turn, choose
the smallest verifiable task that advances the current milestone, validate it
with the relevant subset of docs/validation.md, update docs/progress.md only
when current focus, milestone status, or current gaps change, and commit
appropriate verified checkpoints with Conventional Commit messages.
Keep durable docs compact: update `docs/progress.md` only for focus, status,
validation, or remaining-gap changes; update `docs/performance.md` only for
baseline checkpoints, target thresholds, benchmark harness changes, milestone
exit conclusions, or durable measurement rules. Archive long historical detail
instead of appending it to current docs.
```

Post-MVP performance work is a first-class roadmap track. The initial release
should not depend on JIT, but the optimized interpreter should eventually be
measured against Lua 5.x on equivalent gameplay-style workloads. LuaJIT and
Node.js remain useful upper-reference points for hot scalar loops and future
JIT decisions, not the baseline required for the MVP. Cranelift JIT is a
post-MVP backend milestone after the optimized interpreter and inline-cache
work, and debugger support is planned as runtime debug hooks plus Debug Adapter
Protocol integration rather than script-language syntax.

## Milestones

These milestones start after the completed M0-M6 prototype. Current
implementation status lives in [progress.md](progress.md), and detailed
historical progress is archived under [archive](archive/). The plan below
tracks the first complete non-JIT, non-async interpreter plus post-MVP
debugger, JIT, and release-hardening work.

### Milestone Checkpoint Rules

Each milestone has a checkpoint that defines when work may move forward.
Acceptance lists the behavioral contract; the checkpoint names the proof that
must exist in tests, examples, docs, or benchmarks. If a milestone is marked
`Complete enough` in [progress.md](progress.md), future work should not return
to it unless the current checkpoint or a regression test exposes a concrete
gap.

### Documentation Checkpoint Rules

Milestone docs should record current truth, not implementation chronology.

```text
docs/progress.md: active focus, milestone status, current gaps, validation
docs/performance.md: current rules, current baseline, target thresholds, exit summaries
docs/archive/: long benchmark histories or durable historical context
commit/PR notes: routine before/after measurements and rejected candidates
```

Do not append raw benchmark output, repeated candidate logs, or per-commit
implementation summaries to current docs. A milestone checkpoint may cite the
commands and the final numbers that changed direction; detailed tables should
be archived only when they are needed for later audit.

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

Checkpoint:

```text
cargo test covers VM budget traps, PatchTx budget traps, managed heap roots,
cycle collection, and host-ref exclusion from GC tracing
docs/progress.md marks M7 complete or names the specific failing safety case
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

Checkpoint:

```text
cargo test covers multi-file imports, duplicate/unresolved declarations,
top-level effect rejection, and bytecode equivalence through HIR
docs/progress.md marks M8 complete enough or names the unresolved HIR gap
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

Checkpoint:

```text
grammar conformance fixtures cover every supported construct from
docs/grammar.ebnf, and unsupported constructs have explicit diagnostics
docs/progress.md names any grammar feature still deferred to later milestones
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

Checkpoint:

```text
cargo test covers script struct and enum registry lowering, slot access,
trait defaults, host/script impl checks, and stable schema hashes
docs/progress.md marks M10 complete enough or names the missing metadata path
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

Checkpoint:

```text
cargo test covers nested HostPath reads/writes, overlay read-after-write,
read-only and permission failures, rollback-safe apply, and copied host returns
docs/progress.md marks M11 complete enough or names the missing host boundary
```

### M12: Complete Reflection And Permissions

Goal: reflection is useful for admin/debug tooling while remaining bounded,
permissioned, and schema-safe.

Scope:

```text
TypeRegistry modules, functions, fields, methods, variants, traits, attrs
TypeHint, TypeKind, FieldAccess, MethodAccess, EffectSet, DeclOrigin, DocString
reflect::name, kind, field, fields, has_field
reflect::get and reflect::set for host refs and script records
reflect::methods, has_method, call
reflect::variant and variant_is
reflect::traits and implements
reflect::module and exports
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

Checkpoint:

```text
cargo test covers reflection metadata for every TypeRegistry item category,
permissioned get/set/call, lookup budgets, candidate spans, and schema-safe
mutation denial
docs/progress.md lists only concrete remaining M12 edge cases, or marks M12
complete enough and moves broad diagnostics polish to M16
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
math::max/min/clamp/floor/ceil/abs
controlled random through permissions or context
ctx::now, ctx::tick, logging, event emit helpers
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

Checkpoint:

```text
cargo test covers array, map, set, string, Option, Result, math, context,
random/time permission, and lambda callback behavior
game_server_demo uses stdlib helpers without custom glue for core rewards
docs/progress.md names the next missing stdlib family or marks M13 complete enough
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

Checkpoint:

```text
cargo test covers EngineBuilder registration, compile_file/compile_dir,
Runtime::call, native descriptors, stable ID rejection, permissioned native
calls, signature conversion, and derive macro schema parity
docs/progress.md names the next missing embedding surface or marks M14 complete
enough
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

Checkpoint:

```text
cargo test covers safe-point staging, old-frame lifetime, new-call version
entry, source-file update workflows, function/effect/schema ABI rejection,
compatible additions, and reload reports with repair hints
docs/progress.md names the next missing reload workflow or marks M15 complete
enough
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
frame metadata for parameters, locals, captures, bytecode offsets, and roots
TypeFact inference for locals, host refs, arrays, maps, enums, and null checks
diagnostic/debug shared data for future debugger and DAP integration
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

Checkpoint:

```text
cargo test snapshot fixtures cover parser, semantic, runtime, host, reflection,
hot reload, call-stack, TypeFact, flow-narrowing, and completion diagnostics
docs/progress.md names the next missing diagnostic family or marks M16 complete
enough
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

Checkpoint:

```text
cargo test and demo CLI runs cover level_up, monster_kill_reward,
quest_progress, reflect_debug, hot_reload_function_swap, negative host/reload
cases, and reusable parser/compiler/VM/host/reflect fixtures
docs/progress.md names the next missing demo workflow or marks M17 complete
enough
```

### M18: Performance Measurement And Baselines

Goal: make script performance measurable, reproducible, and comparable before
large optimization work begins.

Scope:

```text
criterion benchmark suite
official microbench and gameplay-style benchmark cases
external comparison harness for Lua 5.x, LuaJIT, Rhai, and JavaScript when available
VM scalar dispatch benchmark
managed heap allocation and materialization benchmark
array/map/set/string stdlib benchmarks
record, enum, Option, and Result benchmarks
HostRef/HostPath/PatchTx benchmark
hot reload safe-point and ABI benchmark
GC pacing benchmark
concise baseline and measurement rules in docs/performance.md
```

Acceptance:

```text
benchmarks run in release mode with stable parameters
benchmark output records environment, profile, runtime options, and checksums
Vela internal baselines separate compile/load time from repeated function calls
external comparisons record runtime versions and environment details
performance docs identify the top interpreter bottlenecks before optimization
```

Checkpoint:

```text
cargo bench records reproducible internal baselines with checksums and
environment notes; docs/performance.md summarizes the current baseline and
external runtime versions when available
docs/progress.md marks optimization work blocked until benchmark gaps are named
or M18 is complete enough
```

### M19: Non-JIT Interpreter And Heap Optimization

Goal: improve the bytecode interpreter enough that non-JIT Vela can target
Lua-comparable performance on representative gameplay workloads.

Scope:

```text
VM dispatch tightening and operand decode cleanup
fast paths for int, float, bool, and string operations
Value layout profiling before low-level representation changes
shape + slot record and enum access
heap allocation reduction for temporary arrays, records, strings, and callbacks
managed heap materialization reduction at native and stdlib boundaries
native stdlib fast paths for array/map/set/string/Option/Result methods
optimized for-in loops and iterator state
closure allocation caching where semantics allow it
GC threshold and safe-point pacing improvements
peephole optimization
precompiled bytecode artifacts and bytecode cache
```

Acceptance:

```text
optimized interpreter preserves all conformance and host-boundary behavior
benchmarks show before/after changes for each accepted optimization
non-JIT gameplay-style benchmark group is within the documented Lua 5.x target band
slow-path diagnostics remain source-spanned and debuggable
no optimization bypasses ExecutionBudget, PatchTx, reflection policy, or GC roots
```

Checkpoint:

```text
cargo test and cargo bench show before/after results for accepted interpreter
or heap optimizations; docs/performance.md summarizes only the current
baseline, target status, and milestone exit conclusion
docs/progress.md names remaining measured bottlenecks or marks M19 complete
enough for inline caches
```

### M20: Inline Cache And Specialization

Goal: specialize common dynamic operations while preserving VM semantics and
safe fallback paths.

Scope:

```text
inline cache for script record fields
inline cache for host field reads and writes
inline cache for method dispatch and stdlib value methods
shape, schema, MethodId, FieldId, and ProgramVersion guards
monomorphic and small polymorphic cache states
profile counters for hot functions and hot bytecode offsets
cache invalidation on hot reload and schema ABI changes
specialized bytecode or side-table fast paths for stable hot operations
```

Acceptance:

```text
cache misses and guard failures fall back to the generic VM path
cache state is owned by ProgramVersion or another versioned runtime artifact
hot reload cannot expose stale FieldId, MethodId, shape, or function targets
benchmark reports separate interpreter-only and cache-enabled results
```

Checkpoint:

```text
cargo test covers cache hits, misses, guard failures, fallback behavior, hot
reload invalidation, and schema invalidation
cargo bench reports interpreter-only versus cache-enabled benchmark groups
docs/performance.md records only durable cache-enabled baseline summaries and
target status; docs/progress.md names remaining cache families or marks M20
complete enough
```

### M21: Debugger Runtime And DAP Integration

Goal: provide a comfortable IDE-style debugging experience through runtime
debug hooks and Debug Adapter Protocol integration without making debugging a
script-language feature.

Scope:

```text
source breakpoints and conditional breakpoints
step into, step over, step out, pause, and continue
call stack and frame inspection with source spans and bytecode offsets
locals, parameters, captured values, and watch/evaluate expressions
safe HostRef display through reflection and host access policy
PatchTx preview without applying host mutations
runtime exception and host error breakpoints
hot reload breakpoint rebinding across ProgramVersion changes
Debug Adapter Protocol server or adapter boundary for IDE integration
```

Acceptance:

```text
debugger can pause at source breakpoints and resume deterministically
single-step behavior matches VM instruction/source-span mapping
watch/evaluate respects reflection permissions and cannot expose Rust references
PatchTx preview never applies mutations by itself
hot reload preserves or reports breakpoint rebinding across compatible updates
debug hooks can be disabled for normal gameplay execution
```

Checkpoint:

```text
cargo test or adapter fixtures cover breakpoints, stepping, frame inspection,
watch/evaluate permissions, PatchTx preview, exception breaks, hot-reload
rebinding, and disabled-debug execution
docs/progress.md names remaining debugger workflows or marks M21 complete enough
```

### M22: Cranelift JIT

Goal: add Cranelift native code generation after the optimized interpreter,
inline caches, debugger contracts, and performance baselines are stable.

Scope:

```text
Cranelift baseline JIT for a restricted hot-function subset
guards for dynamic value tags, shapes, schemas, and method targets
deoptimization or side exits back to the bytecode VM
compiled frame root reporting for GC, debugging, and deoptimization
ExecutionBudget checks in compiled code or mandatory side exits
PatchTx, permissions, reflection, and host calls routed through existing helpers
ProgramVersion ownership of compiled code and invalidation metadata
JIT enable/disable runtime option
```

Acceptance:

```text
JIT is not required for correctness and can be disabled
unsupported functions continue through the bytecode VM
compiled code and VM code produce identical results on conformance fixtures
hot reload drops or invalidates compiled artifacts at safe points
budget, GC, debugger, and PatchTx invariants hold under compiled execution
```

Checkpoint:

```text
cargo test runs VM-versus-JIT equivalence fixtures and invariant checks for
budgeting, GC roots, host calls, PatchTx, permissions, reflection, and reload
invalidation
docs/progress.md names unsupported JIT subsets or marks M22 complete enough
```

### M23: Performance Hardening And Release Targets

Goal: turn the measured and optimized runtime into a release-quality scripting
engine with documented performance expectations.

Scope:

```text
performance regression thresholds for key benchmarks
runtime configuration docs for budgets, GC, heap mode, and caches
public API docs and examples
release validation command set
release-level benchmark archive and trend summary
clear guidance for Lua-comparable, LuaJIT-comparable, and host-heavy workloads
```

Acceptance:

```text
final validation passes fmt, clippy, tests, demos, and benchmarks
public API docs compile
performance docs state achieved target bands and known gaps
hosts can choose deterministic interpreter-only execution without enabling JIT
```

Checkpoint:

```text
final validation passes fmt, clippy, tests, demos, benchmarks, public API docs,
performance thresholds, and release documentation
docs/progress.md and docs/performance.md state achieved targets and known gaps
```

## Current Status Tracking

The current implementation status, active milestone focus, and remaining
current gaps are tracked in [progress.md](progress.md). Current performance
rules, baselines, target bands, and milestone exit summaries are tracked in
[performance.md](performance.md). Keep these files stable as decision
documents; do not use them as changelogs, raw benchmark ledgers, or per-commit
progress logs.

## Key Risks

### Language Scope Creep

Risk: the language drifts into a mixture of Rust, Python, Lua, and JavaScript.

Control:

```text
The first complete interpreter excludes script generics, JIT, script async,
and script macros.
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
reflect::set writes values only; it never changes schema.
```

### Premature Performance Work

Risk: early NaN boxing, JIT, or moving GC makes the system hard to maintain, or
unmeasured micro-optimizations obscure the path to Lua-comparable non-JIT
gameplay performance.

Control:

```text
Close the interpreter loop first.
Optimize only after benchmarks exist.
Prioritize FieldId, shapes, native standard library fast paths, heap reductions,
debugger contracts, and inline caches before implementing Cranelift JIT.
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
