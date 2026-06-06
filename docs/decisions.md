# Decisions

This file is the active architecture decision index. Full pre-compaction
decision history lives in
[archive/decisions-full-2026-06-01.md](archive/decisions-full-2026-06-01.md).

## Standing Constraints

- Script-language generics are not supported.
- Function overloading by arity, type hint, or native signature is not
  supported.
- Scripts never receive real Rust `&mut T` references.
- Host mutation must go through `HostRef`, `HostPath`, `PathProxy`, and
  `HostAccess`.
- Reflection can query metadata and perform controlled reads, writes, and
  calls, but cannot mutate runtime type structure or implement monkey patching.
- The MVP does not include JIT, script async/coroutines, moving GC, or a full
  LSP.
- Pre-release code should replace obsolete internal APIs instead of preserving
  compatibility shims. Product-level hot reload ABI and schema compatibility
  checks remain required.
- Ordinary active source files should stay under 1200 lines unless a clear
  exception is documented. Over-threshold implementation and test files should
  be reviewed and split by responsibility when no exception exists.
- `crates/vela_vm/src/execution.rs` may exceed the ordinary 1200-line threshold
  when it remains opcode dispatch glue. New semantic work should still move
  into focused VM modules, and the dispatch loop should only decode operands,
  charge budgets, preserve source spans, update control flow, and call those
  boundaries.
- Standard library and builtin APIs must remain domain-neutral. Game-specific,
  commerce-specific, or other business-domain capabilities belong in Engine
  host registration, native functions, schemas, or examples, not in builtin
  language surface.
- Runtime call budget presets should stay domain-neutral. Hosts should choose
  per-script or per-call budgets explicitly with `CallOptions::new(...)`;
  `CallOptions` intentionally has no default preset.
- Runtime authorization uses coarse capability profiles, not arbitrary
  business permission strings. Native and standard-library execution checks
  compare effect bits against the engine `CapabilitySet`; business-domain
  isolation is primarily controlled by what host surface the embedding
  registers.

## Active Architecture Decisions

### Source And Artifact Naming

Vela source files use `.vela`. Future precompiled bytecode-only artifacts use
`.vbc`. If a future deployment package contains bytecode plus ABI manifests,
schema metadata, source maps, or reload metadata, it should use a separate
package extension rather than overloading `.vbc`.

### Module Imports And Exports

Public APIs should be imported from the module that owns them. Crate roots
should expose focused `pub mod` entries and avoid broad `pub use` facades unless
the item is an intentional crate identity entrypoint.

`vela_engine::prelude` is the embedding convenience import surface. It may
re-export common Engine, Runtime, native descriptor, host-handle, reflection
permission, and schema descriptor types needed to write host setup code, while
the crate root remains a focused module index.

Rust source may use one direct-parent `super::...` reference inside a local
module group. Multi-level `super::super` paths are prohibited; cross-subsystem
imports should use explicit `crate::...` paths.

### Source Pipeline

The syntax layer owns tokens, AST, parser recovery, and source spans. HIR owns
module graph resolution, declaration IDs, binding maps, type-hint metadata, and
top-level semantic diagnostics. The bytecode compiler consumes HIR diagnostics
and metadata before bytecode emission.

There is no separate public IR crate yet. `HIR + TypeFacts + bytecode` is the
current semantic pipeline; a lower IR/MIR should only be introduced when
optimization, CFG/data-flow, register allocation, or lowering complexity
requires it.

### Function Identity

Vela does not support function overloading. A module has one function per
script-visible name, and a type or trait has one method per receiver/name pair.
Arity, type hints, default values, and native Rust signatures do not create
overload sets. Resolver, reflection, native registration, and hot-reload ABI
logic should model each function name as a single callable.

### Runtime And Heap

The VM is a register bytecode interpreter. Execution budgets cover
instructions, memory, call depth, and patches. Script heap values use stable,
generation-checked non-moving handles; host refs and path proxies remain
external handles and are not traced as Rust-owned state.

`OwnedValue` is the Rust boundary/materialized value name. `Value` is the VM
runtime slot and is `Copy`, containing only scalars or handles. `HeapValue`
stores script heap objects, and heap containers store runtime `Value` entries
directly. There is no separate heap-slot type. Re-export surfaces should stay narrow: embedding
convenience modules may expose `OwnedValue` when it is part of normal host
ergonomics, but internal runtime slot types should remain under their owning VM
modules.

