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
- The MVP does not include JIT, script async/coroutines, moving GC, or a custom
  full IDE product. A full native LSP capability track is allowed before the
  MVP when it stays analysis-only and does not change language or runtime
  semantics.
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

### Tuple, Unit, And Null Direction

Future breaking value-model cleanup should add Rust-like tuple syntax and
`()` as the unit type/value. Unit should replace the current void-like use of
`null`; expected absence should use `Option`, and recoverable failure should
use `Result`. Ordinary script APIs should not use `null` for no-value,
not-found, or failed results. `?` propagation should stay Rust-aligned:
`Option` propagates through `Option`-returning functions, `Result` propagates
through `Result`-returning functions, and cross-family conversion requires
explicit helpers such as `ok_or`. Raw external null, if retained, must be
explicit at the serde/JSON boundary rather than overloaded as the VM no-value
result. The implementation plan lives in
[tuple-unit-null-refactor-plan.md](tuple-unit-null-refactor-plan.md).

### Source And Artifact Naming

Vela source files use `.vela`. Future precompiled bytecode-only artifacts use
`.vbc`. If a future deployment package contains bytecode plus ABI manifests,
schema metadata, source maps, or reload metadata, it should use a separate
package extension rather than overloading `.vbc`.

### External C ABI Boundary

External binary embedding uses a dedicated `vela_c_api` crate. It is separate
from `vela_hot_reload`: hot-reload ABI describes script/module/schema
compatibility, while `vela_c_api` owns opaque C handles, C-compatible value
layouts, and future host adapter vtables. The C ABI must not expose Rust
references or place Rust host state under script GC.

### Record Field Assignment Roots

Script record field assignment targets use the leftmost receiver expression as
the root and evaluate that root exactly once. This allows `self.field += value`
and expression receivers such as `get_or_put(key).field += value` without
special-casing `self` or requiring a local path root. Host field assignments
still resolve through HostAccess first; non-host record writes mutate script
heap records through record field or slot bytecode.

### Module Imports And Exports

Vela has no source-level `module` declaration. `compile_file(path)` is a
single-script entry mode where the file name is not module identity and the
ordinary entrypoint is `main`. `compile_dir(root)` is the module-graph mode:
each `.vela` file under `root` gets a module path from its relative path, so
`game/reward.vela` is `game::reward`. Imports and qualified calls use `::`;
the final import segment is the declaration name and the preceding segments are
the owning module path.

Public APIs should be imported from the module that owns them. Crate roots
should expose focused `pub mod` entries and avoid broad `pub use` facades unless
the item is an intentional crate identity entrypoint.

`vela_engine::prelude` is the embedding convenience import surface. It may
re-export common Engine, Runtime, native descriptor, host-handle, reflection
permission, and schema descriptor types needed to write host setup code, while
the crate root remains a focused module index.

Single-source embedding APIs do not require callers to provide `SourceId`.
`Engine::compile_source`, text hot-reload compile, and text hot-reload staging
assign internal single-source identity. Explicit source identity remains an
internal compiler/reload concern and belongs to module-graph loading,
diagnostic sources, and crate-local tests that need deterministic source
identity.

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

### Native-First LSP Boundary

Vela's full native LSP capability track is allowed before the MVP and may
progress in parallel with M19/M20 optimization when it stays analysis-only. A
custom full IDE product remains outside the MVP. The primary desktop
integration uses native `vela_lsp_server` binaries so editor tooling can use
platform filesystem watchers, threads, cancellation, and large workspace
indexing. WASM may wrap the reusable language-service core for browser tooling,
but it must not constrain the native server architecture.

`vela_language_service` owns reusable editor analysis: virtual workspace
state, open-document overlays, module graph snapshots, diagnostics,
completion, hover, definitions, schema facts, and incremental invalidation. It
must not depend on LSP protocol types, read the filesystem directly, execute
scripts, inspect live host state, or mutate `TypeRegistry`.

`vela_lsp_server` owns protocol and platform integration: JSON-RPC transport,
document sync, workspace folders, file watching, request cancellation,
progress, and LSP position/range conversion. Editor plugins should stay thin
launchers around this binary. Host facts for editor tooling come from a static
schema artifact exported from `TypeRegistry`/`RegistryFacts`; the server must
not run the host application to discover schema metadata.

Thin editor launchers may pass initialization options that mirror `vela.toml`
using `workspace.roots` and `host.schema`. Those options are a fallback
configuration source for native server startup and later
`workspace/didChangeConfiguration` settings; a discovered `vela.toml` remains
the authoritative project configuration.

Native launch flags mirror the same fallback path: `--root` appends a
workspace root and `--schema` sets the host schema artifact before stdio
transport starts. Client-provided initialization options override those launch
defaults, while `vela.toml` discovery still wins once project configuration is
loaded.

