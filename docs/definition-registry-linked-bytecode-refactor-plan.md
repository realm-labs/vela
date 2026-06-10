# Definition Registry and Linked Bytecode Refactor Plan

> **Track:** breaking-change definition-registry and linked-bytecode refactor
> **Primary goal:** rebuild Vela’s definition identity, stdlib registration, registry, linker, bytecode, and VM dispatch model around a clean architecture.
> **Breaking refactor policy:** do **not** preserve old IDs, old bytecode, old serialized `ProgramImage`, old public APIs, or old fallback behavior. Prefer deleting legacy layers over adapting them.
> **Audience:** Codex and engineering agents executing the refactor task-by-task.

---

## 0. Executive Summary

The current Vela codebase is already moving toward ID-first dispatch, but the architecture still mixes several concepts:

```text
source spelling        e.g. "math::max"
semantic identity      e.g. FunctionId / MethodId / TypeId
runtime dispatch key   e.g. dense table index / handle
diagnostic name        e.g. name shown in errors and reflection
```

The immediate smell is `crates/vela_common/src/standard_ids.rs`, which contains a large hand-written list of stdlib IDs. However, the deeper issue is not just that the IDs are handwritten. The deeper issue is that stdlib identity, VM registration, engine metadata, compiler lookup tables, reflection metadata, and runtime fallback behavior are maintained in separate places.

The clean architecture target is:

```text
stdlib / host / script declarations
        ↓
DefinitionRegistry
        ↓
Compiler emits unlinked bytecode using typed DefIds
        ↓
Linker resolves DefIds to dense handles, slots, and cache-ready operands
        ↓
VM executes linked bytecode only
```

The key rule:

```text
Names are source/debug data.
DefIds are semantic identity.
Handles/slots are runtime operands.
DebugNameIds are diagnostics/reflection data.
```

This plan removes the current handwritten stdlib ID table and replaces it with a single definition system.

---

## 1. Current Architectural Problems

### 1.1 Hand-written stdlib IDs are not the root problem

`standard_ids.rs` is a symptom. It contains stable constants for stdlib functions and value methods, such as math functions, Option/Result helpers, string methods, array/map/set methods, and range methods.

The problem is that these IDs are not part of a unified definition model. They are just raw constants consumed by multiple subsystems.

Current pattern:

```text
vela_common::standard_ids.rs
    declares stdlib function/method IDs

vela_engine::standard::ids.rs
    re-exports common stdlib IDs
    also declares type/variant/field IDs

vela_engine::standard::functions.rs
    declares stdlib names, params, docs, effects, and IDs

vela_vm::math_stdlib.rs / option_result.rs / stdlib.rs
    separately registers stdlib names, IDs, and Rust implementations

CompilerOptions
    rebuilds name -> ID / type -> method maps from registry metadata

VM
    stores both name-based and ID-based native dispatch maps
```

This causes multiple sources of truth.

### 1.2 `CallNative` mixes identity, fallback, and diagnostics

The current native call operand conceptually looks like:

```rust
CallNative {
    dst,
    name: String,
    native: Option<FunctionId>,
    args,
}
```

This means a VM instruction carries both:

- a human-readable name;
- an optional semantic ID;
- a runtime fallback strategy.

That is not a clean separation. Runtime fallback by string should not exist in linked bytecode. A string name should be diagnostic metadata, not a hot-path operand.

### 1.3 VM dispatch still supports name fallback

The VM currently keeps both name and ID maps for native functions:

```rust
natives: HashMap<String, NativeFunction>
native_ids: HashMap<FunctionId, NativeFunction>
```

A clean VM should not resolve names during normal execution. Name lookup belongs in the compiler, registry, or linker. Runtime should execute dense handles or generated builtin opcodes.

### 1.4 `CompilerOptions` is a registry-shaped workaround

`CompilerOptions` currently transports many maps:

```rust
HashMap<String, FieldId>
HashMap<(String, String), HostFieldInfo>
HashMap<String, HostMethodId>
HashMap<(String, String), HostMethodId>
HashMap<HostMethodId, Vec<HostMethodParam>>
HashMap<String, NativeFunctionInfo>
...
```

This is a sign that the compiler wants a definition registry but receives a flattened temporary view instead.

### 1.5 Runtime values still use string identity in places

Option/Result and enum-like values still depend on strings such as:

```text
"Option"
"Some"
"Option::Some"
"Result::Ok"
"0"
```

A clean model should store type, variant, and field identity by typed IDs or compact slots, with names kept in side tables for diagnostics/reflection.

---

## 2. Non-Goals

This refactor deliberately does **not** preserve:

- old raw numeric stdlib IDs;
- old `standard_ids.rs`;
- old `CallNative { name, native }` fallback behavior;
- old `CompilerOptions` as the primary compiler query interface;
- old bytecode encoding;
- old `ProgramImage` serialization format;
- source compatibility for engine/VM/compiler public APIs;
- reflection raw ID compatibility;
- tests that assert legacy ID numbers.

This refactor may break examples, tests, docs, generated output, and host embedding APIs. Updating them is part of the plan.

### 2.1 Non-negotiable runtime invariants

No compatibility is preserved for old internal shapes, but the product
architecture contract is not optional. The refactor must preserve these
invariants from the first replacement implementation that executes code:

- scripts never observe or retain real Rust `&mut T` references;
- host mutation remains represented through `HostRef`, host target plans or
  materialized `HostPath`, `PathProxy`, and `HostAccess`;
- host reads, writes, mutations, removals, pushes, and calls remain
  adapter-validated, capability-checked, budgeted, and source-spanned;
- later script traps do not roll back earlier host write-through effects;
- reflection can query metadata and perform controlled reads/writes/calls, but
  it cannot mutate type structure or become monkey patching;
- execution budgets, call-depth budgets, GC roots, and host-ref exclusion from
  script GC tracing remain enforced;
- hot reload keeps active frames on their old linked code and routes new calls
  to the new `ProgramVersion` only after the appropriate safe point;
- stdlib and builtin APIs stay domain-neutral.

If a task has to choose between preserving an old API and preserving one of
these invariants, preserve the invariant and delete or redesign the old API.

---

## 3. Target Architecture

### 3.1 Crate layout

Recommended final crate structure:

```text
crates/
  vela_def/
    src/
      ids.rs              # DefId, FunctionId, MethodId, TypeId, FieldId, VariantId
      path.rs             # DefPath, DefKind, DefOwner
      debug.rs            # DebugNameId, DebugNameTable
      symbol.rs           # optional shared symbol model

  vela_registry/
    src/
      lib.rs              # DefinitionRegistry
      function.rs         # FunctionDef, FunctionSignature
      method.rs           # MethodDef, MethodReceiver
      type.rs             # TypeDef, FieldDef, VariantDef
      validation.rs       # duplicate and collision checks
      compile_view.rs     # compiler query facade

  vela_stdlib/
    src/
      manifest.rs         # single source of truth for stdlib definitions
      generated.rs        # generated or macro-expanded definition data
      register.rs         # installs stdlib definitions into DefinitionRegistry

  vela_stdlib_runtime/
    src/
      lib.rs              # maps stdlib DefIds to VM/native implementations
      install.rs          # installs stdlib runtime bindings into VM tables

  vela_bytecode/
    src/
      unlinked.rs         # bytecode using DefIds and DebugNameIds
      linked.rs           # bytecode using dense handles, slots, resolved targets
      linker.rs           # UnlinkedProgram -> LinkedProgram
      verification.rs     # verifier for linked bytecode invariants

  vela_vm/
    src/
      dispatch.rs         # dense-handle dispatch
      builtins.rs         # generated builtin dispatch enum/table
      execution.rs        # executes LinkedCodeObject / LinkedProgram only
```