Engine embedding APIs, including `Runtime::call`, `args!`, prelude exports,
registered native functions, typed native conversion traits, and callable native
methods, use `OwnedValue` at the public Rust boundary. VM execution frames,
closures, iterators, heap containers, and internal method dispatch use runtime
`Value`; the engine installs explicit conversion bridges when registering
native functions into a VM. Public VM program entrypoints use `OwnedValue`;
low-level runtime-slot program entrypoints are explicitly named
`run_program_runtime*` and are reserved for VM internals, low-level tests, and
benchmark harnesses. Public program entrypoints convert `OwnedValue` through a
temporary script heap and materialize the return before dropping that heap, so
they do not depend on `Value` retaining owned aggregate variants as a boundary
representation.

The compiler may replace a multi-instruction source-level lowering with one
semantics-equivalent bytecode instruction, such as `Truthy` for dynamic
truthiness coercion. Execution budgets are charged against the emitted bytecode
instructions, and optimized opcodes must preserve the same host, reflection,
GC-root, hot-reload, and diagnostic boundaries as their expanded VM sequence.

Before inline caches or JIT work, hot dispatch operands should move from
script-visible strings to stable IDs, slots, reusable path keys, or resolved
call targets. Names remain available for diagnostics, reflection, and source
reports, but they should not be the primary runtime key for hot native,
stdlib, script function, method, record-field, or host-path dispatch.

Managed heap entrypoints materialize return values at API boundaries. Native
calls materialize heap-backed values as needed so existing host/native APIs do
not own script GC state.

Read-only runtime access should avoid materializing owned boundary values.
After the `Value` / `OwnedValue` split, stdlib helpers read compact runtime
`Value` entries from heap objects directly. Mutable accessors, callback calls,
host/native interfaces, GC tracing, and hot-reload ABI remain separate
boundaries.

### Host Boundary

Host state is mutated through call-scoped `HostAccess` operations. Direct host
field, host path, and host method bytecode routes through `HostExecution`,
`ScriptStateAdapter`, and `HostAccess`; the adapter is updated immediately and
`HostAccess` does not retain a journal or mutation counter. There is no patch
descriptor, overlay, journal, host-write count budget, or end-of-call apply step in
the default host boundary.

Embedding APIs may accept Rust `&T` and `&mut T` at a `CallArgs` invocation
boundary, but these references are immediately represented inside the VM as
call-scope `HostRef` handles. Field access still goes through a
`ScriptHostObject`/adapter surface and `HostAccess`; `&T` is read-only and
`&mut T` enables write-through mutation without exposing the real reference to
script code.

Host path map keys store the script string key, not an opaque VM symbol. This
lets directly injected Rust objects and generic adapters resolve
`player.inventory["gold"]` without reaching back into VM symbol interners.
Host object method dispatch receives the full receiver `HostPath`, so root
methods, child collection methods, and trait-object field methods share the
same registration and permission model.

`#[derive(ScriptHost)]` owns generated direct-object field/path access for all
script-visible host fields. Plain `get`/`set` field metadata also means the
field participates in generated direct host path access. `#[script_methods]`
owns generated direct-object method dispatch for `&self` and `&mut self`
receiver methods; method arguments cross the host boundary through scalar
`HostValue` conversions. Child receiver method calls are forwarded through
script-visible fields by default.

Host collection and trait-object surfaces use the same concrete host type
registration model as structs. Rust-side helpers may generate concrete specs
for `HashMap<K,V>`, `HashSet<T>`, `Vec<T>`, or trait-object fields, but scripts
do not see generics and the builder does not expose separate collection-specific
registration APIs. Optional index support is type metadata on the concrete host
schema. Host method parameters that refer to other host objects use typed path
wrappers such as `TypedHostRef<T>` and `TypedHostMut<T>`, which store
`HostPath` only and never expose Rust references to scripts.

High-level embedding calls construct `HostAccess` internally and return a
`CallOutput` that dereferences to the script return `OwnedValue`. Host mutation
counting is not part of the default host boundary; hosts that need diagnostics
should instrument their adapter or domain operations directly.
The shortest runtime method name, `Runtime::call`, is reserved for this common
high-level `CallArgs -> CallOutput` path. Lower-level entrypoints that expose
adapter or `HostAccess` internals use explicit names such as `call_with_adapter`,
`call_raw`, and `call_args_raw`.