Initial LSP formatting uses source-preserving text edits in
`vela_language_service`: full-document formatting is driven by
`vela_syntax::formatting`, while range formatting only trims trailing
spaces/tabs inside the requested range. Neither path requires a successful
parse. `vela_syntax::formatting` owns stable token/trivia extraction from
parser-token spans and skipped source gaps, and `vela_language_service`
projects that stream into an editor-neutral formatting IR that preserves raw
comments, shebang trivia, spans, and blank-line whitespace groups. The first
full-document formatter normalizes token spacing and brace indentation while
preserving comments. It also tracks declaration-member brace contexts for
initial struct field, enum variant, trait method, impl method, and adjacent
top-level declaration layout. The richer formatter still needs AST-aware range
and on-type formatting rules before it can claim complete semantic formatting
coverage.

When the configured host schema is missing or unavailable, editor tooling
reports a schema diagnostic and treats schema-owned host, record, trait, and
enum receivers as dynamic `Any` for unknown-member diagnostics. Builtin
receiver diagnostics, parser diagnostics, HIR diagnostics, and non-schema
analysis diagnostics should still be published from the available source
facts.

LSP code actions may apply structured quick fixes and source-owned refactors,
but semantic rewrites such as null-check to Option/Result guard conversion
must wait for a structured diagnostic or syntax pattern that proves the edit is
local, source-owned, and semantics-preserving. The server must reject dynamic
receiver typo fixes and ambiguous imports rather than invent type facts or
choose arbitrary declarations.

Schema artifacts may omit `schemaVersion` and `schemaHash` while exporters are
still simple, but any provided metadata is validated at load time. `schemaHash`
is a 64-bit FNV-1a hash of the canonical `RegistryFacts` payload represented by
the artifact, formatted as decimal or `0x`-prefixed hexadecimal. A mismatch is
treated as an invalid or stale schema and host facts degrade to `Any`.

Editor callable facts may expose schema enum tuple variants as constructors
only when the schema fields for `Enum::Variant` are numeric reflected tuple
field names such as `0` and `1`. Named schema variant fields are treated as
record-style fields and must not be ordered into callable parameters until the
schema contract carries explicit constructor shape/order metadata.

The next native LSP cleanup rewrites language-service feature queries around a
shared editor-neutral query model: request context, cursor context, symbol
identity, display parts, edit plans, rich completion items, relevance metadata,
and protocol projection boundaries. This refactor may break and delete the
current coarse completion model, thin completion item shape, feature-local
cursor scanners, and LSP conversion assumptions rather than preserving
compatibility shims. It should borrow rust-analyzer's high-level separation of
context construction, feature producers, item models, and LSP projection while
avoiding Rust-specific macro, trait-solver, and full Salsa complexity unless a
Vela-specific need appears. The execution plan lives in
[lsp-clean-architecture-refactor-plan.md](lsp-clean-architecture-refactor-plan.md).

Semantic highlighting uses an editor-neutral Vela taxonomy in
`vela_language_service` with standard LSP names where they exist and explicit
fallback names for custom token types. Custom tokens such as `builtinType`,
`const`, `global`, `boolean`, `null`, operator families, punctuation families,
and unresolved references keep their Vela-specific names in the primary
legend, while `vela_lsp_server` remains responsible for any future
client-specific fallback projection. Editor packages may contribute fallback
scope metadata, but must not compute semantic classifications.

### Function Identity

Vela does not support function overloading. A module has one function per
script-visible name, and a type or trait has one method per receiver/name pair.
Arity, type hints, default values, and native Rust signatures do not create
overload sets. Resolver, reflection, native registration, and hot-reload ABI
logic should model each function name as a single callable.

Script methods may be declared as inherent type methods with
`impl Type { ... }` or as protocol methods with `impl Trait for Type { ... }`.
Inherent script method IDs are derived from the fully qualified receiver type
and method name. Trait method IDs remain derived from the fully qualified trait
and method name. A receiver type may not have two script methods with the same
name, even if one comes from an inherent impl and another comes from a trait
impl.

Closed builtin comparison traits are VM-recognized protocol names, not open
operator overloading. `PartialEq::eq(self, other)` returns `bool`.
`PartialOrd::partial_cmp(self, other)` returns the standard `Option` enum:
`Option::None` means incomparable, while `Option::Some(i64)` uses negative,
zero, or positive values for less, equal, or greater. Source ordering operators
return `false` for incomparable results. This first-slice return shape avoids a
new standard `Ordering` enum while preserving an explicit incomparability
channel. `Ord::cmp(self, other)` returns `i64` using the same negative, zero,
or positive convention and drives total-order collection helpers such as
`Array.sort`, `Array.min`, `Array.max`, and non-leaf `Array.sort_by` keys.
Leaf scalar/string/bytes sorting remains a runtime fast path, while object
sorting requires `Ord`; floats remain rejected by total-order helpers until an
explicit total-float ordering API exists.

