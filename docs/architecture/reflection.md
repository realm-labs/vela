## Reflection System

Reflection exists for:

1. Host type exposure.
2. Script type metadata queries.
3. Dynamic field reads/writes and method calls.
4. Diagnostic host path materialization for controlled inspection.
5. Hot reload ABI checks.
6. Debuggers, GM panels, admin backends, editors, and LSP support.

Allowed:

```text
query types, fields, methods, variants, traits, modules, and functions
controlled reflect::get / reflect::set / reflect::call
query trait implementations
query attributes
construct reflect paths
```

Forbidden:

```text
runtime add_field
runtime remove_field
runtime replace_method
runtime monkey patch
runtime eval-generated code
```

### TypeRegistry

```rust
pub struct TypeRegistry {
    pub types: HashMap<TypeKey, TypeDesc>,
    pub modules: HashMap<ModuleKey, ModuleDesc>,
    pub traits: HashMap<TraitKey, TraitDesc>,
    pub functions: HashMap<FunctionKey, FunctionDesc>,
    pub analysis: Option<RegistryAnalysisData>,
}
```

Hot reload creates a new `Arc<TypeRegistry>` instead of mutating the old registry.
The same registry should be serializable or queryable by editor tooling, so the
future LSP sees the exact host schema used by the runtime.

### Stable IDs

Do not rely only on strings or Rust `TypeId`.

```rust
pub struct TypeKey {
    pub module: Symbol,
    pub name: Symbol,
    pub stable_id: u64,
}
```

Fields, methods, variants, traits, and functions also need stable IDs:

```rust
pub struct FieldId(pub u64);
pub struct MethodId(pub u64);
pub struct VariantId(pub u64);
pub struct TraitId(pub u64);
pub struct FunctionId(pub u64);
```

Field order may change, but `FieldId` must not. Macro-generated IDs are
deterministically derived from script-facing stable paths and aliases, while
registration still rejects duplicate IDs.

### TypeDesc

```rust
pub struct TypeDesc {
    pub key: TypeKey,
    pub name: Symbol,
    pub module: Symbol,
    pub kind: TypeKind,
    pub schema_version: u32,
    pub schema_hash: u64,
    pub fields: Vec<FieldDesc>,
    pub methods: Vec<MethodDesc>,
    pub variants: Vec<VariantDesc>,
    pub implemented_traits: Vec<TraitKey>,
    pub attrs: AttrMap,
    pub origin: DeclOrigin,
    pub docs: Option<DocString>,
}

pub enum TypeKind {
    Null,
    Bool,
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    F32,
    F64,
    String,
    Bytes,
    Array,
    Map,
    Set,
    Range,
    Function,
    Closure,
    Host,
    ScriptStruct,
    ScriptEnum,
}
```

Reflection metadata values may contain arrays, maps, typed records, and typed
enums, but those shapes are represented by the reflection value model
(`ReflectValue::Array`, `ReflectValue::Map`, `ReflectValue::Record`,
`ReflectValue::ScriptRecord`, and `ReflectValue::ScriptEnum`). Script maps use
key-preserving map entries so non-string keys are not stringified at reflection
boundaries. `ReflectValue::Record` remains the string-field shape for copied
metadata records. These aggregate shapes are not `HostValue` payloads.
`HostValue` is reserved for scalar host-boundary values and host handles.

Tooling metadata:

```rust
pub struct RegistryAnalysisData {
    pub schema_source: SchemaSource,
    pub generated_at_schema_hash: u64,
}

pub enum SchemaSource {
    Script,
    HostRust,
    Generated,
    ExternalSchema,
}

pub struct DeclOrigin {
    pub source_id: Option<SourceId>,
    pub span: Option<SourceSpan>,
    pub generated: bool,
}

pub struct DocString {
    pub summary: String,
    pub details: Option<String>,
}
```

`DeclOrigin` is optional for host-generated schemas, but host macro output
should provide enough information for hover and go-to-definition when possible.

### TypeHint

There are no script-language generics, but lightweight type-hint contracts are
allowed. Only builtin contracts carry type arguments:

```rust
pub enum TypeHint {
    Any,
    Primitive(PrimitiveTag),
    Array,
    ArrayOf(Box<TypeHint>),
    Map,
    MapOf {
        key: Box<TypeHint>,
        value: Box<TypeHint>,
    },
    Set,
    SetOf(Box<TypeHint>),
    Iterator,
    IteratorOf(Box<TypeHint>),
    OptionOf(Box<TypeHint>),
    ResultOf {
        ok: Box<TypeHint>,
        err: Box<TypeHint>,
    },
    PathProxy,
    Record(TypeKey),
    Enum(TypeKey),
    Host(TypeKey),
    Trait(String),
    Function,
}
```