There is no default end-of-call apply or automatic rollback. If a script writes
a host field and later traps, the earlier Rust-side mutation remains. PathProxy
wraps HostPath and uses HostAccess, but complex Rust objects remain handles
and paths; the high-frequency host field boundary accepts only scalar
HostValue conversion. Owned complex script values cross through explicit
serialization/owned-value paths.

`ScriptHost` derives may declare reflected host trait implementations with
static `implements` metadata. This records TypeRegistry trait metadata for
reflection and ABI/schema hashing; it does not create script monkey-patching or
runtime trait-structure mutation.

### Reflection

Reflection metadata is copied, permission-aware, and read-only with respect to
type structure. TypeRegistry descriptors are the source for reflected types,
fields, methods, traits, variants, modules, functions, source spans, docs,
attributes, effects, access, and reflection-tool permissions.

Function descriptors keep public export status separate from reflection
visibility and reflective callability. Private functions may be visible to
authorized reflection tooling without becoming public API or reflective call
targets, and hot-reload ABI checks compare those access bits explicitly.

Reflective reads, writes, and calls resolve descriptor metadata to stable IDs
and route host interaction through HostAccess. Private, effectful, host path, and
field-level operations require explicit reflection permissions.

### Capability Profiles

The engine runtime exposes a domain-neutral `CapabilitySet` and named
`ExecutionProfile` constructors. Capability bits include host read/write,
event emission, deterministic time, controlled random, and controlled
reflection effects. Native and context calls declare `EffectSet`; pure calls
take the fast path, while effectful calls require the corresponding capability
bit before execution.

Fine-grained business permission strings are not part of the runtime native
call hot path. Hosts that need strict isolation should register only the native,
context, schema, and reflection surface that a script may use, then choose a
coarse execution profile for the allowed effect classes. Reflection's own
`ReflectPermissionSet` remains a tooling/policy model for metadata visibility
and controlled reflection operations; it must not be used as host business
authorization for native execution.

### Macro Stable IDs

User-facing host and native macros do not accept manually chosen numeric stable
IDs. `ScriptHost` and `ScriptReflect` derive type and field IDs from the
script-facing stable type path and field name, while `#[script_methods]` and
native function macros derive method/function IDs from the owner path or public
`::` qualified function name. Optional `alias` values are the compatibility mechanism
for rename-safe schema evolution. Low-level descriptor constructors may still
take explicit IDs for engine internals and focused tests.

Script-owned struct and enum payload fields are reflected as writable by
default because script values can be copied and updated without touching host
state. Copy-returning `reflect::set` for script values still enforces
`reflect_writable` and field-level required permissions, while HostRef
`reflect::set` additionally requires host field writability before recording a
HostAccess write.

Global field reflection enumerates both type-level fields and enum variant
payload fields. Variant payload field metadata uses `Type::Variant` as the
owner, matching targeted variant reflection, and policy filtering applies to
each field before it appears in `reflect::fields()`.

### Static Path Syntax

Vela uses `::` for static namespace paths: imports, type paths, enum variant
paths, native module functions, macro schema paths, and reflection module or
function identities. `.` is reserved for runtime value access such as fields,
methods, host paths, and metadata record fields. Dotted text remains valid as
ordinary data, for example event names and permission keys.

### Hot Reload

Hot reload replaces function-level or module-level code objects at safe points.
Old ProgramVersion handles keep old code alive, rejected updates do not advance
versions, and reports carry copied diagnostics plus ABI details.

Compiled updates may be staged before a safe point. Staging never advances the
active ProgramVersion; hosts must call the runtime reload check at event, tick,
or explicit call-boundary safe points to consume the pending update and receive
the accepted or rejected report. Host mutations write through immediately via
`HostAccess` and `ScriptStateAdapter`, so reload checks do not commit, inspect, or
rewrite patch journals; `HostAccess` does not retain one by default.

Function, method, module, trait, schema, effect, access, parameter, return, and
source-span metadata participate in ABI validation. Engine registries are the
source for host/native ABI manifests.