Compiler identity lookup uses the definition registry, not reflection metadata
or `CompilerOptions` identity maps. During the registry migration the engine
keeps a `DefinitionRegistry` compile sidecar derived from validated reflection
and native metadata; source and hot-reload compiler entry points pass a
`RegistryCompileView` so native calls resolve to `FunctionId` before bytecode
emission. Reflection metadata remains the user-visible query surface, while the
definition registry is the compiler/linker identity source.
`CompilerOptions` may carry only non-identity compile settings or capability
hints, such as host index capability metadata and native module roots. It must
not store native function IDs, value method IDs, host type IDs, host field IDs,
host method IDs, or method parameter metadata.

### Primitive Native Contracts

Native parameter type hints are contracts, not conversion requests. Known
primitive parameters are checked by the compiler or by linked runtime guards;
positional native calls may still pass optional or variadic arguments after the
known metadata prefix until the registry has first-class optional/variadic
metadata.

Macro-generated descriptors for Rust `Option<T>` parameters and returns use
`TypeHint::Any` for now. The script-visible value is `null` or the dynamic
standard `Option` enum. This is a macro bridge limitation, not a language
type-hint limitation: source and explicit metadata may express `Option<T>` as a
builtin contract, but the current generated native wrapper keeps Rust
`Option<T>` payload metadata erased until conversion semantics are tightened.
Typed native conversion still decodes the `Option<T>` value at the Rust
boundary.

Embedding float conversions are exact: Rust `f32` maps to Vela `f32`, Rust
`f64` maps to Vela `f64`, and the embedding layer does not silently convert
between integer, `f32`, and `f64` values.

Wrapping arithmetic and bit manipulation are explicit stdlib helper functions
for the primitive refactor checkpoint. Bitwise syntax operators are deferred.
The current representative shift helpers use `u32` shift counts, return zero
when the count is greater than or equal to the left operand width, and rotate
helpers use native modulo-width rotate semantics.

### String Literals And Interpolation

Multiline strings use triple quotes, `"""..."""`, and preserve body text
without indentation trimming. Interpolated strings require an explicit `f`
prefix, as `f"..."` or `f"""..."""`; ordinary strings never interpolate.
Interpolation supports `{expr}` plus escaped literal braces `{{` and `}}`.

Interpolated strings lower to a dedicated format-string bytecode instruction.
They must not lower through numeric `+`, implicit string concatenation, or a
stdlib compatibility helper. Runtime formatting uses the same user-facing
`OwnedValue::display_text()` rule as standard output.

### Runtime And Heap

The VM is a register bytecode interpreter. Execution budgets cover
instructions, memory, call depth, and patches. Runtime budgets keep immutable
limits, mutable counters, and precomputed active flags separate so hot paths
test budget mode directly instead of repeatedly interpreting sentinel limit
values. Script heap values use stable, generation-checked non-moving handles;
host refs and path proxies remain external handles and are not traced as
Rust-owned state.

Execution budgets account for heap collection growth at the mutation boundary
when either memory bytes or explicit collection limits are enabled.
`ExecutionBudget::unbounded()` disables instruction, memory, call-depth, and
collection-growth bookkeeping, so hosts can choose the lower-overhead trusted
path. Array and set budget deltas are based on script-visible element count
rather than spare `Vec` capacity; map deltas are based on script-visible keys
and values. Hosts may add explicit collection length limits for arrays, maps,
and sets independently from the byte budget. Native allocator reserve failures
are runtime allocation errors when the growth-budget path is active.

Typed scalar fast paths are interpreter specializations, not alternate
language semantics. Proven `i64` hot paths may use typed frame slots and fused
typed branch bytecode such as immediate compare or remainder-compare jumps, but
they must preserve the same checked arithmetic, division-by-zero, source-span,
budget, and hot-reload behavior as the generic bytecode path. These
specializations replace pre-release bytecode shapes instead of preserving
compatibility aliases.

Type facts and type hints may select static linked bytecode, field slots, and
guarded inline caches, but they are not required for ordinary dynamic member
access. If a receiver type is unknown, dot field access remains name-based
dynamic bytecode and fails only at runtime when the actual value does not
support the requested member. Linked bytecode must preserve that dynamic path
instead of treating unresolved field slots as link errors.

Map string-literal index bytecode is a source-level lowering for ordinary
`map["key"]` reads and writes. The instruction stores a `ConstantId` pointing
at a string literal, and runtime dispatch borrows that constant directly,
avoiding per-iteration string-object lookup and key cloning. Dynamic string
indexes continue to use the generic index path, and benchmark-specific fused
condition or method-call shapes are not part of this lowering.