This structure does not have to be introduced in one PR. The task plan below gives a safer incremental sequence.

### 3.1.1 Dependency boundaries

The refactor should enforce crate dependencies rather than rely on convention:

| Crate | May depend on | Must not depend on |
|---|---|---|
| `vela_def` | external stable-hash crate only | any Vela crate |
| `vela_registry` | `vela_def`, shared diagnostics/effects if split | `vela_vm`, `vela_engine`, compiler internals |
| `vela_stdlib` definition manifest | `vela_def`, `vela_registry` metadata types | `vela_vm` execution internals |
| `vela_stdlib_runtime` | `vela_def`, `vela_stdlib`, VM native function types | compiler or engine builders |
| `vela_bytecode` | `vela_def`; linker may query `vela_registry` | `vela_engine`, host adapters |
| `vela_vm` | linked `vela_bytecode`, `vela_def`, host/runtime contracts | parser, HIR, compiler, registry mutation APIs, `vela_stdlib_runtime` |
| `vela_engine` | compiler, registry, stdlib registration, VM, stdlib runtime installation | internal fallback registries that duplicate identity |

If these boundaries create a dependency cycle, split the data model before
continuing. Do not add a shortcut dependency that makes the cycle permanent.

### 3.2 Data flow

```text
Source / stdlib manifest / host registration
        ↓
DefinitionRegistry
        ↓
Semantic analysis and compiler queries
        ↓
UnlinkedProgram
    - FunctionId
    - MethodId
    - TypeId
    - FieldId
    - VariantId
    - DebugNameId
        ↓
Linker
        ↓
LinkedProgram
    - NativeHandle
    - ScriptFunctionHandle
    - MethodDispatchHandle
    - TypeHandle
    - FieldSlot
    - GlobalSlot
    - HostTargetPlanId
        ↓
VM execution
```

### 3.3 Layer responsibilities

| Layer | Owns | Must not own |
|---|---|---|
| Parser / syntax | source spelling | stable identity |
| HIR / semantic | resolved names and type facts | runtime handles |
| DefinitionRegistry | semantic identity and metadata | VM execution tables |
| Compiler | unlinked bytecode with typed IDs | final handle layout |
| Linker | dense handles, slots, executable layout | source lookup |
| VM | linked bytecode execution | string fallback resolution |
| Reflection | metadata and debug names | hot dispatch operands |

### 3.4 `ProgramVersion` ownership

Executable state must be version-owned. A `ProgramVersion` owns:

- linked bytecode and linked code objects;
- runtime handle tables and linked layouts;
- debug name tables and source-span/call-site metadata;
- inline cache state and cache invalidation metadata;
- bytecode-offset profile layout and future counter storage;
- hot-reload ABI/schema manifests derived from the registry.

Hot reload builds a new `ProgramVersion` with fresh linked layouts. Rejected
reloads keep the previous version unchanged. Active frames retain references to
their old version and cannot observe partial updates. New calls enter the new
version only after a safe point accepts it.

No linked handle, slot, cache entry, or debug-name table index may outlive the
`ProgramVersion` that owns it.

---

## 4. Identity Model

### 4.1 `DefKind`

Create one central definition kind enum:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum DefKind {
    Function,
    Method,
    Type,
    Field,
    Variant,
    Trait,
    Module,
    Global,
}
```

### 4.2 `DefPath`

A definition path is the canonical semantic path used to derive stable identity.

```rust
#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct DefPath {
    pub package: String,       // "std", "host", "script", or crate/module package
    pub module: Vec<String>,   // ["math"], ["option"], etc.
    pub owner: Option<String>, // "String", "Array", "Option::Some", etc.
    pub name: String,          // "max", "len", "Some", "0", etc.
    pub kind: DefKind,
}
```

Examples:

```text
function std::math::max
method   std::String::len
type     std::String
variant  std::Option::Some
field    std::Option::Some::0
```

### 4.3 `DefId`

Use a deterministic hash of canonical `DefPath`. The final identity
representation is fixed by this refactor:

```rust
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct DefId(u128);
```

Typed wrappers:

```rust
#[repr(transparent)]
pub struct FunctionId(DefId);

#[repr(transparent)]
pub struct MethodId(DefId);

#[repr(transparent)]
pub struct TypeId(DefId);

#[repr(transparent)]
pub struct FieldId(DefId);

#[repr(transparent)]
pub struct VariantId(DefId);
```

Do not use bare `u64` in public identity APIs.

### 4.4 Hashing rules

Canonical input format:

```text
vela-def-v1\0
kind=<kind>\0
package=<package>\0
module=<module.path>\0
owner=<owner-or-empty>\0
name=<name>\0
```

Rules:

- case-sensitive;
- UTF-8;
- no implicit alias normalization;
- module separators canonicalized to `::`;
- owner path canonicalized before hashing;
- hash algorithm is BLAKE3 truncated to 128 bits;
- the BLAKE3 input format and version prefix are part of the artifact ABI for
  this clean architecture generation.

SipHash is not appropriate for stable cross-run IDs. FNV is too weak for this
long-term identity role. Do not add temporary `u64` IDs or preserve old raw ID
spaces during the migration.

### 4.5 Collision policy

Even with `u128`, the registry must validate collisions:

```rust
if same DefId but different DefPath:
    panic or return RegistryError::DefIdCollision
```

Because this is a clean breaking refactor, collision handling can be strict.

### 4.6 `Symbol` is not identity

`Symbol` / `SymbolInterner` may still exist, but it is only for memory/performance within one compilation session.

Do not use `Symbol` for ABI identity.

```text
Symbol       = local interned string index
DefId        = stable semantic identity
Handle/Slot  = linked runtime operand
DebugNameId  = diagnostic/reflection side-table index
```

---

## 5. Definition Registry

### 5.1 Purpose

`DefinitionRegistry` is the single source of truth for:

- stdlib functions;
- stdlib value methods;
- stdlib types;
- host functions;
- host types;
- host fields;
- host methods;
- script functions and script methods after semantic registration;
- reflection metadata;
- compiler lookup.

It replaces the current `CompilerOptions` string-map layer as the compiler’s primary query API.

### 5.2 Core API

Sketch:

```rust
pub struct DefinitionRegistry {
    defs_by_id: HashMap<DefId, Def>,
    ids_by_path: HashMap<DefPath, DefId>,
    functions_by_path: HashMap<DefPath, FunctionId>,
    methods_by_receiver: HashMap<(TypeId, String), MethodId>,
    fields_by_owner: HashMap<(TypeId, String), FieldId>,
    variants_by_owner: HashMap<(TypeId, String), VariantId>,
    debug_names: DebugNameTable,
}