Accepted hot-reload reports distinguish actual bytecode-changed functions from
source-changed modules. Module impact is derived from deterministic source
hashes and reverse import dependencies so hosts can invalidate module-scoped
caches without treating every recompiled function as changed.

Changed-file hot reload events are watcher ergonomics, not partial compilation.
The engine validates the changed `.vela` path, then recompiles the full module
root so import resolution, dependency impact, and ABI checks always see the
complete module graph.

### Standard Library And Dynamic Types

Option and Result are dynamic enum-shaped values, not script generics. Stdlib
helpers and analysis TypeFacts may describe dynamic payloads, but the language
surface remains non-generic.

Script type hints are advisory metadata for analysis, reflection, dispatch
hints, and ABI. They do not enforce script-local runtime value types unless a
host, native, or schema boundary explicitly performs conversion or validation.
Function return annotations are optional and have the same metadata-first
semantics.

`null` is retained for no-value, void-like results, host nullable boundaries,
and missing metadata. Expected absence should use `Option::None`, recoverable
business failure should use `Result::Err`, and unrecoverable script/runtime
failures should use VM diagnostics rather than `Result::Err`.

Array, map, set, string, range, math, context, random, and other
domain-neutral helpers are deterministic unless an Engine-installed
capability-gated native explicitly provides controlled nondeterminism.

Host-provided deterministic time belongs to the `time` stdlib module
(`time::now`, `time::tick`, `time::elapsed_since`). `ctx` remains available for
host-registered context objects, fields, methods, events, and logging examples,
but it is not the builtin time module namespace.

### Reflection Permissions

The core reflection policy API owns base call authorization. Direct reflective
method calls and reflected function invocation must require
`reflect::call_methods` before checking callable metadata, required host
permissions, or effect-specific call permissions.

### Analysis And Tooling

TypeFacts, completions, hover, match exhaustiveness, effect diagnostics, null
narrowing, Option/Result predicate narrowing, and pattern diagnostics are
analysis/tooling data. They should not change VM semantics unless a separate
compiler/runtime decision says so.

### Example Layout

Runnable examples live in the `vela_examples` workspace package as standalone
Cargo bins under `examples/src/bin/<example>/`. Each example keeps its `main.rs`
and `.vela` source files in the same directory so users can inspect and run one
capability without following a parameter-dispatched demo runner or a separate
script tree.

### Debugger Support

Debugger support is a post-MVP runtime and Debug Adapter Protocol capability,
not a script-language feature. Runtime debug hooks may expose source
breakpoints, stepping, stack frames, watches, safe HostRef display, HostAccess
preview, and hot-reload breakpoint rebinding, but they must respect reflection,
host access, HostAccess, and TypeRegistry boundaries.

Bytecode code objects carry read-only frame maps for debugger and diagnostic
inspection. These maps may name parameters, locals, pattern bindings, and
captures with their registers and source spans, but they must not affect VM
execution or allow runtime mutation of type or host structure. Runtime stack
frames should preserve caller bytecode offsets as observational metadata for
stepping, profiling, and future breakpoint rebinding. Runtime call frames
should also keep register-to-GC-root metadata separate from collection policy
so debuggers and future optimized backends can inspect roots without changing
which values the collector preserves.

### Cranelift JIT

Cranelift JIT is a mandatory post-MVP backend after interpreter optimization,
inline caches, debugger contracts, and conformance are stable. JIT must remain
disableable, must be semantically equivalent to VM execution, and must preserve
ExecutionBudget, GC roots, HostAccess, reflection policy, hot reload invalidation,
and debugger-visible frame/source metadata.

## Validation Rules

- Multi-level `super` scan must return no matches:

```bash
rg -n '(super::){2,}|super\s*::\s*super' crates examples tests --glob '*.rs'
```

- Remaining `pub use` entries should be deliberate API surface:

```bash
rg -n '^\s*pub use\b' crates --glob '*.rs'
```

## Update Rules

- Add or update entries here when a change creates a durable architecture rule,
  compatibility policy, naming convention, module boundary, or semantic
  constraint.
- Do not record routine implementation steps, small refactors, or test-only
  details here.
- Keep active entries concise. Move detailed historical rationale to
  `docs/archive/` when this file stops being quick to scan.