`OwnedValue` is the Rust boundary/materialized value name. `Value` is the VM
runtime slot and is `Copy`, containing only scalars or handles. `HeapValue`
stores script heap objects, and heap containers store runtime `Value` entries
directly. There is no separate heap-slot type. Re-export surfaces should stay narrow: embedding
convenience modules may expose `OwnedValue` when it is part of normal host
ergonomics, but internal runtime slot types should remain under their owning VM
modules.

Engine embedding APIs use explicit boundary types. `CallArgs`, `args!`,
prelude exports, registered native functions, typed native conversion traits,
and callable native methods use `OwnedValue` when values cross as detached Rust
data. `Runtime::call` returns a runtime-managed `VelaValue` so hosts can keep
script aggregates on the persistent VM heap by default. VM execution frames,
closures, iterators, heap containers, and internal method dispatch use runtime
`Value`; the engine installs explicit conversion bridges when registering
native functions into a VM. Public VM program entrypoints use `OwnedValue`;
low-level runtime-slot program entrypoints are explicitly named
`run_program_runtime*` and are reserved for VM internals, low-level tests, and
benchmark harnesses. Public program entrypoints convert `OwnedValue` through a
temporary script heap and materialize the return before dropping that heap, so
they do not depend on `Value` retaining owned aggregate variants as a boundary
representation.

Runtime embedding has one high-level return-value surface. `Runtime::call`
returns a runtime-managed `VelaValue` pinned as a persistent runtime heap root.
Hosts can pass that value back into later calls on the same `Runtime` without
materializing or copying the script aggregate, and can explicitly call
`value_to_owned` when Rust needs a detached representation. A `VelaValue`
belongs to the `Runtime` that created it; passing it to another runtime is a
runtime type error. `VelaValue` is still script VM state, not Rust host state,
and it does not expose real Rust references or place Rust objects under script
GC. With the `serde` feature enabled, `Runtime::from_value` deserializes a
`VelaValue` directly from runtime `Value` plus heap state, so Rust can decode
script-owned results into structs/enums/scalars without first constructing a
detached `OwnedValue`.

Semantic object equality and ordering are opt-in through closed builtin
operator traits. `PartialEq` drives user-object `==`/`!=`, `Eq` marks full
equivalence, `PartialOrd` drives ordering operators, and `Ord` drives total
ordering and sorting. User records/structs do not receive implicit structural
equality or ordering; they must implement the builtin trait explicitly or use
explicit derive such as `#[derive(PartialEq, Eq)]` or
`#[derive(PartialEq, Eq, PartialOrd, Ord)]` when every field satisfies the
required traits. Missing support is a compile-time diagnostic when statically
known and a source-spanned runtime error for dynamic values. `Hash` is not a
script-visible builtin trait. `f32` and `f64` implement partial comparison
semantics but do not satisfy `Eq` or `Ord`, so float sorting and float
`Eq`/`Ord` derivation are deferred until a later total-float-order or explicit
partial-sort design.
Reference identity comparison for script heap objects and host refs uses
`===` and `!==`. These operators are not overloadable, do not call user
`PartialEq`/`Eq`/`PartialOrd`/`Ord`, do not call `ValueKey`, and must not read
host state. Statically known non-reference operands are rejected; dynamic
non-reference operands fail with a source-spanned runtime error. `==` and `!=`
must not recursively materialize and deep-compare object graphs; deep equality
belongs in an explicit, budgeted helper if it is added later.

Map and Set key semantics are owned by a focused runtime `ValueKey` layer.
Map keys and Set elements are script runtime `Value`s, but lookup and uniqueness
do not use Rust `Value` equality or user comparison traits directly. Instead,
`ValueKey` follows stable key classes: immutable leaf keys compare by value,
script heap objects and host refs compare by identity, and transient values are
rejected. Mutable records and structs must not use structural or user-defined
business equality as Map/Set keys, because field mutation would make the
container index unstable. Transient mutation proxies such as `PathProxy` are
not keyable until they have an explicit host path identity policy.
Array membership and dedup helpers use the same container-equivalence boundary:
`contains`, `index_of`, and `distinct` compare by `ValueKey`, not by
`PartialEq` or `Eq`. Business equality remains explicit through `==`, `!=`, and
predicate helpers such as `find`, `any`, `filter`, and `count`.

High-frequency embedding can cache script entry lookup with `Runtime::entry`.
The common call API remains `Runtime::call`: a `&str` target performs ordinary
name resolution, while a `VelaFunction` target carries the runtime id, entry
name, active version id, and cached parameter metadata. Runtime execution
resolves to a `CodeObject` before entering the VM so the VM does not repeat the
entry-name lookup on the hot path. Hot reload does not freeze old entry
handles; if the runtime version has advanced, the handle re-resolves by name
against the active program and reports the normal missing-function or ABI
errors if the target is no longer valid.

