## Diagnostics

Errors must include:

```text
error kind
source span
call stack
related type/field/method information
candidates
repair hints
```

Examples:

```text
FieldNotFound:
  type: billing::account::Account
  field: balnace
  candidates: ["balance"]
```

Copied reflection records for script-defined modules, functions, types, traits,
fields, methods, trait methods, and variants include `source_span: { source,
start, end }` when the registry knows the declaration location. Host-provided
descriptors may leave this field as `null`. Unknown reflection lookups carry
ranked related candidates with the same optional source spans where descriptors
have source locations, so admin/debug tooling can jump from a misspelled lookup
to nearby schema declarations without parsing human-readable messages.
Dynamic `reflect::get` and `reflect::set` calls on script record or enum values
preserve the script type name at the reflection boundary. If that type or
variant exists in the registry, unknown-field diagnostics use the registered
field metadata and related source spans rather than treating the value as an
anonymous record.
Field reflection records also expose the declared `type` hint when one is
known, or `null` for unhinted/dynamic fields. These are copied documentation and
tooling hints, not generic script types or static enforcement.
Field access records expose copied `required_permissions` so admin/debug tools
can explain why a field is hidden or denied under the active reflection policy.
Method and trait-method reflection records expose copied `params`, `return`,
and `returns` metadata. `return` matches function reflection naming, while
`returns` is a script-accessible alias because `return` is a keyword.

```text
FieldNotWritable:
  type: billing::account::Account
  field: ledger
  reason: field is read-only
  hint: account.ledger.add(...) instead
```

```text
HotReloadAbiMismatch:
  Function: billing.on_invoice_paid
  old_params: [ctx, account, invoice]
  new_params: [ctx, account]
  reason: exported event function cannot remove parameters
```

```text
StaleHostRef:
  type: billing::account::Account
  object_id: 1024
  reason: generation mismatch
```

## IDE And LSP Readiness

A full native LSP capability track is allowed before the MVP and may progress
in parallel with M19/M20 optimization when it stays analysis-only. The track
should expose diagnostics, completion, signature help, hover, go to definition,
symbols, semantic tokens, references, rename, code actions, formatting, inlay
hints, source overlays, static host schema facts, and incremental invalidation
without executing scripts or running host applications. A custom full IDE
product remains outside the MVP. The required foundation is:

```text
lossless CST or equivalent token tree with comments, newlines, and spans
stable AST node IDs and expression IDs after lowering
incremental-friendly parser with error recovery
module graph and import resolver
SymbolTable and BindingMap shared by compiler and tools
TypeRegistry available as host/schema input
TypeFact inference for editor hints
diagnostics that carry spans, related locations, candidates, and fix hints
```

Strong hints should be gradual, not mandatory static typing:

```text
known schema or type hint -> precise completion and diagnostics
known host ref -> precise fields and methods from TypeRegistry
known array/map element facts -> lambda parameter hints
known enum -> variant completion and match pattern hints
unknown dynamic value -> degrade to Any
reflect with non-constant field name -> degrade to Any
reflect with constant field name and known schema -> resolve normally
io/fs stdlib effects -> require explicit io_read/io_write capabilities
fs stdlib paths -> stay relative to the configured sandbox root
```

LSP feature mapping:

```text
completion          SymbolTable + TypeFact + TypeRegistry
hover               TypeFact + docs + EffectSet + DeclOrigin
go to definition    BindingMap + DeclOrigin
find references     BindingMap reference index
rename              SymbolTable ownership and module visibility
diagnostics         parser recovery + semantic model + TypeRegistry
semantic tokens     CST token kinds + resolved symbols
code actions        diagnostics with structured fix hints
```

Design constraints for future tooling:

```text
do not make runtime reflection mutate TypeRegistry in place
do not allow monkey patching to add fields or methods at runtime
do not erase source spans during lowering
do not make host schemas string-only; keep stable IDs and docs/origin metadata
do not require full static type success before bytecode generation
```

Record literals and map literals intentionally stay distinct:

```text
Account { balance: 1 }    typed record or host-like constructor
{ "balance": 1 }          map literal
{ balance: 1 }            map literal with identifier key
```

The parser may use context to disambiguate blocks from map literals, but LSP
completion should prefer record fields only after a known type path followed by
`{` or when expected type information exists.

## Debugger Architecture