`TypeHint` is public metadata and syntax-facing documentation. Primitive hints
use the shared `PrimitiveTag` set: `null`, `bool`, `i8`, `i16`, `i32`, `i64`,
`u8`, `u16`, `u32`, `u64`, `f32`, `f64`, `char`, `string`, and `bytes`. Script-local
parameter, local, field, and return annotations are contracts, not conversions:
statically known mismatches are compile errors, and dynamic or externally
supplied mismatches are runtime guard errors.

Hints are still meaningful. They feed reflection metadata, hot-reload ABI
checks, diagnostics, completions, hover, dispatch hints, field-slot lowering,
and host schema documentation. They are not the complete internal analysis type
system. Keeping parameterization limited to builtin contracts preserves the
no-user-generics language rule and keeps host schemas stable.

### TypeFacts

The semantic analyzer should produce internal `TypeFact` values for diagnostics,
completion, hover, and limited type narrowing. These facts are analysis data,
not script syntax, so they may be more expressive than `TypeHint`.

```rust
pub enum TypeFact {
    Unknown,
    Never,
    Any,
    Primitive(PrimitiveTag),
    Range,
    Array {
        element: Box<TypeFact>,
    },
    Map {
        key: Box<TypeFact>,
        value: Box<TypeFact>,
    },
    Set {
        element: Box<TypeFact>,
    },
    Option {
        some: Box<TypeFact>,
    },
    OptionSome {
        some: Box<TypeFact>,
    },
    OptionNone,
    Result {
        ok: Box<TypeFact>,
        err: Box<TypeFact>,
    },
    ResultOk {
        ok: Box<TypeFact>,
    },
    ResultErr {
        err: Box<TypeFact>,
    },
    Function {
        params: Vec<TypeFact>,
        returns: Box<TypeFact>,
    },
    Record {
        name: String,
    },
    Enum {
        name: String,
        variant: Option<String>,
    },
    Host {
        name: String,
    },
    Trait {
        name: String,
    },
    Module {
        name: String,
    },
    PathProxy {
        root: Box<TypeFact>,
        path: HostPathShape,
        value: Box<TypeFact>,
    },
    Union(Vec<TypeFact>),
}

pub struct HostPathShape {
    pub root: TypeKey,
    pub segments: Vec<PathSegmentShape>,
}

pub enum PathSegmentShape {
    Field(FieldId),
    Index,
    Key,
    VariantField(FieldId),
}
```

Rules:

```text
Unknown means the analyzer cannot prove a useful type yet.
Any means the program explicitly entered a dynamic boundary.
Union is for local analysis and hover; it is not user-facing generic syntax.
Array/Map/Set element facts are inferred from literals, host schemas, stdlib
analysis rules, and local flow; scripts still write plain array/map/set hints.
Dynamic reflection and unknown host calls should degrade to Any instead of
blocking execution.
```

Examples:

```rust
let xs = [1, 2, 3]        // TypeFact::Array { element: Primitive(I64) }
let ys: Array = []        // public TypeHint::Array, internal element Unknown
let z = reflect::get(x, k) // Any unless k is a known constant and schema exists
```

### Semantic Model For Tools

The resolver should build a semantic model that can be reused by the compiler,
diagnostics, and future LSP support:

```rust
pub struct SemanticModel {
    pub modules: ModuleGraph,
    pub symbols: SymbolTable,
    pub bindings: BindingMap,
    pub expr_facts: HashMap<ExprId, TypeFact>,
    pub pattern_facts: HashMap<PatternId, TypeFact>,
    pub diagnostics: Vec<Diagnostic>,
}
```

Minimum capabilities:

```text
resolve imports and module exports
resolve local bindings, function parameters, fields, methods, variants, traits
track source spans for declarations and references
infer expression facts when cheap and deterministic
apply flow narrowing for if/match and null checks
report unresolved names with candidate suggestions
report field and method errors when receiver facts are known
degrade to Any at dynamic boundaries
```

LSP completion should prefer:

```text
local bindings
function parameters
imported symbols
fields and methods from receiver TypeFact
enum variants and match patterns
stdlib functions and methods
reflect APIs
```

This design supports strong editor hints where the program provides enough
metadata, while preserving Vela as a dynamic language at runtime.

### FieldDesc

```rust
pub struct FieldDesc {
    pub id: FieldId,
    pub name: Symbol,
    pub hint: TypeHint,
    pub access: FieldAccess,
    pub storage: FieldStorage,
    pub default_value: Option<Value>,
    pub attrs: AttrMap,
    pub origin: DeclOrigin,
    pub docs: Option<DocString>,
}

pub struct FieldAccess {
    pub readable: bool,
    pub writable: bool,
    pub reflect_readable: bool,
    pub reflect_writable: bool,
}

pub enum FieldStorage {
    RecordSlot { slot: u16 },
    HostField { field_id: FieldId },
    Computed {
        getter: HostMethodId,
        setter: Option<HostMethodId>,
    },
}
```