Rust-side calls to methods on returned `VelaValue` handles use
`Runtime::call_method`. Methods remain type-level script methods keyed by the
receiver script type and stable `MethodId`; there is no per-value method
registration or monkey patching. `Runtime::method` caches the owner type,
method name, method id, version id, and parameter metadata. Calls validate the
receiver runtime and script type, then re-resolve by method id when the active
version changes.

With the `serde` feature enabled, Rust structs and enums that implement serde
traits can cross the ordinary script-owned value boundary explicitly through
`to_owned_value`, `from_owned_value`, `CallArgs::with_serde_value`, and
`Runtime::insert_global`. This path serializes Rust data into Vela-owned
records, enums, arrays, maps, sets, and scalars. It is a
snapshot/data-transfer path for messages, configs, globals, and return values,
not a host-state binding: script mutation of the value does not write back to
the original Rust object unless Rust deserializes a returned value and applies
it itself. Host state that must be mutated in place still uses `HostRef`,
`HostPath`, `PathProxy`, and `HostAccess`.

`Runtime` and `VelaValue` are `Send` so hosts can move a runtime and retained
script values into worker or actor threads. They are not a concurrent execution
model: script calls still require mutable runtime access, and one runtime must
not be called concurrently. Persistent host globals stored inside a runtime
therefore require `Send`; call-scoped direct host references remain local to
that invocation.

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
runtime-managed `VelaValue`. Host mutation counting is not part of the default
host boundary; hosts that need diagnostics should instrument their adapter or
domain operations directly.
The shortest runtime method name, `Runtime::call`, is reserved for this common
high-level `CallArgs -> VelaValue` path. Lower-level entrypoints that expose
adapter or `HostAccess` internals use explicit names such as `call_with_adapter`,
`call_raw`, and `call_args_raw`.

Mutable cross-call script globals are host-managed declarations, not module
`let` or `static` initializers. Scripts declare globals as ordinary module
items, for example `pub global state: ServerState`; the declaration contributes
ABI/name/type metadata and Rust inserts a runtime instance under the fully
qualified name such as `game::state::state`. Rust-defined globals load as
persistent host-object roots and then use normal `HostRef`, `HostPath`,
`ScriptStateAdapter`, and write-through `HostAccess` semantics. Vela-defined
script-value globals use the same declaration surface but are stored as
persistent VM heap roots owned by `Runtime`; Rust constructs, reads, replaces,
or updates them through one short embedding API: `insert_global` accepts
`OwnedValue`, serde values passed by reference, and `VelaValue` handles from
the same runtime. Rust-side construction supports explicit constructors such as
`OwnedValue::record`, convenience macros such as `owned_record!`, and serde
struct/enum conversion. `VelaValue` insertion attaches the runtime-managed value
as a global root without first materializing a detached `OwnedValue`. The other
public runtime methods remain `set_global`, `global`, `global_as`, and
`update_global`; host-object globals keep their explicit host-specific API.
The VM receives script globals as a concrete runtime value map rather than an
extension trait, because there is only one runtime-owned script global store.
Declared globals compile to `GlobalSlot` operands for the runtime hot path;
the fully qualified global name remains in bytecode for diagnostics and
fallback. Runtime-owned script globals and Rust-owned host globals both maintain
slot tables, so a resolved global read avoids string map lookup on the common
path.
There is no special `global.vela` file, top-level mutable initialization, or
script-owned Rust state under GC.

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

### Indexed For-In

`for index, value in iterable` is syntax-level sugar over the existing `for-in`
lowering, not an eager `enumerate()` collection method or a Rust-style iterator
adapter. The exposed index is the source iteration position. If the value
pattern skips an item, later matching iterations keep their original source
indexes instead of being renumbered by body execution count.

### Example Layout

Runnable examples live in the `vela_examples` workspace package as standalone
Cargo bins under `examples/src/bin/<example>/`. Each example keeps its `main.rs`
and `.vela` source files in the same directory so users can inspect and run one
capability without following a parameter-dispatched demo runner or a separate
script tree.

### CLI Role

`vela_cli` is the final direct script execution binary, analogous to a language
runtime command. It must stay domain-neutral and must not embed example host
state such as Player, Monster, Context, or permission-denial fixtures. Host
world demos belong in `vela_examples`; `vela_cli <script.vela>` compiles the
file, runs `main()` with no host arguments, and prints the returned value.

### Package And Service Providers