pub enum Def {
    Function(FunctionDef),
    Method(MethodDef),
    Type(TypeDef),
    Field(FieldDef),
    Variant(VariantDef),
    Trait(TraitDef),
    Module(ModuleDef),
    Global(GlobalDef),
}
```

### 5.3 Query API

The compiler should ask typed questions:

```rust
registry.resolve_function_path(path: &ResolvedPath) -> Option<FunctionId>;

registry.resolve_method(
    receiver_type: TypeId,
    method_name: &str,
) -> Option<MethodId>;

registry.resolve_field(
    owner_type: TypeId,
    field_name: &str,
) -> Option<FieldId>;

registry.function_signature(id: FunctionId) -> &FunctionSignature;
registry.method_signature(id: MethodId) -> &MethodSignature;
registry.type_def(id: TypeId) -> &TypeDef;
```

Avoid:

```rust
HashMap<String, FunctionId>
HashMap<(String, String), MethodId>
```

Names can appear as query inputs at semantic boundaries, but successful results should become typed IDs.

### 5.4 Validation

Registry validation must check:

- duplicate `DefPath`;
- duplicate `DefId` with different path;
- duplicate function path;
- duplicate method for same receiver type and method name;
- duplicate field for same owner type and field name;
- duplicate variant for same enum type and variant name;
- duplicate runtime builtin mapping;
- missing implementation for VM-callable native/builtin function;
- missing reflection metadata for public definitions;
- missing debug name for registered definitions.

### 5.5 Compile view

If the compiler needs a compact view, expose an explicit `RegistryCompileView`:

```rust
pub struct RegistryCompileView<'a> {
    registry: &'a DefinitionRegistry,
}

impl RegistryCompileView<'_> {
    pub fn native_function(&self, path: &ResolvedPath) -> Option<FunctionCompileInfo>;
    pub fn value_method(&self, receiver: TypeId, name: &str) -> Option<MethodCompileInfo>;
    pub fn host_field(&self, receiver: TypeId, name: &str) -> Option<FieldCompileInfo>;
}
```

This may internally cache maps, but the source of truth remains the registry.

---

## 6. Stdlib Manifest

### 6.1 Goal

Replace handwritten stdlib ID files with one manifest.

The manifest must define:

- stdlib functions;
- stdlib value methods;
- stdlib types;
- stdlib variants;
- stdlib fields;
- docs;
- effects;
- access flags;
- parameter metadata;
- return type metadata;
- VM implementation target.

### 6.2 Suggested macro shape

Example:

```rust
vela_stdlib! {
    fn std::math::max(left: Any, right: Any) -> Any {
        rust = vela_vm::math_stdlib::scalar::math_max;
        docs = "Returns the larger numeric value.";
        effects = pure;
        access = public_reflect_callable;
    }

    fn std::option::some(value: Any) -> Any {
        rust = vela_vm::option_result::option_some;
        docs = "Wraps a value in Option::Some.";
        effects = pure;
        access = public_reflect_callable;
    }

    type std::String {
        method len(self) -> Int {
            rust = vela_vm::string_methods::len;
            docs = "Returns the number of characters.";
            effects = pure;
        }

        method is_empty(self) -> Bool {
            rust = vela_vm::string_methods::is_empty;
            effects = pure;
        }
    }

    enum std::Option {
        variant Some {
            field 0: Any;
        }

        variant None;
    }
}
```

### 6.3 Generated output

The macro/build step should generate:

```rust
pub enum BuiltinFunction {
    MathMax,
    MathMin,
    MathClamp,
    OptionSome,
    OptionNone,
    ResultOk,
    ResultErr,
    SetFromArray,
    ...
}

impl BuiltinFunction {
    pub const fn function_id(self) -> FunctionId;
    pub const fn debug_name(self) -> &'static str;
}
```

Also generate:

- `register_stdlib_defs(registry: &mut DefinitionRegistry)`;
- reflection descriptors;
- docs metadata;
- parameter metadata;
- compile signatures;
- duplicate validation tests.

Runtime implementation bindings are a separate generated or declarative table
keyed by `FunctionId`/`MethodId`:

```rust
pub fn stdlib_runtime_bindings() -> &'static [StdlibRuntimeBinding];
```

The semantic manifest is the source of identity and metadata. Runtime bindings
map those identities to VM/native implementation functions. Keeping these
in `vela_stdlib_runtime`, outside the semantic manifest crate, prevents
dependency cycles and keeps `DefinitionRegistry` from owning VM function
pointers. `vela_vm` must not depend on `vela_stdlib_runtime`; the engine or
runtime builder installs those bindings into VM runtime tables.

### 6.4 No raw numeric constants

Do not expose:

```rust
pub const MATH_MAX_FUNCTION_ID: FunctionId = FunctionId::new(0xff00_0100);
```

Instead expose either:

```rust
BuiltinFunction::MathMax.function_id()
```

or generated typed helper:

```rust
stdlib::ids::function::MATH_MAX
```

where the value is derived from the manifest path, not handwritten.

### 6.5 Deleting current ID files

Delete:

```text
crates/vela_common/src/standard_ids.rs
crates/vela_engine/src/standard/ids.rs
```

Replace imports with manifest-generated APIs.

---

## 7. Bytecode Model

### 7.1 Split unlinked and linked bytecode

Introduce two bytecode layers.

#### Unlinked bytecode

Produced by the compiler.

Uses semantic IDs:

```rust
pub enum UnlinkedInstructionKind {
    CallFunction {
        dst: Register,
        target: FunctionId,
        debug_name: DebugNameId,
        args: Vec<CallArgument>,
    },

    CallMethod {
        dst: Register,
        receiver: Register,
        target: MethodId,
        debug_name: DebugNameId,
        args: Vec<CallArgument>,
    },

    GetField {
        dst: Register,
        base: Register,
        field: FieldId,
        debug_name: DebugNameId,
    },
}
```

#### Linked bytecode

Executed by the VM.

Uses dense handles and slots:

```rust
pub enum InstructionKind {
    CallNative {
        dst: Option<Register>,
        target: NativeHandle,
        args: Vec<Register>,
    },

    CallScript {
        dst: Register,
        target: ScriptFunctionHandle,
        args: Vec<CallArgument>,
    },

    CallMethod {
        dst: Register,
        receiver: Register,
        target: MethodDispatchHandle,
        args: Vec<CallArgument>,
    },

    GetRecordSlot {
        dst: Register,
        record: Register,
        slot: FieldSlot,
    },
}
```

### 7.2 Remove string fallback from executable bytecode

No linked instruction should contain a dispatch fallback name.

Allowed:

```rust
debug_name: DebugNameId
```

Not allowed:

```rust
name: String
native: Option<FunctionId>
```

If a name is needed for an error, the VM asks the linked program’s debug table.

### 7.3 Linker

Add:

```rust
pub struct Linker<'a> {
    registry: &'a DefinitionRegistry,
    runtime_layout: &'a RuntimeLayout,
}