Debugger support is a post-MVP runtime and adapter capability, not a
script-language feature. The first target is an IDEA/Kotlin/Java-like
experience through runtime debug hooks plus a Debug Adapter Protocol boundary;
a dedicated JetBrains plugin can build on that boundary later.

Debugger-visible behavior should include:

```text
source breakpoints and conditional breakpoints
step into, step over, step out, pause, and continue
call stack with source spans, function names, and ProgramVersion identity
parameters, locals, captures, and watch/evaluate expressions
safe HostRef display through reflection and host access policy
read-only host inspection through reflection and host access policy
runtime exception and host error breakpoints
hot reload breakpoint rebinding across ProgramVersion changes
```

Debug operations must use the same safety boundaries as scripts:

```text
do not expose real Rust references
do not bypass HostAccess or ScriptStateAdapter for host mutation
do not mutate TypeRegistry or runtime type structure
respect reflection permissions and host read/write/call policies
charge or suspend execution budgets through explicit debugger policy
resume only at VM safe points or well-defined debug suspension points
```

The VM, bytecode compiler, and future optimized backends must preserve enough
debug metadata to reconstruct source locations, frame values, GC roots,
captured variables, and side-exit state. JIT and inline-cache fast paths must
either support debugger suspension directly or side-exit to an equivalent
bytecode VM frame before exposing state.

## Performance Architecture Contract

Performance work must preserve the language and embedding contracts. The
optimized interpreter, inline caches, specialization, and Cranelift JIT are
implementation choices behind the same VM semantics.

Stable runtime facts:

```text
FieldId, MethodId, VariantId, FunctionId, TraitId, ShapeId, and TypeKey are stable handles
bytecode offsets, source spans, and source maps remain available for diagnostics, profiling, and debugging
ProgramVersion owns bytecode, registry snapshots, debug metadata, profile data, inline-cache state, and compiled code
call frames expose registers, frame maps, and roots for GC, debugging, deoptimization, and hot reload lifetime tracking
host mutation flows through HostRef, HostTargetPlan, PathProxy, HostAccess, and ScriptStateAdapter only
```

Optimization rules:

```text
every optimized path has a VM-equivalent slow path
guards validate dynamic value tags, shapes, schemas, methods, fields, and ProgramVersion assumptions
guard failure is a normal slow-path transition, not a correctness failure
optimized code must charge or preserve ExecutionBudget behavior
optimized code must report or preserve GC roots before allocation, calls, and safe points
optimized code must preserve debugger-visible source locations, frame state, and safe suspension points
optimized code must not bypass HostAccess, reflection policy, permissions, or host access checks
hot reload invalidates version-owned caches and compiled code at safe points
dynamic type hints and TypeFacts guide optimization but are not correctness guarantees
```

The non-JIT performance target is intentionally part of the post-MVP roadmap:
an optimized bytecode interpreter should aim for Lua 5.x comparable performance
on representative host-boundary workloads. LuaJIT and Node.js are useful reference
ceilings for hot scalar loops and future JIT work, but they are not the first
release target.

## Performance Roadmap

### Phase 1: Measurement And Baselines

```text
official microbenchmarks and domain-style host-boundary benchmarks
release-mode benchmark parameters and checksum validation
VM scalar dispatch, function-call, heap, stdlib, record, string, and HostAccess cases
external reference comparison harness for Lua 5.x, LuaJIT, Rhai, and JavaScript
profile capture and bottleneck notes in docs/performance.md
```

Only tracked benchmark sources and fixtures define the official benchmark
surface.

### Phase 2: Non-JIT Optimized Interpreter

```text
dispatch loop tightening
bytecode operand decode cleanup
fast primitive arithmetic, comparison, and branch paths
shape + slot record and enum access
native stdlib fast paths for arrays, maps, sets, strings, Option, and Result
managed heap allocation and materialization reduction
optimized for-in and callback paths
GC pacing and allocation thresholds
simple peephole optimization
precompiled `.vbc` bytecode artifacts and bytecode cache
```

This is the main path toward Lua-comparable performance without JIT. The work
should be benchmark-driven and must not make host access, hot reload,
reflection, or diagnostics less reliable.

Precompiled `.vbc` bytecode artifacts improve startup, deployment validation, and
reload/load latency. They do not by themselves improve the execution speed of
an already-loaded function, because that function already runs as bytecode.