Vela plugin discovery should use package manifests plus a trait-backed service
provider catalog, not script-side runtime `require`, `eval`, or directory
scanning. Package manifests own source roots, path dependencies, package
identity, and requested capabilities. Module identity is package-aware:
`PackageId + ModulePath`; `SourceId` remains internal.

Service providers are explicit trait implementations exported with
`#[provider(id = "...")]`. The service trait is inferred from
`impl ServiceTrait for ProviderType`; the attribute carries only the stable
provider identity and export intent. Provider identity is
`PackageId + ServiceTraitId + ProviderId`, so provider type renames do not
change host-visible SPI identity. First-slice package dependencies are path
dependencies only; foreign host-language modules, remote registries, version
solving, and script-side package loading are deferred.

### Opt-In IO Stdlib

I/O is an Engine-side native stdlib extension, not a VM-default primitive.
Embedders must opt in with `with_stdio()` and/or `with_fs_io(root)` and grant
`io_read`/`io_write` capabilities. Filesystem helpers operate only on relative
paths inside the configured sandbox root. Ordinary filesystem failures return
script-visible `Result::Err(IoError)` values; capability failures and runtime
type errors remain VM diagnostics.

### Public Docs And Playground

Public documentation lives in `site/docs/{en,zh}` as bilingual Markdown, and
the GitHub Pages site is static HTML/CSS/JS without a frontend build system.
The browser playground uses a dedicated `vela_playground_wasm` crate compiled to
`wasm32-unknown-unknown`; Pages generates `site/pkg` with `wasm-bindgen` during
deployment rather than committing generated browser bindings.

The playground WASM boundary returns stable JSON strings for compile/run
results and diagnostics. It enables standard natives plus controlled time and
random capabilities, but does not expose host mutation, filesystem I/O, or host
state in the browser sandbox.

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

### Value Method Identity

Value method compilation resolves receiver value facts to stdlib `TypeId`
definitions and then resolves methods through the `DefinitionRegistry`.
`CallMethod` carries a typed `MethodId` for value methods when a registry view
is available. Named argument metadata and method identity come from method
definitions in the registry, not from `CompilerOptions`.

### Host Definition Runtime IDs

Host types, fields, and methods register into `DefinitionRegistry` with
semantic IDs derived from canonical `DefPath`. Adapter-facing runtime IDs such
as `HostTypeId`, host `FieldId`, and `HostMethodId` are stored as host runtime
metadata on those definitions and are used only when emitting current
`HostTargetPlan` and host call operands. This keeps registry identity globally
deterministic while preserving existing HostAccess adapter contracts.

### Unlinked Bytecode Naming

Compiler output bytecode is named `UnlinkedProgram`, `UnlinkedCodeObject`,
`UnlinkedInstruction`, and `UnlinkedInstructionKind`. These types may still be
consumed by current runtime image and VM paths until the linked-bytecode phase
lands, but new compiler-facing APIs should use the unlinked names and must not
reintroduce ambiguous `Program` or `CodeObject` output types.

### Linked Bytecode Shape

Executable bytecode is represented by `LinkedProgram`, `LinkedCodeObject`,
`Instruction`, and `InstructionKind`. Linked instructions carry dense runtime
handles or slots such as `NativeHandle`, `ScriptFunctionHandle`,
`MethodDispatchHandle`, `TypeHandle`, `VariantHandle`, and `FieldSlot`.
Human-readable names live in a `DebugNameTable` side table and linked
instructions reference them by `DebugNameId` only.

### Linked Bytecode Linker

`vela_bytecode::linker` converts `UnlinkedProgram` values into
`LinkedProgram` values. Native functions, methods, script functions, types,
and variants are stored in linked side tables owned by the linked program, and
instructions carry only dense handles, slots, host target plan IDs, or global
slots. Name-only method and record/enum field bytecode is rejected by
`LinkError` instead of being preserved as runtime fallback dispatch.

### Linked Bytecode Verification

Linked bytecode verification checks local register, constant, jump,
cache-site, and host target invariants plus linked-program side-table
references. Invalid debug names and out-of-bounds native, script function,
method dispatch, type, or variant handles are rejected before linked bytecode
can become executable.

### Linked Closure Ownership

Linked closures store `ScriptFunctionHandle` values and execute only with the
owning `LinkedProgram` that contains those handles. Higher-order stdlib
callbacks must carry linked-program context through `MethodRuntime`; they must
not reconstruct unlinked bytecode or rely on script-provided method names.

### Nested Linked Function Handles

The linker assigns nested `ScriptFunctionHandle` values in the same order that
linked nested functions are appended to the linked program side table. Recursive
linking must not reserve handles before recursively appending child functions,
because transitive closures would otherwise point at the wrong code object.

### Primitive Embedding Conversions