impl Linker<'_> {
    pub fn link_program(&self, program: UnlinkedProgram) -> LinkResult<LinkedProgram>;
}
```

The linker resolves:

```text
FunctionId -> NativeHandle or ScriptFunctionHandle
MethodId   -> MethodDispatchHandle
FieldId    -> FieldSlot / HostTargetPlan
TypeId     -> TypeHandle / layout metadata
GlobalId   -> GlobalSlot
```

### 7.4 Link errors

Use explicit errors:

```rust
pub enum LinkErrorKind {
    UnknownFunction { id: FunctionId },
    UnknownMethod { id: MethodId },
    UnknownField { id: FieldId },
    MissingNativeImplementation { id: FunctionId },
    MissingScriptFunction { id: FunctionId },
    MethodReceiverMismatch { method: MethodId },
    UnsupportedDynamicTarget { id: DefId },
}
```

No runtime fallback should hide linker failures.

### 7.5 Verifier

The linked verifier must validate:

- register bounds;
- constant bounds;
- jump bounds;
- native handle bounds;
- script function handle bounds;
- method dispatch handle bounds;
- field slot bounds;
- host target plan bounds;
- cache-site kind matches instruction kind;
- contiguous dynamic args;
- debug name references are valid;
- no unresolved DefIds remain in linked bytecode.

---

## 8. VM Runtime Model

### 8.1 Runtime tables

Replace mixed name/ID maps with dense tables:

```rust
pub struct RuntimeTables {
    native_functions: Vec<NativeFunction>,
    host_native_functions: Vec<HostNativeFunction>,
    method_dispatch: Vec<MethodDispatchEntry>,
    builtin_functions: Vec<BuiltinFunctionEntry>,
}
```

Handles:

```rust
#[repr(transparent)]
pub struct NativeHandle(u32);

#[repr(transparent)]
pub struct HostNativeHandle(u32);

#[repr(transparent)]
pub struct MethodDispatchHandle(u32);
```

### 8.2 Native dispatch

Current conceptual behavior:

```text
try native_ids[FunctionId]
else natives[name]
```

Target behavior:

```text
native_functions[NativeHandle]
```

No hash lookup. No name fallback.

### 8.3 Builtin dispatch

For stdlib, choose one of two clean approaches.

#### Option A: generated enum dispatch

```rust
pub enum BuiltinFunction {
    MathMax,
    MathMin,
    OptionSome,
    ...
}