### Phase 2.5: Cache-Ready Architecture Prep

```text
resolved call operands for native, stdlib, script, method, and callback paths
focused VM dispatch modules for host access, script calls, stdlib/method calls, and callback-heavy paths
HostTargetPlan and HostAccess resolved targets or direct adapter thunk boundaries
borrowed Value views at native and stdlib boundaries where semantics allow
verified-bytecode invariants for future unchecked register and operand access
version-owned profile metadata for hot bytecode offsets and later cache invalidation
frame maps, GC-root visibility, budget checkpoints, and host-boundary slow-path contracts for later JIT/deopt work
```

This phase must happen before inline-cache implementation and before any JIT
work. Its purpose is to remove architectural friction, not to add cache state.
It keeps diagnostics, reflection names, and fallback paths intact while moving
hot execution operands toward IDs, slots, or resolved targets.

### Phase 3: Inline Cache And Specialization

```text
inline cache for script field access
inline cache for host field read/write
inline cache for method dispatch and stdlib value methods
small polymorphic cache states
profile counters for hot bytecode offsets
specialized fast paths guarded by shape, schema, and ProgramVersion
cache invalidation on schema ABI or hot reload changes
```

Inline caches are still interpreter technology. They should be version-owned,
cheap to invalidate, and safe to disable for deterministic debugging or
performance investigations.

### Phase 4: Debugger Contracts

```text
runtime debug hooks and suspension points
source breakpoint binding and conditional breakpoint evaluation
frame maps for parameters, locals, captures, registers, and GC roots
watch/evaluate through controlled reflection and host policies
read-only host inspection and host error breakpoints
Debug Adapter Protocol boundary for IDE integration
hot reload breakpoint rebinding through ProgramVersion metadata
```

Debugger support must stay disableable for normal embedded execution. Optimized
interpreter paths, inline caches, and later JIT code must preserve the metadata
needed to reconstruct a bytecode-equivalent debug frame.

### Phase 5: Cranelift JIT

```text
baseline native compilation for restricted hot functions
tag, shape, schema, method, field, and version guards
side exits or deoptimization back to the bytecode VM
compiled frame root maps for GC, debugging, and deoptimization
budget checks in compiled code or side exits to checked VM helpers
host calls routed through existing NativeCallContext and HostAccess helpers
runtime option to enable or disable JIT
```

JIT is not part of the MVP, and it is not required to meet the non-JIT Lua
comparison target. Cranelift is a post-MVP backend milestone after interpreter
correctness, conformance, profiling data, inline caches, and debugger
contracts are stable. It must remain disableable, and VM execution remains the
correctness baseline.

## Security And Sandbox

### Permissions

```rust
pub struct PermissionSet {
    pub reflect: ReflectPermissionSet,
    pub host_read: HostAccessPolicy,
    pub host_write: HostAccessPolicy,
    pub allow_io: bool,
    pub allow_network: bool,
    pub allow_random: bool,
    pub allow_time_now: bool,
}
```

Default embedded script settings:

```text
allow_io = false
allow_network = false
allow_random = false, or only through controlled host context
allow_time_now = false, or only through controlled host context
host_write = only objects provided by the event context
reflect_write = disabled by default or tightly controlled
```

### Budgets

```text
instruction budget
memory budget
max call depth
max reflection lookup count
max host method call count
```

## Testing Strategy

### Unit Tests

```text
lexer tests
parser snapshot tests
parser recovery tests
CST span preservation tests
AST lowering tests
resolver tests
semantic model tests
TypeFact inference tests
bytecode compiler tests
VM instruction tests
Value conversion tests
GC root tests
reflection registry tests
HostAccess tests
ABI diff tests
```

### Integration Tests

```text
script reads host field
script writes host field through HostAccess
reflect::set creates HostAccess
account.balance += 1 creates Add patch
lambda parameter facts are inferred from array/map receiver facts
host schema fields are available through TypeRegistry
hot reload replaces function body
old call frame uses old version
new call frame uses new version
ABI mismatch rejects update
```

### Example Tests

```text
examples/src/bin/<example> directories with standalone main.rs and colocated .vela scripts
  player_level_up
  monster_kill_reward
  quest_progress
  reflect_debug
  hot_reload_function_swap
```

### Validation Commands

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p vela_examples --bin level_up
```

Later:

```bash
cargo bench --workspace
cargo fuzz run parser
```