Rust embedding conversions preserve concrete scalar tags exactly. A Rust
`i32` argument becomes Vela `i32`, not `i64`; `HostValueInto`/`HostValueFrom`
use the same exact-tag rule for host fields and methods. Callers that intend an
`i64` contract must pass an explicit `i64` value, and HostAccess arithmetic
rejects mixed scalar tags instead of widening or narrowing.

Rust `Vec<u8>` and byte slices cross embedding and host boundaries as the
`bytes` primitive. Other `Vec<T>` values remain arrays; `Vec<u8>` decode
expects `OwnedValue::Bytes`/`HostValue::Bytes` instead of accepting an array of
`u8` scalars as an implicit conversion.

Serde owned-value conversion preserves primitive tags exactly. Rust `i8`,
`u32`, `u64`, `f32`, and the other scalar primitives become matching
`ScalarValue` variants, and deserialization expects the same concrete tag
rather than widening, narrowing, or integer-float conversion. `u64::MAX` is a
supported exact boundary value.

Serde byte buffers use the explicit Serde bytes hook (`serialize_bytes` /
`deserialize_byte_buf`) to cross as `OwnedValue::Bytes`. With `serde_json`,
that hook is represented as a JSON byte array, not base64 or hex. Large
unsigned integers use JSON integer text and must round-trip through Rust
`serde_json` as `u64` without precision loss; JavaScript-number-safe encodings
would require an explicit future config rather than a hidden conversion.

The C ABI value surface uses explicit primitive tags (`I8` through `U64`,
`F32`, `F64`) instead of old `Int`/`Float` tags. C arguments are copied into
Vela-owned `OwnedValue` values before execution; returned strings and bytes are
ABI-owned buffers that callers must release with `vela_value_free`, or with
the specific `vela_string_free` / `vela_bytes_free` helper when they own the
raw pointer directly.

Hot-reload function, method, trait, and schema compatibility checks normalize
primitive type hints through `PrimitiveTag` before comparing contracts.
Changing any primitive contract, such as `i32 -> i64`, `i64 -> u64`,
`f32 -> f64`, or `Bytes -> String`, is incompatible unless a future explicit
product compatibility rule is added. Report rendering may still use hint text
for diagnostics, but compatibility decisions must not depend on old `int` or
`float` names.

Host schema derive inference emits exact supported primitive hint names from
Rust field types (`i8` through `u64`, `f32`, and `f64`). Platform-sized or
unsupported wide Rust integer fields such as `usize`, `isize`, `i128`, and
`u128` do not receive an inferred primitive hint; callers must provide an
explicit supported contract instead of relying on an alias or hidden
conversion.

### Controlled Dynamic Method Dispatch

Unknown-receiver calls with a source-static method name are first-class linked
dynamic method calls, not legacy name-only fallback. Static known receiver
calls keep the `MethodId` / linked-dispatch fast path, and statically provable
missing methods may remain compile-time diagnostics. Runtime dynamic dispatch
resolves through controlled standard, script, or host metadata, preserves
source argument names until target lookup, reports source-spanned runtime
errors, and guards inline caches by receiver identity plus schema/hot-reload
epochs where applicable.

### Benchmark Comparison Modes

External language comparison rows must report their execution mode. Vela uses
`internal_hot_loop`, embedded Lua 5.4 and Rhai use `embedded_hot_loop`, and
Node.js/Python 3 use `process_hot_loop`. Mixed-mode benchmark rows are
directional references and must not be collapsed into one fairness ranking or
mixed with VM cache-delta rows.

### Typed Scalar Bytecode

The first non-JIT scalar specialization tier is verified `i64` bytecode emitted
from compiler-owned type facts. Dynamic or mixed numeric operands stay on
generic scalar bytecode, and typed operations preserve checked arithmetic,
source spans, hot-reload compatibility, and HostAccess boundaries. Direct
integer range loops may use `I64RangeNext`; broader numeric matrices and
superinstructions require separate measured justification.

I64 immediate comparisons use a single comparison opcode carrying the compare
operator. Arithmetic-with-immediate bytecode, such as remainder by a constant,
must stay separate from compare/jump bytecode unless a future profiling pass
justifies a broadly reusable superinstruction family. Do not add
benchmark-shaped fused opcodes such as remainder-by-immediate plus equality
plus jump.

Superinstructions must be lowered only when the compiler can prove the fused
condition shape directly or prove that removed temporary registers are not
observable. Do not add post-compile fused rewrites from adjacent opcodes alone.

### Runtime Scalar Value Layout

The VM runtime `Value` enum stores primitive scalar tags as direct variants
(`I8` through `U64`, `F32`, and `F64`) instead of wrapping a nested
`ScalarValue`. `ScalarValue` remains the boundary representation for
`OwnedValue`, `HostValue`, constants, reflection, serde, C/API-facing values,
and diagnostics. Runtime-to-boundary conversion must go through
`Value::from_scalar` and `Value::as_scalar` rather than reintroducing a nested
runtime scalar variant.