pub fn dispatch_builtin(
    builtin: BuiltinFunction,
    args: &[Value],
    runtime: &mut RuntimeContext,
) -> VmResult<Value>;
```

Pros:

- no table lookup for stdlib;
- compile-time exhaustiveness;
- clear builtin inventory.

Cons:

- generated dispatch can get large.

#### Option B: dense native table

Stdlib functions are installed into `RuntimeTables` like host natives.

Pros:

- simpler uniform dispatch;
- easier dynamic host/native integration.

Cons:

- less explicit builtin specialization.

Recommended initial target: **Option B** for fewer moving parts, then optionally specialize hot stdlib functions with generated enum dispatch.

### 8.4 Debug names

Linked VM instructions may carry `DebugNameId` or an instruction debug side table:

```rust
pub struct LinkedProgramDebug {
    names: DebugNameTable,
    call_sites: Vec<CallSiteDebugInfo>,
}
```

VM errors should use:

```rust
program.debug_name(debug_name_id)
```

not instruction-embedded `String`.

### 8.5 Runtime value identity

Change enum/record heap values from string identity to typed identity.

Current style:

```rust
HeapValue::Enum {
    enum_name: String,
    variant: String,
    fields,
}
```

Target style:

```rust
HeapValue::Enum {
    type_id: TypeId,
    variant_id: VariantId,
    fields: ScriptFields<Value>,
}
```

For script records:

```rust
HeapValue::Record {
    type_id: TypeId,
    shape: ShapeId,
    fields: ScriptFields<Value>,
}
```

Names are resolved through registry/debug metadata.

---

## 9. Host Boundary Model

### 9.1 Host definitions are registry definitions

Host types, fields, and methods should register into `DefinitionRegistry` as normal definitions:

```rust
host type Player
host field Player.health
host method Player.damage
host method Player.inventory.push
```

Each gets:

```rust
TypeId
FieldId
MethodId
```

### 9.2 Host access linking

The compiler should emit unlinked host access with semantic IDs or typed plans.

The linker should lower them into:

```rust
HostTargetPlanId
ResolvedHostAccess handle shape
cache-site metadata
```

The VM should not resolve host field names on the hot path.

### 9.3 Host schema changes

Since compatibility is not a goal for this refactor, old host schema IDs do not need to be preserved.

However, after the clean architecture is in place, schema invalidation should use:

```text
schema epoch
TypeId
FieldId
MethodId
HostTargetPlanId
cache generation
```

not names.

---

## 10. Reflection Model

Reflection should expose definitions from the registry.

It can show:

```text
path
kind
docs
params
return type
effects
permissions
debug name
stable ID display
```

But reflection should not be the source of identity.

Do not let reflection-only descriptors become the compiler’s input format. The registry is primary; reflection is a view.

---

## 11. Refactor Plan for Codex

This section is written as executable task groups. Each group can become one or more Codex tasks.
Each task title is a Markdown checkbox. Codex should change `[ ]` to `[x]`
only after the task acceptance criteria and validation pass.
Transitional APIs may exist only inside the task sequence that removes them.
They must not become milestone architecture or be listed as accepted final
surface.

---

### Phase 0: Baseline and branch setup

- [x] **Task 0.1: Create breaking-change branch and architecture notes**

**Objective:** Make it explicit that this branch does not preserve compatibility.

**Changes:**

- Add `docs/architecture/clean-identity-refactor.md` with this plan or a condensed version.
- Add a top-level note in `docs/progress.md` that the branch is a breaking clean architecture track.
- Disable or mark legacy ID compatibility tests if any exist.

**Acceptance criteria:**

- The repository documents that old stdlib IDs and bytecode are not preserved.
- No code changes yet.

- [x] **Task 0.2: Inventory and delete-plan current identity surfaces**

**Objective:** Make every legacy identity and dispatch surface visible before demolition.

**Changes:**

- Inventory all uses of:
  - `standard_ids`;
  - raw `0xff00_...` stdlib IDs;
  - `CallNative` name or optional-ID operands;
  - `CallMethod` name or optional value-method operands;
  - `CompilerOptions` identity maps;
  - native and host-native name maps;
  - Option/Result string type, variant, and field identity;
  - `ProgramImage` serialization assumptions;
  - C API, WASM playground, example, and docs entry points that compile or run code.
- Record which phase deletes or replaces each surface.
- Add grep commands for each surface to the cleanup checks if missing.

**Acceptance criteria:**

- The plan names every known legacy identity surface and its removal phase.
- Later tasks can delete old paths without rediscovering hidden dependencies.

**Inventory and delete plan:**

| Legacy surface | Current owner examples | Replacement/delete phase |
|---|---|---|
| `standard_ids` module and re-exports | `crates/vela_common/src/standard_ids.rs`, `crates/vela_common/src/lib.rs`, `crates/vela_engine/src/standard/ids.rs` | Phase 1 moves typed IDs to `vela_def`; Phase 3 replaces stdlib identity with manifest data; Phase 3.4 deletes `standard_ids.rs`; Phase 9.1 removes public old ID APIs. |
| Raw `0xff00_...` stdlib and builtin IDs | `vela_common::standard_ids`, `vela_engine/src/standard/ids.rs`, `clock.rs`, `random.rs`, `io.rs`, `context_schema.rs`, standard method modules, standard type schema hashes | Phase 1 introduces BLAKE3-128 `DefPath` IDs; Phase 3 stdlib manifest owns stdlib IDs; Phase 8 registers host/context surfaces through registry; Phase 9.1 removes handwritten raw constants. |
| `CallNative` mixed diagnostic name and optional semantic ID | `crates/vela_bytecode/src/lib.rs`, `compiler/calls.rs`, `verification.rs`, `crates/vela_vm/src/execution.rs`, `native_function_calls.rs`, standard ID dispatch tests | Phase 5 introduces unlinked vs linked instructions; Phase 5.2/5.3 link native calls to dense handles; Phase 6.1 executes linked code only; Phase 6.2 removes native name fallback; Phase 9.2 deletes name/optional-ID operands. |
| `CallMethod` mixed method name and optional value-method ID | `crates/vela_bytecode/src/lib.rs`, `compiler/calls.rs`, `compiler/methods.rs`, `crates/vela_vm/src/execution.rs`, `script_method_calls.rs`, `script_methods.rs`, engine compiler-options tests | Phase 4 replaces compiler lookup with registry queries; Phase 5 links method targets; Phase 6.3 updates method dispatch; Phase 9.2 deletes fallback method-name and optional-ID operand patterns. |
| `CompilerOptions` identity maps | `crates/vela_bytecode/src/compiler/options.rs`, `crates/vela_engine/src/compiler_options.rs`, compiler/VM/engine tests and benches that construct options directly | Phase 2 adds `RegistryCompileView`; Phase 4 makes compiler entry points consume registry data; Phase 4.4 reduces `CompilerOptions` to real settings; Phase 9.3 removes identity maps. |
| VM native and host-native name maps | `crates/vela_vm/src/lib.rs` `natives`, `native_ids`, `host_natives`, `host_native_ids`; `crates/vela_engine/src/engine.rs` install paths and alias fallback | Phase 3.3 separates stdlib runtime bindings; Phase 5 linker produces handles; Phase 6.2 removes name fallback; Phase 9.4 deletes name maps from VM dispatch. |
| Engine/native lookup by name | `Engine::host_native_function_by_name`, `Engine::context_host_native_function_by_name`, validation and reflection/native tests | Phase 2 registry debug names preserve reflection/source lookup; Phase 4 registry compile view replaces compiler identity lookup; Phase 6/9 keep any name lookup as reflection/debug only, not dispatch. |
| Option/Result string enum identity | `crates/vela_vm/src/try_propagation.rs`, `heap_values.rs`, `owned_value.rs`, `option_result.rs`, option/result method modules, serde/runtime value conversion | Phase 3 manifest declares Option/Result type, variants, and fields; Phase 7.1 converts runtime Option/Result identity to `TypeId`/`VariantId`; Phase 9 cleanup removes string-based runtime identity. |
| Script enum/record string identity | `crates/vela_bytecode/src/compiler/constructors.rs`, `patterns.rs`, `schema_defaults.rs`, `crates/vela_vm/src/script_object_construction.rs`, `record_fields.rs`, reflection value access | Phase 2 registry/debug names separate identity from names; Phase 7.2 converts enum values to typed identity; Phase 7.3 converts records to `TypeId`/`ShapeId`/field slots. |
| `ProgramImage` old compatibility assumptions | `crates/vela_bytecode/src/program_image.rs`, `ProgramImage::to_program`, `ProgramVersion::to_program`, hot-reload tests that execute `to_program()` output, runtime-image plan compatibility notes | Phase 5 defines unlinked/linked program artifacts; Phase 6 makes VM entry accept linked code; Phase 6.1 makes `ProgramVersion` own linked layouts; Phase 9 removes old bytecode/image compatibility paths that are not diagnostics/tests. |
| C API compile/run entry points | `crates/vela_c_api/src/lib.rs` `vela_runtime_compile_source`, `Runtime::new(engine, program)`, `runtime.call` | Phase 6 updates runtime construction/execution to linked code; Phase 9 updates external entry points to the final artifact/runtime shape without old bytecode compatibility. |
| WASM playground entry points | `crates/vela_playground_wasm/src/lib.rs`, site quickstart/runtime docs | Phase 6 updates compile/run path after linked execution becomes canonical; Phase 9 updates playground/docs to stop relying on old `Program`/name-dispatch assumptions. |
| Examples, CLI, docs, benches, and tests that compile or run code | `examples/`, `crates/vela_cli/src/main.rs`, `crates/vela_vm/benches`, `crates/vela_engine` and `crates/vela_vm` tests, `site/docs`, architecture docs | Update continuously with each implementation phase; Phase 9 final cleanup removes tests/docs asserting old API shapes and keeps only new-architecture coverage. |

---

### Phase 1: Introduce `vela_def`

- [x] **Task 1.1: Add `vela_def` crate**

**Objective:** Centralize typed definition identities.

**Changes:**

- Add `crates/vela_def`.
- Add it to workspace members.
- Define:
  - `DefKind`;
  - `DefPath`;
  - `DefId`;
  - typed ID wrappers:
    - `FunctionId`;
    - `MethodId`;
    - `TypeId`;
    - `FieldId`;
    - `VariantId`;
    - `TraitId`;
    - `GlobalId` if needed.
- Implement stable deterministic ID generation from `DefPath`.

**Acceptance criteria:**

- `cargo test -p vela_def` passes.
- Unit tests prove:
  - same path generates same ID;
  - different kinds generate different IDs;
  - different owners generate different IDs;
  - BLAKE3-128 output is stable for committed fixture paths;
  - canonical path formatting is stable.

- [x] **Task 1.2: Move ID wrapper types out of `vela_common`**

**Objective:** Stop treating definition IDs as generic common utilities.

**Changes:**

- Move or re-export typed IDs from `vela_def`.
- Update imports across the workspace.
- Keep non-definition IDs in current crates:
  - `Register`;
  - `ConstantId`;
  - `InstructionOffset`;
  - `GlobalSlot`;
  - `CacheSiteId`;
  - `HostTargetPlanId`.

**Acceptance criteria:**

- `FunctionId`, `MethodId`, `TypeId`, `FieldId`, `VariantId` come from `vela_def`.
- `vela_common` no longer owns semantic definition identity.
- Old raw numeric stdlib ID spaces are not preserved or aliased.
- Workspace compiles after mechanical import updates.

---

### Phase 2: Introduce `DefinitionRegistry`

- [x] **Task 2.1: Add `vela_registry` crate**

**Objective:** Create the central definition table.

**Changes:**

- Add `crates/vela_registry`.
- Define:
  - `DefinitionRegistry`;
  - `Def`;
  - `FunctionDef`;
  - `MethodDef`;
  - `TypeDef`;
  - `FieldDef`;
  - `VariantDef`;
  - `FunctionSignature`;
  - `ParamDef`;
  - `EffectSet` or import existing effect model.
- Add registration APIs:
  - `register_function`;
  - `register_type`;
  - `register_method`;
  - `register_field`;
  - `register_variant`.

**Acceptance criteria:**

- Duplicate path is rejected.
- Duplicate semantic key is rejected.
- ID collision with different path is rejected.
- Registry lookup by path and ID works.

- [x] **Task 2.2: Add `DebugNameTable`**

**Objective:** Decouple names from hot operands.

**Changes:**

- Add `DebugNameId`.
- Add `DebugNameTable`.
- Registry should assign debug names for definitions.
- Add APIs:
  - `debug_name(id: DebugNameId) -> &str`;
  - `debug_name_for_def(id: DefId) -> DebugNameId`.

**Acceptance criteria:**

- Definitions can be printed/debugged without embedding strings in instructions.
- Debug name IDs are stable inside a registry instance.

- [x] **Task 2.3: Add `RegistryCompileView`**

**Objective:** Prepare to replace `CompilerOptions`.

**Changes:**

- Add typed query API for compiler:
  - resolve native function path;
  - resolve value method;
  - resolve host method;
  - resolve host field;
  - resolve type;
  - get function/method params.
- Internally it may use maps, but public compiler-facing API must return typed IDs.

**Acceptance criteria:**

- Unit tests cover function, method, field, and type lookup.
- No compiler migration yet.

---

### Phase 3: Replace stdlib ID constants with stdlib manifest

- [x] **Task 3.1: Add `vela_stdlib` crate or module**

**Objective:** Create the single source of truth for stdlib definitions.

**Changes:**

- Add `crates/vela_stdlib` for semantic stdlib definitions.
- Add `crates/vela_stdlib_runtime` for VM implementation bindings.
- Define stdlib functions, methods, types, variants, fields in one manifest.
- Generate/register definitions into `DefinitionRegistry`.
- Define stdlib runtime bindings separately from semantic definitions.

**Acceptance criteria:**

- All current stdlib native functions are declared in the manifest.
- All current stdlib value methods are declared in the manifest.
- Option/Result types, variants, and fields are declared in the manifest.
- Runtime implementation bindings are keyed by manifest-derived IDs.
- Registry metadata and `vela_stdlib` do not depend on VM function pointers.
- Registry validation catches duplicate stdlib names.

- [x] **Task 3.2: Generate stdlib function metadata from manifest**

**Objective:** Delete duplicated stdlib metadata definitions.

**Changes:**

- Replace current hand-authored standard function descriptor construction with generated descriptors from manifest.
- Ensure docs, params, returns, effects, and access are sourced from the manifest.

**Acceptance criteria:**

- `standard_native_function_descs()` or its replacement is generated from manifest data.
- No separate list of math/option/result/set descriptors exists outside the manifest.

- [x] **Task 3.3: Generate VM stdlib registration from manifest**

**Objective:** Delete duplicated stdlib VM registration lists.

**Changes:**

- Generate stdlib VM registration table:
  - function ID;
  - debug name;
  - Rust implementation target.
- Replace manual calls such as:
  - `register_native_with_id(MATH_MAX_FUNCTION_ID, "math::max", math_max)`;
  - `register_native_with_id(OPTION_SOME_FUNCTION_ID, "option::some", option_some)`.

**Acceptance criteria:**

- VM stdlib registration consumes manifest-generated data.
- The same manifest entry drives registry metadata and runtime binding lookup.
- No semantic registry type stores VM function pointers.

- [x] **Task 3.4: Delete `standard_ids.rs`**

**Objective:** Remove handwritten stdlib ID tables.

**Changes:**

- Delete `crates/vela_common/src/standard_ids.rs`.
- Delete or empty `crates/vela_engine/src/standard/ids.rs`.
- Replace all imports of old constants with manifest-generated typed accessors.

**Acceptance criteria:**

- No `0xff00_...` stdlib ID constants remain.
- No compile errors from missing old ID constants.
- Tests do not assert old raw ID values.

---

### Phase 4: Migrate compiler from `CompilerOptions` to registry queries

- [x] **Task 4.1: Add registry to compiler entry points**

**Objective:** Make the compiler consume `RegistryCompileView`.

**Changes:**

- Add compiler entry points that accept registry/compile view.
- Keep temporary old entry points only if needed during migration, but do not preserve as final API.
- Replace native function lookup from `CompilerOptions::native_function_id` with registry lookup.

**Acceptance criteria:**

- Native calls compile to `FunctionId` from registry.
- Missing native functions become compile errors or unresolved external errors, not silent string fallback.

- [x] **Task 4.2: Replace value method lookup**

**Objective:** Compile value methods to `MethodId`.

**Changes:**

- Replace `value_method_id_for_type(type_name, method)` with registry method resolution.
- Method params come from `MethodDef`.
- Receiver type should resolve to `TypeId`, not string.

**Acceptance criteria:**

- String/array/map/set/range/Option/Result methods compile to typed `MethodId`.
- Named argument handling uses registry param metadata.

- [x] **Task 4.3: Replace host type/field/method lookup**

**Objective:** Compile host access via typed registry definitions.

**Changes:**

- Host types register into registry.
- Host fields and methods register into registry.
- Compiler resolves host field/method by `TypeId` + member name.
- Compiler emits typed IDs or typed host target plans.

**Acceptance criteria:**

- Host field/method lowering no longer depends on raw `(String, String)` maps as the primary API.
- Host target planning still passes current tests after test updates.

- [x] **Task 4.4: Remove `CompilerOptions` or reduce it to non-identity options**

**Objective:** Stop using `CompilerOptions` as an identity registry.

**Changes:**

- Delete identity maps from `CompilerOptions`.
- Keep only true compiler options:
  - feature flags;
  - optimization settings;
  - diagnostics settings;
  - capability mode if compile-time relevant.
- Rename if useful:
  - `CompilerSettings`;
  - `CompileOptions`.

**Acceptance criteria:**

- `CompilerOptions` no longer stores native function IDs, host method IDs, type IDs, field IDs, or value method params.
- Compiler obtains identity from `DefinitionRegistry`.

---

### Phase 5: Split unlinked and linked bytecode

- [x] **Task 5.1: Introduce `UnlinkedProgram` and `UnlinkedInstructionKind`**

**Objective:** Let compiler output semantic-ID bytecode.

**Changes:**

- Move current `Program`/`CodeObject` toward unlinked or linked forms.
- Define:
  - `UnlinkedProgram`;
  - `UnlinkedCodeObject`;
  - `UnlinkedInstructionKind`.
- Native, script, method, field, and host operands use typed IDs.

**Acceptance criteria:**

- Compiler outputs `UnlinkedProgram` / `UnlinkedCodeObject`.
- No runtime handles are required during compilation.

- [ ] **Task 5.2: Introduce `LinkedProgram` and runtime handles**

**Objective:** Add the executable bytecode representation.

**Changes:**

- Define:
  - `LinkedProgram`;
  - `LinkedCodeObject`;
  - `InstructionKind` for linked execution.
- Add handle types:
  - `NativeHandle`;
  - `ScriptFunctionHandle`;
  - `MethodDispatchHandle`;
  - `TypeHandle`;
  - `FieldSlot`.

**Acceptance criteria:**

- Linked instructions contain handles/slots, not strings or unresolved IDs.
- Debug names live in a side table.

- [ ] **Task 5.3: Implement linker**

**Objective:** Convert unlinked bytecode to linked bytecode.

**Changes:**

- Add `vela_bytecode::linker`.
- Link:
  - native functions;
  - script functions;
  - script methods;
  - stdlib value methods;
  - host fields/methods;
  - record/enum fields;
  - globals.
- Produce explicit `LinkError` on missing definitions.

**Acceptance criteria:**

- Linker fails on unresolved native calls.
- Linker fails on missing native implementation.
- Linker maps functions/methods to dense handles.
- VM no longer needs to know names for dispatch.

- [ ] **Task 5.4: Update verifier for linked bytecode**

**Objective:** Verify executable invariants.

**Changes:**

- Add handle bounds validation.
- Ensure no unresolved IDs exist in linked bytecode.
- Ensure debug IDs are valid.
- Keep existing register/constant/jump checks.

**Acceptance criteria:**

- Invalid handle index is rejected before execution.
- Invalid debug name reference is rejected.
- Existing bytecode verification tests are ported.

---

### Phase 6: Move VM to linked-only execution

- [ ] **Task 6.1: Change VM entry points to accept linked code**

**Objective:** Make the VM execute only linked bytecode.

**Changes:**

- Change `Vm::run` and program execution APIs to use `LinkedCodeObject` / `LinkedProgram`.
- Engine/runtime should compile and link before VM execution.
- Tests that directly compile and run should call link step.
- `ProgramVersion` should own linked bytecode, debug tables, runtime handles,
  profile layout, and cache state.

**Acceptance criteria:**

- VM execution path receives no unlinked bytecode.
- VM cannot accidentally execute unresolved calls.
- Active frames retain old linked code through old `ProgramVersion` ownership.
- New calls enter newly linked code only after hot-reload safe-point acceptance.

- [ ] **Task 6.2: Remove native name fallback**

**Objective:** Delete name lookup from runtime dispatch.

**Changes:**

- Remove:
  - `natives: HashMap<String, NativeFunction>`;
  - `host_natives: HashMap<String, HostNativeFunction>`;
  - name fallback resolution.
- Replace with:
  - `Vec<NativeFunction>`;
  - `Vec<HostNativeFunction>`;
  - handle lookup.

**Acceptance criteria:**

- Native dispatch does not use `HashMap<String, ...>`.
- No instruction dispatch uses `name` as fallback.
- Unknown native is a link error, not a runtime fallback error.

- [ ] **Task 6.3: Update method dispatch**

**Objective:** Method calls use linked method handles.

**Changes:**

- Replace method name dispatch in hot path with `MethodDispatchHandle`.
- Use debug names only for error reporting.
- Preserve semantic dispatch behavior, but not legacy implementation shape.

**Acceptance criteria:**

- Value method hot path does not hash/compare method strings.
- Script method dispatch uses handles/IDs.
- Fallback-by-name is removed from linked execution.

- [ ] **Task 6.4: Update script function calls**

**Objective:** Script calls use function handles.

**Changes:**

- Replace `Program.functions: BTreeMap<String, CodeObject>` with indexed function table.
- Store debug names separately.
- Link `FunctionId` to `ScriptFunctionHandle`.

**Acceptance criteria:**

- Script function call does not look up by string during execution.
- Missing function is a link error.
- Hot reload/versioning can rebuild handle tables.

---

### Phase 7: Replace runtime string identity for records/enums

- [ ] **Task 7.1: Convert Option/Result runtime identity to TypeId/VariantId**

**Objective:** Remove hardcoded string identity from built-in enum values.

**Changes:**

- Replace `"Option"`, `"Some"`, `"None"`, `"Result"`, `"Ok"`, `"Err"` runtime identity with:
  - `OPTION_TYPE_ID`;
  - `OPTION_SOME_VARIANT_ID`;
  - etc., generated from manifest.
- Field `"0"` becomes `FieldId` or `FieldSlot`.

**Acceptance criteria:**

- Option/Result operations compare typed IDs, not strings.
- Error messages still print readable names through debug metadata.

- [ ] **Task 7.2: Convert script enum values to typed identity**

**Objective:** Use the same identity model for script enums.

**Changes:**

- Semantic phase registers script enum types and variants in registry.
- Compiler emits typed variant/field IDs.
- VM stores enum values by IDs.

**Acceptance criteria:**

- Enum tag comparisons use `VariantId`.
- Enum field access uses `FieldId`/slot.
- Reflection still shows source names.

- [ ] **Task 7.3: Convert records to TypeId/ShapeId/FieldSlot**

**Objective:** Move record field hot paths to slots.

**Changes:**

- Record construction uses type/shape metadata.
- Field reads/writes use linked slots where possible.
- Dynamic fallback should be explicit and separated from static slot path.

**Acceptance criteria:**

- Static record field access does not use string field lookup.
- Dynamic access, if supported, is represented as dynamic access, not hidden fallback.

---

### Phase 8: Clean host boundary and registry integration

- [ ] **Task 8.1: Register host types directly into registry**

**Objective:** Stop building compiler identity maps from reflection descriptors.

**Changes:**

- Host registration produces `TypeDef`, `FieldDef`, and `MethodDef`.
- Reflection descriptors become views over registry definitions or are generated from them.
- Engine builder stores registry definitions as primary data.

**Acceptance criteria:**

- Host method/field IDs come from `DefPath` through registry.
- Engine no longer reconstructs compiler identity maps from reflection-only descriptors.

- [ ] **Task 8.2: Link host target plans**

**Objective:** Ensure host access instructions are linked to cache-ready targets.

**Changes:**

- Unlinked host access refers to typed host field/method IDs.
- Linker produces `HostTargetPlanId` and related runtime metadata.
- VM uses plan IDs / resolved access handles.

**Acceptance criteria:**

- Host field/path hot path uses linked target plan.
- VM does not resolve host member names in normal execution.

---

### Phase 9: Delete legacy layers

Start Phase 9 only after registry compiler queries, stdlib manifest/runtime
bindings, linked bytecode execution, runtime typed identity, and
`ProgramVersion` ownership are all validated. This phase is cleanup, not a
place to discover missing architecture.

- [ ] **Task 9.1: Remove old stdlib ID APIs**

**Delete:**

- `vela_common::standard_ids`;
- `vela_engine::standard::ids`;
- any public re-export of old numeric constants.

**Acceptance criteria:**

- Grep for `0xff00_` finds no stdlib ID declarations.
- Grep for old constants such as `MATH_MAX_FUNCTION_ID` returns none, unless generated aliases are deliberately retained for internal tests. Prefer none.

- [ ] **Task 9.2: Remove name fallback fields from bytecode**

**Delete from linked bytecode:**

- `CallNative.name`;
- `CallNative.native: Option<FunctionId>`;
- `CallMethod.method` if used as hot dispatch operand;
- `CallMethod.value_method_id: Option<HostMethodId>` pattern;
- any `Option<Id>` where `None` means fallback by name.

**Acceptance criteria:**

- Linked bytecode has no optional identity fields.
- Missing identity is impossible after linking.

- [ ] **Task 9.3: Remove identity maps from `CompilerOptions`**

**Delete:**

- native function ID maps;
- host method ID maps;
- host field ID maps;
- value method param maps;
- type ID maps.

**Acceptance criteria:**

- Compiler identity lookup depends on registry.
- `CompilerOptions` contains only actual compiler settings.

- [ ] **Task 9.4: Remove runtime name maps**

**Delete from VM:**

- `HashMap<String, NativeFunction>`;
- `HashMap<String, HostNativeFunction>`;
- any dispatch lookup by native function name.

**Acceptance criteria:**

- VM native dispatch is table/handle based.
- Runtime errors use debug side table for names.

---

## 12. Suggested Codex Task Format

Use this template for each Codex task.

```markdown
## Task: <name>