### MethodDesc

```rust
pub struct MethodDesc {
    pub id: MethodId,
    pub name: Symbol,
    pub params: Vec<ParamDesc>,
    pub returns: TypeHint,
    pub kind: MethodKind,
    pub effects: EffectSet,
    pub access: MethodAccess,
    pub attrs: AttrMap,
    pub origin: DeclOrigin,
    pub docs: Option<DocString>,
}

pub struct ParamDesc {
    pub name: Symbol,
    pub hint: TypeHint,
    pub has_default: bool,
    pub default_value: Option<Value>,
}

pub enum MethodKind {
    ScriptFunction(FunctionKey),
    HostNative(HostMethodId),
    TraitMethod(TraitKey, MethodId),
}
```

### EffectSet

```rust
pub struct EffectSet {
    pub read_host: bool,
    pub write_host: bool,
    pub io: bool,
    pub random: bool,
    pub network: bool,
    pub may_yield: bool, // reserved for future coroutine/async support
}
```

Effects are used for:

```text
permission control
sandboxing
hot reload ABI checks
debugger hints
function budgets
event-system constraints
```

For the MVP, script execution does not yield inside a call. `may_yield` exists
only so future ABI checks can reserve the effect bit; it should remain `false`
for all MVP functions.

### Attributes

```rust
pub enum AttrValue {
    Bool(bool),
    Scalar(ScalarValue),
    String(Symbol),
    Array(Vec<AttrValue>),
    Map(HashMap<Symbol, AttrValue>),
}

pub type AttrMap = HashMap<Symbol, AttrValue>;
```

Example:

```rust
#[event("invoice.paid")]
#[budget(instructions = 50000)]
pub fn on_invoice_paid(ctx, account, invoice) {
    // ...
}
```

### Script Reflection API

First-version API:

```rust
reflect::type_of(value)
reflect::types()
reflect::type_info(name)
reflect::has_type(name)
// type queries return copied ReflectType descriptor records
reflect::name(type)
reflect::kind(type)
reflect::owner(descriptor)
reflect::origin(descriptor)
reflect::access(descriptor)

reflect::fields(type)
reflect::field(type, name)
// field queries return copied ReflectField descriptor records

reflect::has_field(value, name)
reflect::get(value, name)
reflect::set(value, name, value)

reflect::methods(type)
reflect::method(type, name)
reflect::has_method(value, name)
reflect::call(value, name, args)
reflect::params(value)
reflect::returns(value)

reflect::variants(type)
reflect::variant(value)
reflect::variant_info(value, name)
reflect::variant_is(value, name)
reflect::has_variant(value, name)

reflect::implements(value, trait)
reflect::traits(type)
reflect::trait_info(name)
reflect::has_trait(name)

reflect::modules()
reflect::module(name)
reflect::has_module(name)
reflect::functions()
reflect::function(name)
reflect::has_function(name)
reflect::exports(module)

reflect::permissions()
reflect::has_permission(name)
```

For `HostRef`, `reflect::set(account, "balance", 10)` routes through `HostAccess` and writes the adapter immediately.
For script records and enum payload records, `reflect::set(value, name, new_value)`
returns an updated copied value. It does not mutate the caller's existing local
binding unless the script assigns the returned value, and it rejects unknown
fields instead of adding runtime schema members.

Dot syntax and reflection share the same path foundation:

```text
account.balance = 10                  -> compile-time FieldId, fast
reflect::set(account, "balance", 10)  -> runtime FieldId lookup, slower but flexible
```

### Reflection Permissions

```rust
pub enum ReflectPermission {
    ReadTypeInfo,
    ReadValueFields,
    WriteValueFields,
    CallMethods,
    CallHostReadMethods,
    CallHostWriteMethods,
    CallEventMethods,
    AccessPrivate,
    InspectHostPath,
}
```

Suggested defaults:

| Script Kind | Read Types | Read Fields | Write Fields | Call Methods | Host Read Effects | Host Write Effects | Event Effects | Private | Inspect HostPath |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| embedded app | yes | yes | cautious | yes | yes | cautious | cautious | no | no |
| config validation | yes | yes | no | pure only | no | no | no | no | no |
| GM/admin | yes | yes | yes | yes | configurable | configurable | configurable | configurable | yes |
| test script | yes | yes | yes | yes | yes | yes | yes | configurable | yes |

Field descriptors may also carry required reflection permission names. Policy
checks filter `reflect::fields`, `reflect::field`, `reflect::has_field`, and enum
payload field metadata by those names. Dynamic `reflect::get` / `reflect::set`
on host refs fail before reading or recording a patch when the active policy
lacks a required field permission. Dynamic script record and enum payload
reflection uses the same permission metadata when the registry knows the script
field, while `reflect::set` still returns an updated copied value rather than
mutating type structure.