### First-Class Char Primitive

`char` is a first-class primitive with Rust `char` semantics: one Unicode
scalar value, not a byte and not a one-character string. Vela uses single-quote
char literals such as `'x'` and `'\u{5956}'`; double-quote literals remain
strings. String iteration yields `char` values. The pre-release implementation
does not preserve the old internal behavior where serde decoded
single-character strings as Rust `char`. Minimal char methods mirror Rust names
for common operations: `to_string`, `is_whitespace`, `is_ascii`, and
`is_ascii_digit`.

### Rust-Like String Indexing

Vela strings follow Rust `str` indexing semantics. `string.len()` returns byte
length, `string.find(needle)` returns an optional byte index, and
`string.slice(start, end)` uses a byte range that must land on UTF-8 character
boundaries. Character-level traversal uses `for ch in text`, yielding
first-class `char` values. Vela does not expose a `char_at` random-access API
because UTF-8 character indexing is O(n) and would misrepresent performance.

### String Parse Surface

String parsing methods use exact primitive names:
`parse_i8`, `parse_i16`, `parse_i32`, `parse_i64`, `parse_u8`, `parse_u16`,
`parse_u32`, `parse_u64`, `parse_f32`, `parse_f64`, `parse_bool`, and
`parse_char`. Each returns `Option<T>`. Integer parsers reject invalid or
out-of-range text, float parsers reject invalid, `NaN`, and infinite values,
`parse_bool` accepts only `true` and `false`, and `parse_char` accepts exactly
one Unicode scalar value.

### Iterator View Naming

Explicit one-shot iterator creation uses `values()` / `iter()` for arrays,
sets, and bytes, `iter()` for maps and ranges, and `chars()` / `bytes()` for
string traversal. Direct bytes `for-in`, `bytes.iter()`, and `bytes.values()`
yield `u8` values. Direct map `for-in` and `map.iter()` yield
`MapEntry { key, value }` records in key order, matching Rust's key/value map
iteration model without exposing references. `map.keys()` and `map.values()`
are explicit projection views, and `map.entries()` is equivalent to
`map.iter()`.

### Iterator Adapter Ownership

Lazy iterator adapters are one-shot cursors that take ownership of the source
iterator state and leave the original iterator exhausted. Adapter stepping,
`for-in`, and terminal methods use the callback-capable method runtime so
`map`, `filter`, `any`, `all`, `find`, and `collect_array` share callback
dispatch, heap-root protection, budget, and host-access behavior.
Iterator terminals that materialize collections are explicit:
`collect_array`, `collect_set`, and `collect_map`. `collect_map` consumes
`MapEntry { key, value }` records and duplicate keys follow map insertion
semantics, so later entries overwrite earlier entries.

### Iterator Source Bounds

Collection-backed iterators read source heap slots lazily instead of copying
the full collection at creation. Arrays and sets snapshot traversal length, and
maps snapshot traversal keys, so later writes to existing items are observed
while later growth does not extend the iterator.

### Public Type Hint Spelling

Public script type hints use lowercase only for scalar/literal primitive
contracts such as `null`, `bool`, `char`, `i64`, and `f64`. Erased dynamic,
text/binary, collection, callable, and Option/Result contracts use capitalized
names: `Any`, `String`, `Bytes`, `Array`, `Map`, `Set`, `Range`, `Iterator`,
`Function`, `Closure`, `Option`, and `Result`.

Only builtin type-hint contracts may be parameterized:
`Array<T>`, `Set<T>`, `Map<K, V>`, `Iterator<T>`, `Option<T>`, and
`Result<T, E>`. They exist to make contracts, diagnostics, static facts,
bytecode guard metadata, mutation checks, embedding metadata, reflection, and
hot-reload ABI precise without introducing a general script generic system.
`Map<K, V>` keys and `Set<T>` elements use the runtime `ValueKey` keyability
contract: immutable leaf values compare by value, script heap objects and host
refs compare by identity, and transient values such as `PathProxy` are
rejected. User/schema/host generics such as `Player<T>`, scalar
parameterization such as `String<T>`, and callable signature syntax such as
`Function<T>` remain rejected. Unparameterized `Array`, `Map`, `Set`,
`Iterator`, `Option`, and `Result` remain valid erased contracts.

### LSP On-Type Formatting Scope

Native on-type formatting is conservative: it may respond to closing brace and
newline triggers, but edits must be limited to the current brace-delimited
construct or a current-line fallback. Broader AST-aware reflow remains a later
formatter capability and must not be reached through whole-document edits while
the user is typing.

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