### Objective
One-sentence goal.

### Context
Reference the architecture section and current files involved.

### Files likely touched
- path/to/file.rs
- path/to/other.rs

### Required changes
1. ...
2. ...
3. ...

### Must not do
- Do not preserve legacy fallback.
- Do not add compatibility aliases unless explicitly required by the task.
- Do not introduce a new duplicate source of truth.

### Acceptance criteria
- ...
- ...
- `cargo fmt --all -- --check`
- `cargo test -p <crate>` or focused workspace test

### Follow-up tasks
- ...
```

---

## 13. Validation Strategy

### 13.1 Per phase validation

Run focused tests during early phases:

```bash
cargo fmt --all -- --check
cargo test -p vela_def
cargo test -p vela_registry
cargo test -p vela_stdlib
cargo test -p vela_stdlib_runtime
```

After compiler/bytecode changes:

```bash
cargo test -p vela_bytecode
cargo test -p vela_vm
cargo test -p vela_engine
```

After each major integration phase:

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

### 13.2 New required tests

Add tests for:

- stable ID generation;
- duplicate path rejection;
- DefId collision rejection;
- stdlib manifest registration;
- generated VM stdlib registration;
- compiler native function resolution through registry;
- method resolution through registry;
- link failure on unknown function;
- link failure on missing native implementation;
- linked bytecode verifier rejecting invalid handles;
- VM native dispatch by handle;
- VM method dispatch by handle;
- Option/Result identity by TypeId/VariantId;
- no string fallback in linked VM execution.

### 13.3 Grep-based cleanup checks

Useful commands:

```bash
rg "standard_ids"
rg "0xff00_"
rg "CallNative \{"
rg "native: Option<FunctionId>"
rg "CallMethod \{"
rg "value_method_id: Option"
rg "native_ids"
rg "host_native_ids"
rg "HashMap<String, NativeFunction>"
rg "HashMap<String, HostNativeFunction>"
rg "CompilerOptions"
rg "compiler_options_from_registry"
rg "enum_name: String|variant: String"
rg "enum_tag\(|is_builtin_enum"
rg "ProgramImage"
rg "to_program\("
rg "vela_runtime_compile_source|Runtime::new|run_program\("
```

Expected final state:

- `standard_ids` not found;
- no handwritten stdlib raw ID constants;
- native and host-native dispatch maps by name removed;
- linked `CallNative` contains handle only;
- linked `CallMethod` contains linked method targets or handles only;
- optional semantic IDs are not used as name-fallback markers;
- `CompilerOptions` no longer carries identity maps;
- `ProgramImage::to_program` is not part of runtime execution or compatibility
  support;
- C API, WASM playground, examples, CLI, tests, benches, and docs use the final
  linked runtime entry path rather than old `Program`/name-dispatch assumptions;
- runtime Option/Result and script enum identity no longer uses string type or
  variant names.

---

## 14. Migration Notes

### 14.1 Recommended order

Do not start by rewriting the VM. Start with identity and registry.

Recommended order:

```text
1. vela_def
2. vela_registry
3. stdlib manifest
4. compiler registry queries
5. unlinked bytecode
6. linker
7. linked VM execution
8. runtime value typed identity
9. legacy cleanup
```

### 14.2 Avoid half-clean states

Avoid introducing another intermediate ID table that becomes permanent.

Bad:

```rust
pub const GENERATED_MATH_MAX_ID: FunctionId = ...
```

Better:

```rust
BuiltinFunction::MathMax.function_id()
```

Best:

```rust
registry.resolve_builtin(BuiltinFunction::MathMax)
```

### 14.3 Do not keep fallback for convenience

During transition it may be tempting to keep:

```rust
target: Option<NativeHandle>,
name: String,
```

This recreates the current problem.

Prefer a temporary separate instruction if absolutely necessary:

```rust
UnlinkedInstructionKind::CallUnresolvedByName { ... }
```

Then ensure it cannot appear in linked bytecode.

---

## 15. Definition of Done

This refactor is complete when:

1. `standard_ids.rs` is deleted.
2. Stdlib definitions are declared once in a manifest.
3. Stable typed IDs are generated from canonical definition paths with BLAKE3-128.
4. Compiler consumes `DefinitionRegistry` / `RegistryCompileView`, not identity maps in `CompilerOptions`.
5. Compiler emits unlinked bytecode with typed IDs.
6. Linker produces linked bytecode with dense handles, slots, and resolved targets.
7. `ProgramVersion` owns linked code, handle tables, debug names, profile layout, cache state, and hot-reload manifests.
8. VM executes linked bytecode only.
9. VM native/method/script dispatch does not use string fallback.
10. Runtime Option/Result and script enum identity use `TypeId`/`VariantId`, not strings.
11. Reflection/debug names come from side tables.
12. Old bytecode compatibility and old raw ID compatibility are not preserved.
13. HostAccess, reflection, budget, GC, and hot-reload invariants listed in section 2.1 are preserved.
14. Workspace tests pass after updating tests/examples to the new architecture.

---

## 16. Final Architecture Rule

Use this rule to evaluate every design decision in this refactor:

```text
If a value is used for human output, it is a debug name.
If a value identifies a semantic definition, it is a DefId wrapper.
If a value is used in VM hot dispatch, it is a linked handle or slot.
If a value can be missing after linking, linking is incomplete.
```

That rule should eliminate the current mixed model of names, optional IDs, fallback maps, and duplicated stdlib declarations.
