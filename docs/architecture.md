# Architecture

This document describes the technical architecture for a Hot Reload First dynamic scripting language implemented in Rust for game server logic.

The core idea is:

```text
Scripts describe game logic with natural syntax.
The VM represents mutations to the Rust world as PatchTx operations.
The runtime performs reliable function-level hot reload by replacing CodeObject mappings.
```

## Reference Designs

These projects are useful references, but this language should not copy them directly.

| Project | Useful Ideas | Do Not Copy |
|---|---|---|
| Luau | High-quality interpreter, bytecode optimization, inline caches, game-logic performance focus | Lua syntax and table/metatable object model |
| Wren | Small embedded VM and restrained syntax | The Rust host patch model needs custom design |
| Rhai | Rust embedding experience and small-language strategy | Expression power and hot reload are not enough for this goal |
| Rune | Rust-like dynamic language, VM, hot reload, Rust embedding | The host state PatchTx model is more specialized |
| Starlark | Determinism, restraint, and tool friendliness | It is not a direct fit for high-performance game server logic |
| Mun | Hot Reload First runtime ideas | Static typing and LLVM/AOT are different from this project |

References:

- Luau performance: https://luau.org/performance/
- Mun language: https://mun-lang.org/
- Mun GitHub: https://github.com/mun-lang/mun
- Codex goals: https://developers.openai.com/codex/use-cases/follow-goals
- Codex goal cookbook: https://developers.openai.com/cookbook/examples/codex/using_goals_in_codex
- Codex best practices: https://developers.openai.com/codex/learn/best-practices

## Compile And Runtime Pipeline

```text
Source Code
   ↓
Lexer / Parser
   ↓
CST / AST
   ↓
Resolver / Symbol Table / Semantic Model
   ↓
HIR / Lowered IR / TypeFacts
   ↓
Bytecode Compiler
   ↓
CodeObject / ProgramVersion
   ↓
VM Runtime / GC / Stack / CallFrame
   ↓
Host Bridge / Reflection / PatchTx
   ↓
Rust World / ECS / Actor State / Database Adapter
```

## File Extensions

Vela source files use `.vela`.

Precompiled bytecode-only artifacts use `.vbc` when that cache/artifact format
is implemented. If a future deployment package needs bytecode plus ABI
manifest, schema metadata, source maps, and reload metadata, it should use a
separate package format rather than overloading `.vbc`.

## Suggested Workspace Structure

```text
vela/
  Cargo.toml
  crates/
    vela_common/          # Span, Symbol, IDs, diagnostics
    vela_syntax/          # Lexer, parser, lossless CST, AST
    vela_hir/             # Resolver, HIR, name binding
    vela_analysis/        # Semantic model, TypeFacts, completion data
    vela_bytecode/        # Instruction, CodeObject, compiler
    vela_vm/              # Runtime, VM, Value, GC, call frames
    vela_reflect/         # TypeRegistry, TypeDesc, reflection API
    vela_host/            # HostRef, HostPath, PatchTx, StateAdapter
    vela_macros/          # #[derive(ScriptHost)] and related macros
    vela_std/             # Native standard library implementation
    vela_hot_reload/      # ProgramVersion, ABI diff, code swap
    vela_lsp/             # Future language server, not part of MVP
    vela_cli/             # repl, compile, run, hot reload demo
  examples/
    game_server_demo/
  docs/
    architecture.md
    grammar.ebnf
    goal.md
    progress.md
    decisions.md
    blocked.md
    performance.md
    reflection.md
    hot_reload.md
    host_bridge.md
  tests/
    fixtures/
```

## Implementation Architecture Hygiene

The implementation should prefer clean architecture over compatibility with
old internal shapes. During pre-release development, obsolete internal APIs,
transitional behavior, and temporary artifacts should be replaced instead of
kept behind compatibility shims. This rule does not apply to product-level hot
reload ABI and schema compatibility checks, which remain part of the runtime
contract.

Code structure rules:

```text
split large files by crate/module responsibility
split large functions when control flow stops being locally understandable
extract cohesive parameter structs when function signatures grow around one concept
replace accumulating conditional branches with match, enum-driven dispatch, tables, or focused helper types
move feature-specific policy out of generic execution loops when it starts to distort the loop
adjust architecture when a feature can only be added through awkward patch code
```

Compatibility rules:

```text
do not add aliases, duplicate APIs, or migration paths only to preserve old internal callers
do not keep legacy behavior in parallel with new behavior unless a milestone explicitly requires both
update tests and examples to the current architecture instead of supporting old paths
document accepted product compatibility rules in hot reload, schema ABI, and artifact formats
```

## Critical Vertical Loop

The first phase should close this loop:

```text
Rust Host Type Metadata
        ↓
script dot-syntax access
        ↓
FieldId / MethodId compile-time resolution
        ↓
VM bytecode execution
        ↓
HostRef / PathProxy
        ↓
PatchTx collects changes
        ↓
host safe-point commit
        ↓
hot reload replaces function CodeObject values
```

## Language Semantics

The first grammar draft lives in [grammar.ebnf](grammar.ebnf). It is the syntax
target for the parser milestones before semantic validation and lowering.

Parser implementations should preserve source spans for every token and AST
node. A future LSP needs this for diagnostics, completion replacement ranges,
go-to-definition, rename, hover, and incremental reparsing. The compiler may
lower into a simpler AST/HIR, but the syntax layer should keep a lossless CST or
equivalent token tree with comments and newlines.

Example script:

```rust
use game.player.Player
use game.reward.Reward

struct KillReward {
    item_id
    count
}

enum QuestProgress {
    None
    Active { quest_id, count }
    Finished { quest_id }
}

trait Damageable {
    fn damage(self, amount)
}

#[event("monster.kill")]
pub fn on_kill(ctx, player, monster) {
    player.exp += monster.exp

    if player.exp >= ctx.config.exp_to_next_level(player.level) {
        player.level += 1
        player.exp = 0
        ctx.emit("player.level_up", player.id, player.level)
    }

    let rewards = ctx.config.kill_rewards
        .filter(|r| r.monster_id == monster.id)
        .map(|r| KillReward {
            item_id: r.item_id,
            count: r.count,
        })

    for reward in rewards {
        player.inventory.add(reward.item_id, reward.count)
    }

    match player.quest_progress {
        QuestProgress.Active { quest_id, count } => {
            player.quest_progress = QuestProgress.Active {
                quest_id,
                count: count + 1,
            }
        }
        _ => {}
    }
}
```

### Dynamic Type Boundary

The language is dynamically typed, with lightweight hints and metadata.

Function overloading is not part of the language. A module may contain only one
function for a given name, and a type or trait may contain only one method for a
given receiver/name pair. Parameter count, type hints, default values, and
native Rust signatures do not create overload sets.

Different value categories may independently define the same method name, such
as string and array helpers, because dispatch starts from the receiver category
or reflected receiver type. That is receiver-based method dispatch, not
same-scope overload resolution.

Supported value categories:

```text
null
bool
int
float
string
array
map
set
range
function
closure
record / struct-like value
enum / tagged union
host ref
path proxy
trait/protocol object
```

Script generics are not supported:

```text
Array<T>      not supported
Map<K, V>     not supported
Option<T>     not supported
Result<T, E>  not supported
```

Dynamic enum definitions can still model Option and Result:

```rust
enum Option {
    Some(value)
    None
}

enum Result {
    Ok(value)
    Err(error)
}
```

`null`, `Option`, `Result`, and runtime errors have separate responsibilities:

```text
null        no meaningful value, void-like results, host nullable boundaries, or missing metadata
Option.None expected absence in gameplay or lookup logic
Result.Err  recoverable failure with a script-visible reason
VM error    unrecoverable trap, script bug, contract violation, budget failure, or sandbox denial
```

Script and standard-library APIs should prefer `Option` for expected missing
data and `Result` for expected recoverable failure. They should not use `null`
as the normal "not found" or "failed" result. `null` remains the value for
statement-only blocks, no-result native calls, reflection metadata gaps, and
host/Rust nullable interop.

Control-flow expressions produce values. Empty or statement-only blocks
evaluate to `null`, and expression-valued `if` without an `else` evaluates to
`null` on the untaken branch.

### Dynamic Traits / Protocols

Traits are runtime capabilities or protocols, not Rust traits.

Supported:

```text
trait method declarations
trait default methods
host type implementations
script type implementations
runtime implements checks
dynamic trait method dispatch
```

Not supported:

```text
generic traits
associated types
complex where clauses
traits participating in object memory layout
static monomorphization
```

## Host State Bridge

The host state bridge is the central differentiator. Scripts must not receive real mutable Rust references.

Wrong direction:

```rust
&mut Player
```

Correct direction:

```rust
HostRef<Player>
PathProxy<Player.level>
PatchTx
```

Script code looks natural:

```rust
player.level += 1
player.exp = 0
player.inventory.add("gold", 100)
```

Runtime operations are explicit:

```text
ReadModifyWrite(player.level, Add(1))
Set(player.exp, 0)
CallHostMethod(player.inventory, add, ["gold", 100])
```

### HostRef

```rust
pub struct HostRef {
    pub type_id: HostTypeId,
    pub object_id: HostObjectId,
    pub generation: u32,
}
```

`generation` prevents stale references from writing to a new object after ID reuse.

### HostPath

```rust
pub struct HostPath {
    pub root: HostRef,
    pub segments: SmallVec<[PathSegment; 4]>,
}

pub enum PathSegment {
    Field(FieldId),
    Index(u32),
    Key(Symbol),
    VariantField(FieldId),
}
```

### PatchTx

```rust
pub struct PatchTx {
    pub patches: Vec<Patch>,
    pub overlay: PatchOverlay,
}

pub struct Patch {
    pub path: HostPath,
    pub op: PatchOp,
    pub source_span: SourceSpan,
}

pub enum PatchOp {
    Set(Value),
    Add(Value),
    Sub(Value),
    Remove,
    Push(Value),
    CallHostMethod {
        method: HostMethodId,
        args: Vec<Value>,
    },
}
```

### Read And Write Semantics

Scripts must observe writes made earlier in the same transaction:

```rust
player.level = 10
print(player.level) // prints 10
```

Read logic:

```text
read_path(path):
    if tx.overlay has path:
        return overlay value
    else:
        return host snapshot value
```

Write logic:

```text
write_path(path, value):
    validate access
    record patch
    update overlay
```

### Read-Modify-Write

`player.level += 1` should prefer an explicit operation:

```rust
PatchOp::Add(Value::Int(1))
```

This lets the host perform atomic validation, range checks, conflict handling, and logging during apply.

### Host State Adapter

```rust
pub trait ScriptStateAdapter {
    fn read_path(&self, path: &HostPath) -> HostResult<Value>;

    fn write_path(&mut self, path: &HostPath, value: Value) -> HostResult<()>;

    fn call_method(
        &mut self,
        path: &HostPath,
        method: HostMethodId,
        args: &[Value],
    ) -> HostResult<Value>;

    fn validate_patch(&self, patch: &Patch) -> HostResult<()>;

    fn apply_patch(&mut self, patch: Patch) -> HostResult<()>;
}
```

The same runtime can adapt to:

```text
plain Rust structs
ECS worlds
actor state
database entities
network-replicated state
test mock state
```

## Rust Host Macros

### Type Exposure

```rust
#[derive(ScriptHost, ScriptReflect)]
#[script(name = "Player", id = 1001, module = "game.player")]
pub struct Player {
    #[script(get, set, id = 1)]
    pub level: u32,

    #[script(get, set, id = 2)]
    pub exp: u64,

    #[script(get, set, id = 3)]
    pub title: String,

    #[script(get, id = 4)]
    pub inventory: Inventory,
}
```

### Method Exposure

```rust
#[script_methods]
impl Player {
    #[script_method(id = 1, effect = "write_host")]
    pub fn add_exp(
        ctx: &mut NativeCallContext,
        player: HostRef<Player>,
        amount: i64,
    ) -> HostResult<()> {
        ctx.tx.push_add(player.field(FieldId(2)), Value::Int(amount))
    }
}
```

Host method implementations may mutate real Rust state later inside the host
adapter apply path, but the VM-facing callable receives `HostRef`, `HostPath`,
or copied values rather than `&mut self`.

### Generated Items

Macros should generate at least:

```text
TypeDesc
FieldDesc list
MethodDesc list
read_field / write_field helpers
method dispatch helpers
schema_hash
stable ID validation
```

## Host Function Registration

Host functions are Rust functions registered into the Vela engine as native
callables. They are used for logging, deterministic utility APIs, event context
helpers, config access, controlled random, metrics, and host-provided services.

Native functions follow the same no-overload rule as script functions. Each
public native callable has one canonical module/name and one stable ID. Hosts
should use explicit names such as `spawn_monster` and `spawn_monster_at`
instead of registering multiple signatures under the same script-visible name.

There are three registration shapes:

```text
global function       log("message")
module function       math.clamp(value, min, max)
host type method      player.inventory.add(item_id, count)
```

All three shapes must become registry entries with stable IDs, signatures,
effects, permissions, docs, and conversion rules. Scripts call them normally,
but the VM dispatches them through a native function table.

### Native Function Descriptor

```rust
pub struct NativeFunctionDesc {
    pub id: NativeFunctionId,
    pub module: Symbol,
    pub name: Symbol,
    pub params: Vec<ParamDesc>,
    pub returns: TypeHint,
    pub effects: EffectSet,
    pub access: FunctionAccess,
    pub attrs: AttrMap,
    pub origin: DeclOrigin,
    pub docs: Option<DocString>,
}

pub struct NativeFunctionId(pub u64);

pub struct FunctionAccess {
    pub public: bool,
    pub reflect_visible: bool,
    pub reflect_callable: bool,
    pub required_permissions: PermissionSet,
}
```

Native functions are also exposed through `FunctionDesc` so reflection, hot
reload ABI checks, diagnostics, and future LSP tooling see the same function
surface as the VM.

```rust
pub enum FunctionKind {
    Script(CodeObjectId),
    HostNative(NativeFunctionId),
}

pub struct FunctionDesc {
    pub key: FunctionKey,
    pub name: Symbol,
    pub module: Symbol,
    pub params: Vec<ParamDesc>,
    pub returns: TypeHint,
    pub kind: FunctionKind,
    pub effects: EffectSet,
    pub access: FunctionAccess,
    pub attrs: AttrMap,
    pub origin: DeclOrigin,
    pub docs: Option<DocString>,
}
```

### Native Function Trait

The VM should call host functions through a small erased trait:

```rust
pub trait NativeFunction: Send + Sync + 'static {
    fn desc(&self) -> &NativeFunctionDesc;

    fn call(
        &self,
        ctx: &mut NativeCallContext,
        args: &[Value],
    ) -> HostResult<Value>;
}

pub struct NativeCallContext<'a> {
    pub runtime: &'a mut Runtime,
    pub state: &'a mut dyn ScriptStateAdapter,
    pub tx: &'a mut PatchTx,
    pub permissions: &'a PermissionSet,
    pub budget: &'a mut ExecutionBudget,
}
```

`NativeCallContext` is the only native entry point that may touch host services
or `PatchTx`. A native function must not hand real Rust references back to the
script. Returned host objects must be represented as `HostRef`, copied
host-value data, or script-owned `Value`.

The engine owns the executable native function table separately from the
reflectable descriptors:

```rust
pub struct Engine {
    pub registry: Arc<TypeRegistry>,
    pub native_functions: HashMap<NativeFunctionId, Arc<dyn NativeFunction>>,
    pub native_methods: HashMap<HostMethodId, Arc<dyn NativeFunction>>,
}
```

### Builder API

The engine builder should support explicit descriptors for stable schemas:

```rust
let engine = Engine::builder()
    .register_native_fn(
        NativeFunctionDesc::new("game.log", NativeFunctionId(10_001))
            .param("message", TypeHint::String)
            .returns(TypeHint::Null)
            .effects(EffectSet::pure_host_log())
            .docs("Writes to the game log."),
        game_log,
    )
    .register_native_fn(
        NativeFunctionDesc::new("math.clamp", NativeFunctionId(20_001))
            .param("value", TypeHint::Float)
            .param("min", TypeHint::Float)
            .param("max", TypeHint::Float)
            .returns(TypeHint::Float)
            .effects(EffectSet::pure()),
        math_clamp,
    )
    .build()?;
```

For simple cases, a convenience wrapper may infer descriptors from Rust function
signatures, but production host APIs should prefer explicit stable IDs:

```rust
let engine = Engine::builder()
    .register_fn("game.log", game_log)?
    .build()?;
```

### Rust Signature Mapping

Native functions should use narrow conversion rules:

```text
Rust bool/i64/f64/String          <-> Vela bool/int/float/string
Option<T> in Rust API             <-> nullable argument or return value
Vec<T> / HashMap<K, V> copies      <-> script array/map values
HostRef<T>                         <-> host object reference
&mut NativeCallContext             -> explicit host access and PatchTx access
HostResult<T>                      -> Vela call success or diagnostic error
```

Do not expose these Rust types directly to scripts:

```text
&T
&mut T
Arc<Mutex<T>>
database connection handles
network connection handles
runtime-owned service pointers
```

If a native function needs to mutate host state, it should either:

```text
record PatchTx operations through NativeCallContext
call ScriptStateAdapter methods
return a value that script code later writes through normal PatchTx paths
```

### Method Registration

Host type methods are registered through `#[script_methods]` and become
`MethodDesc { kind: MethodKind::HostNative(...) }`. Method calls receive the
receiver as a host path or host ref, not as `&mut T` in the VM.

```rust
#[script_methods]
impl Inventory {
    #[script_method(
        id = 1,
        name = "add",
        effect = "write_host",
        docs = "Adds an item stack to this inventory."
    )]
    pub fn add(
        ctx: &mut NativeCallContext,
        inventory: HostRef<Inventory>,
        item_id: String,
        count: i64,
    ) -> HostResult<()> {
        ctx.tx.push_method_call(inventory, HostMethodId(1), vec![
            Value::String(item_id.into()),
            Value::Int(count),
        ])?;
        Ok(())
    }
}
```

This keeps method syntax ergonomic:

```rust
player.inventory.add("gold", 100)
```

while preserving the host boundary:

```text
CallHostMethod(player.inventory, add, ["gold", 100])
```

### Registration Rules

```text
function module/name/stable_id must be unique
function overloading is unsupported; duplicate script-visible names are invalid
registered signatures must be deterministic and serializable into TypeRegistry
effects must be declared up front
permission checks happen before native call dispatch
native calls consume execution budget
native functions cannot store Value or HostRef beyond the call unless explicitly allowed
native functions cannot mutate TypeRegistry at runtime
reflection can call only reflect_callable native functions
hot reload can replace script functions, but host native function ABI is fixed for the engine version
```

## Reflection System

Reflection exists for:

1. Host type exposure.
2. Script type metadata queries.
3. Dynamic field reads/writes and method calls.
4. Automatic `HostPath` / `Patch` construction.
5. Hot reload ABI checks.
6. Debuggers, GM panels, admin backends, editors, and LSP support.

Allowed:

```text
query types, fields, methods, variants, traits, modules, and functions
controlled reflect.get / reflect.set / reflect.call
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
pub struct FieldId(pub u32);
pub struct MethodId(pub u32);
pub struct VariantId(pub u32);
pub struct TraitId(pub u64);
pub struct FunctionId(pub u64);
```

Field order may change, but `FieldId` must not.

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
    Int,
    Float,
    String,
    Array,
    Map,
    Set,
    ScriptRecord,
    ScriptEnum,
    HostObject,
    HostValue,
    Trait,
    Function,
    Closure,
}
```

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

There are no generics, but lightweight type hints are allowed:

```rust
pub enum TypeHint {
    Any,
    Null,
    Bool,
    Int,
    Float,
    String,
    Array,
    Map,
    Set,
    Record(TypeKey),
    Enum(TypeKey),
    Host(TypeKey),
    Trait(TraitKey),
    Function,
}
```

`TypeHint` is public metadata and syntax-facing documentation. Script-local
parameter, local, field, and return annotations do not enforce runtime value
types by themselves; a function annotated `fn f(name: string) -> string` can
still receive or return a different script value unless a host/native/schema
boundary explicitly performs conversion or validation.

Hints are still meaningful. They feed reflection metadata, hot-reload ABI
checks, diagnostics, completions, hover, dispatch hints, field-slot lowering,
and host schema documentation. They are not the complete internal analysis type
system. Keeping them small preserves the no-generics language rule and keeps
host schemas stable.

### TypeFacts

The semantic analyzer should produce internal `TypeFact` values for diagnostics,
completion, hover, and limited type narrowing. These facts are analysis data,
not script syntax, so they may be more expressive than `TypeHint`.

```rust
pub enum TypeFact {
    Unknown,
    Any,
    Never,
    Null,
    Bool,
    Int,
    Float,
    String,
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
    Record(TypeKey),
    Enum(TypeKey),
    Host(TypeKey),
    Trait(TraitKey),
    Function(FunctionSigFact),
    PathProxy {
        root: Box<TypeFact>,
        path: HostPathShape,
        value: Box<TypeFact>,
    },
    Union(Vec<TypeFact>),
}

pub struct FunctionSigFact {
    pub params: Vec<TypeFact>,
    pub returns: Box<TypeFact>,
    pub effects: EffectSet,
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
let xs = [1, 2, 3]        // TypeFact::Array { element: Int }
let ys: array = []        // public TypeHint::Array, internal element Unknown
let z = reflect.get(x, k) // Any unless k is a known constant and schema exists
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
    Int(i64),
    Float(f64),
    String(Symbol),
    Array(Vec<AttrValue>),
    Map(HashMap<Symbol, AttrValue>),
}

pub type AttrMap = HashMap<Symbol, AttrValue>;
```

Example:

```rust
#[event("monster.kill")]
#[budget(instructions = 50000)]
pub fn on_kill(ctx, player, monster) {
    // ...
}
```

### Script Reflection API

First-version API:

```rust
reflect.type_of(value)
reflect.types()
reflect.type_info(name)
reflect.has_type(name)
// type queries return copied ReflectType descriptor records
reflect.name(type)
reflect.kind(type)
reflect.owner(descriptor)
reflect.origin(descriptor)
reflect.access(descriptor)

reflect.fields(type)
reflect.field(type, name)
// field queries return copied ReflectField descriptor records

reflect.has_field(value, name)
reflect.get(value, name)
reflect.set(value, name, value)

reflect.methods(type)
reflect.method(type, name)
reflect.has_method(value, name)
reflect.call(value, name, args)
reflect.params(value)
reflect.returns(value)

reflect.variants(type)
reflect.variant(value)
reflect.variant_info(value, name)
reflect.variant_is(value, name)
reflect.has_variant(value, name)

reflect.implements(value, trait)
reflect.traits(type)
reflect.trait_info(name)
reflect.has_trait(name)

reflect.modules()
reflect.module(name)
reflect.has_module(name)
reflect.functions()
reflect.function(name)
reflect.has_function(name)
reflect.exports(module)

reflect.permissions()
reflect.has_permission(name)
```

For `HostRef`, `reflect.set(player, "level", 10)` creates a `Patch` instead of mutating Rust directly.
For script records and enum payload records, `reflect.set(value, name, new_value)`
returns an updated copied value. It does not mutate the caller's existing local
binding unless the script assigns the returned value, and it rejects unknown
fields instead of adding runtime schema members.

Dot syntax and reflection share the same path foundation:

```text
player.level = 10                -> compile-time FieldId, fast
reflect.set(player, "level", 10) -> runtime FieldId lookup, slower but flexible
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
| gameplay | yes | yes | cautious | yes | yes | cautious | cautious | no | no |
| config validation | yes | yes | no | pure only | no | no | no | no | no |
| GM/admin | yes | yes | yes | yes | configurable | configurable | configurable | configurable | yes |
| test script | yes | yes | yes | yes | yes | yes | yes | configurable | yes |

Field descriptors may also carry required reflection permission names. Policy
checks filter `reflect.fields`, `reflect.field`, `reflect.has_field`, and enum
payload field metadata by those names. Dynamic `reflect.get` / `reflect.set`
on host refs fail before reading or recording a patch when the active policy
lacks a required field permission. Dynamic script record and enum payload
reflection uses the same permission metadata when the registry knows the script
field, while `reflect.set` still returns an updated copied value rather than
mutating type structure.

## Struct, Record, And Enum Memory Model

### Record

Script structs are dynamic values with stable shapes:

```rust
struct Position {
    x
    y
}
```

Runtime:

```rust
ObjRecord {
    shape_id: ShapeId,
    fields: Vec<Value>,
}
```

Field access:

```rust
pos.x
```

Compiles to:

```text
GET_FIELD_CONST r_dst, r_obj, shape=Position, field=x, slot=0
```

If the shape matches, the VM reads the slot directly. Otherwise it falls back to the slow path.

### Enum

```rust
enum Damage {
    Physical { amount }
    Magical { amount, element }
    True { amount }
}
```

Runtime:

```rust
ObjEnum {
    enum_id: TypeKey,
    variant_id: VariantId,
    fields: Vec<Value>,
}
```

`match` compiles into tag checks and field bindings.

## VM And Bytecode

Use register-based bytecode:

```text
LOAD_CONST      r0, const#10
GET_HOST_FIELD  r1, player, FieldId(level)
ADD             r2, r1, r0
SET_HOST_FIELD  player, FieldId(level), r2
RETURN          null
```

Benefits:

```text
fewer instructions
local optimization is easier
field and method access can be specialized
good fit for later inline caches
```

### Value

Use a clear first implementation before low-level layout optimization:

```rust
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(GcRef<ObjString>),
    Array(GcRef<ObjArray>),
    Map(GcRef<ObjMap>),
    Set(GcRef<ObjSet>),
    Record(GcRef<ObjRecord>),
    Enum(GcRef<ObjEnum>),
    Closure(GcRef<ObjClosure>),
    HostRef(HostRef),
    PathProxy(PathProxy),
    TraitObject(TraitObject),
}
```

Only consider the following after profiling proves `Value` overhead is too high:

```text
16-byte tagged value
NaN boxing
pointer tagging
specialized arrays
```

### Execution Budget

The VM charges an instruction budget while executing:

```rust
pub struct ExecutionBudget {
    pub instruction_limit: u64,
    pub memory_limit_bytes: usize,
    pub max_call_depth: usize,
    pub max_patches: usize,
}
```

Budgets prevent:

```text
infinite loops
unbounded memory growth
recursive stack overflow
too many state writes in a single event
```

## Threading Model

Vela is a single-threaded scripting language from the script author's point of
view. A single `Runtime` executes one script call at a time on one OS thread,
with one VM stack, one active `PatchTx`, and one script heap/GC context.

The language does not expose:

```text
thread creation
shared-memory concurrency
locks or atomics
async/await
coroutines
channels
parallel iterators
```

If a game server needs concurrency, the Rust host owns it. The host may run
multiple independent Vela runtimes on different threads, shard actors across
workers, schedule events on an async runtime, or perform IO in background tasks.
Each script invocation still observes a single-threaded VM boundary.

Allowed host-level concurrency models:

```text
one Runtime per actor worker
one Runtime per shard or scene
runtime pool with no concurrent use of the same Runtime
host async tasks that call into Vela only at explicit scheduling points
background IO that returns copied data or HostRef handles to later script calls
```

Required boundaries:

```text
do not call the same Runtime concurrently from multiple threads
do not share script GC objects across runtimes or threads
do not let native functions store borrowed Value references after a call
do not expose host locks, atomics, or thread handles to scripts
do not apply PatchTx concurrently with VM execution for the same host object set
```

Data crossing host threads must be copied, serialized, or represented by stable
host handles such as `HostRef`. Cross-thread conflict resolution, ordering,
locking, database transactions, actor mailboxes, and network IO are host
responsibilities, not Vela language features.

Hot reload follows the same rule: the host may coordinate update distribution
across worker threads, but each runtime swaps `ProgramVersion` only at its own
safe points.

## GC

GC manages:

```text
string objects
arrays
maps
sets
records
enums
closures
upvalues
iterators
call frame objects
```

GC does not manage:

```text
Rust Player
Rust World
Rust Inventory
database objects
network connections
```

Scripts hold only `HostRef` values for host state.

First-version GC:

```text
non-moving mark-sweep
arena allocation
explicit root stack
event/tick boundary step_gc
configurable GC budget
```

API:

```rust
runtime.step_gc(GcBudget::micros(200));
runtime.collect_full_gc();
runtime.set_gc_config(GcConfig {
    max_pause_micros: 500,
    heap_growth_factor: 1.5,
});
```

Moving GC is deferred because it complicates:

```text
GcRef stability
host bridge
debugger
reflection objects
call frames
FFI/native functions
```

## Hot Reload First

### Core Model

```rust
pub struct Runtime {
    pub current: ArcSwap<ProgramVersion>,
    pub active_versions: VersionEpochs,
}

pub struct ProgramVersion {
    pub id: VersionId,
    pub registry: Arc<TypeRegistry>,
    pub modules: HashMap<ModuleId, Module>,
    pub functions: HashMap<FunctionSymbolId, Arc<CodeObject>>,
}
```

### Function Calls Use Indirection

Calling:

```rust
combat.on_kill(player, monster)
```

Internally uses:

```text
FunctionSymbolId("combat.on_kill")
```

At call time:

```text
FunctionSymbolId -> current ProgramVersion -> CodeObject
```

Hot reload replaces the mapping.

### Old Stack And New Stack

Rules:

```text
currently executing old functions continue on old CodeObject values
new calls use new CodeObject values
old ProgramVersion values are released after all old stacks exit
updates take effect only at safe points
```

The first version does not switch bytecode in the middle of an executing function.

### Safe Points

Suggested safe points:

```text
event end
tick boundary
before host patch apply
after host patch apply
explicit runtime.check_reload()
```

Avoid interrupting arbitrary instructions to replace function bodies.

### Top-Level Side Effects

Module top-level code may include:

```text
const
struct
enum
trait
fn
use
attribute
```

Disallow or strictly limit:

```text
register_event(...)
spawn_task(...)
open_file(...)
global_counter += 1
network call
random call
```

Event registration should happen through attributes and reflection scanning:

```rust
#[event("monster.kill")]
pub fn on_kill(ctx, player, monster) {
    // ...
}
```

### Hot Reload ABI Checks

Function changes allowed:

```text
function body changes
local variable changes
new private helper functions
new public functions
```

Function changes rejected:

```text
exported event function removes parameters
exported event function reorders parameters
effect permissions expand without host approval
return semantics are incompatible
```

Struct changes allowed:

```text
new field with default
field rename with unchanged FieldId
field order changes
new methods
```

Struct changes rejected or requiring migration:

```text
deleted field
FieldId reuse
incompatible field type hint
default value cannot be constructed
```

Enum changes allowed:

```text
new variant
variant rename with unchanged VariantId
new variant field with default
```

Enum changes requiring caution or rejection:

```text
deleted variant
changed existing variant field structure
VariantId reuse
```

## Standard Library

### Array

```rust
arr.len()
arr.is_empty()
arr.push(value)
arr.pop()
arr.map(|x| ...)
arr.filter(|x| ...)
arr.find(|x| ...)
arr.any(|x| ...)
arr.all(|x| ...)
arr.count(|x| ...)
arr.sum(|x| ...)
arr.group_by(|x| ...)
arr.sort_by(|x| ...)
```

Array methods should expose analysis-only signatures so LSP can infer lambda
parameter facts without adding script generics. For example, if `arr` has
`TypeFact::Array { element: E }`, then:

```text
arr.filter(|x| predicate) gives x: E and returns Array(element = E)
arr.map(|x| value) gives x: E and returns Array(element = TypeFact(value))
arr.find(|x| predicate) gives x: E and returns Option-like enum containing E
arr.sum(|x| value) gives x: E and returns int or float depending on value
```

### Map

```rust
map.len()
map.has(key)
map.get(key)
map.get_or(key, default)
map.set(key, value)
map.remove(key)
map.keys()
map.values()
map.entries()
map.map_values(|v| ...)
map.filter(|k, v| ...)
```

Map methods follow the same rule. If `map` has
`TypeFact::Map { key: K, value: V }`, `map.filter(|k, v| ...)` gives `k: K`,
`v: V`, and returns `Map(key = K, value = V)` as an internal fact only.

These analysis rules are not user-visible generic syntax. They are part of the
standard library metadata consumed by `vela_analysis` and future LSP tooling.

### Option And Result

```rust
enum Option {
    Some(value)
    None
}

enum Result {
    Ok(value)
    Err(error)
}
```

The `?` operator should support Option/Result-style propagation.

Use `Option` when absence is an ordinary script-visible branch, such as
collection lookup or search. Use `Result` when the caller needs a recoverable
failure reason. Runtime traps such as division by zero, type mismatch,
permission denial, budget exhaustion, and future explicit panic-style
operations should return VM diagnostics, not `Result.Err`.

### String

```rust
text.len()
text.is_empty()
text.contains(needle)
text.find(needle)
text.starts_with(prefix)
text.ends_with(suffix)
text.strip_prefix(prefix)
text.strip_suffix(suffix)
text.to_upper()
text.to_lower()
text.trim()
text.trim_start()
text.trim_end()
text.replace(old, new)
text.repeat(count)
text.slice(start, end)
text.char_at(index)
text.split(separator)
text.split_once(separator)
text.split_lines()
text.split_whitespace()
text.parse_int()
text.parse_float()
text.parse_bool()
```

### Math And Time

```text
math.max
math.min
math.clamp
math.lerp
math.move_towards
math.distance2d
math.distance3d
math.pow
math.sqrt
math.sign
math.floor
math.ceil
math.round
math.abs
math.random  # only with permission
```

Time should come from host context, not direct system time:

```rust
ctx.now
ctx.tick
ctx.elapsed_since(start)
```

## Embedding API

### Engine

```rust
let engine = Engine::builder()
    .with_standard_natives()
    .register_host_type::<Player>()
    .register_host_type::<Monster>()
    .register_host_type::<Inventory>()
    .register_reflect_schema::<RewardView>()
    .register_typed_native_fn::<(String,), _>(
        NativeFunctionDesc::new("game.log", NativeFunctionId::new(10_001))
            .param("message", TypeHint::String)
            .returns(TypeHint::Null)
            .effects(EffectSet::pure()),
        game_log,
    )
    .build()?;
```

### Compile

```rust
let program = engine.compile_dir("scripts")?;
let mut runtime = Runtime::new(engine, program);
```

### Call

```rust
let mut tx = PatchTx::new();

runtime.call(
    "combat.on_kill",
    &args![host(player), host(monster)],
    CallOptions::gameplay(),
    &mut state_adapter,
    &mut tx,
)?;

tx.apply(&mut state_adapter)?;
```

### Hot Reload

```rust
runtime
    .stage_hot_reload_update_file("scripts/combat.vela")?
    ?;

if let Some(report) = runtime.check_reload()? {
    if !report.accepted {
        log::error!("hot reload failed: {:#?}", report.errors);
    }
}
```

Runtime update compilation uses the runtime's active `ProgramVersion`, so hosts
do not need to separately fetch the current version before compiling an update.
Source load and path errors are returned immediately, while accepted updates and
ABI or policy rejections are staged until the host calls `runtime.check_reload()`
at a safe point. Hosts that already have a `PatchTx` can use
`runtime.apply_patch_tx_at_safe_point(tx, &mut state)` to check for a pending
reload before and after successful host patch apply.

For full module-root workflows, hosts can call
`runtime.stage_hot_reload_update_dir("scripts")` with the same safe-point
semantics. For file-watcher workflows, hosts may stage an update from a changed
`.vela` file inside a module root. The engine validates the changed path and
recompiles the full root so imports, module dependency impact, and ABI checks
are based on the same complete module graph as directory reloads.

Hot-reload ABI manifests copy optional declaration spans from reflected schema,
function, and method descriptors. When schema, function effect/access, or method
effect/access ABI checks reject an update, the rejected diagnostic points at the
new declaration span when it is known, and rendered report lines preserve that
span for editor/admin tooling.

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
  type: game.player.Player
  field: levle
  candidates: ["level"]
```

Copied reflection records for script-defined modules, functions, types, traits,
fields, methods, trait methods, and variants include `source_span: { source,
start, end }` when the registry knows the declaration location. Host-provided
descriptors may leave this field as `null`. Unknown reflection lookups carry
ranked related candidates with the same optional source spans where descriptors
have source locations, so admin/debug tooling can jump from a misspelled lookup
to nearby schema declarations without parsing human-readable messages.
Dynamic `reflect.get` and `reflect.set` calls on script record or enum values
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
  type: game.player.Player
  field: inventory
  reason: field is read-only
  hint: use player.inventory.add(...) instead
```

```text
HotReloadAbiMismatch:
  function: combat.on_kill
  old_params: [ctx, player, monster]
  new_params: [ctx, player]
  reason: exported event function cannot remove parameters
```

```text
StaleHostRef:
  type: game.player.Player
  object_id: 1024
  reason: generation mismatch
```

## IDE And LSP Readiness

A full LSP is not part of the MVP, but the core architecture must not make it
hard to add later. The required foundation is:

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
Player { level: 1 }    typed record or host-like constructor
{ "level": 1 }         map literal
{ level: 1 }           map literal with identifier key
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
PatchTx preview without applying host mutations
runtime exception and host error breakpoints
hot reload breakpoint rebinding across ProgramVersion changes
```

Debug operations must use the same safety boundaries as scripts:

```text
do not expose real Rust references
do not bypass PatchTx or ScriptStateAdapter for host mutation
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
host mutation flows through HostRef, HostPath, PathProxy, PatchTx, and ScriptStateAdapter only
```

Optimization rules:

```text
every optimized path has a VM-equivalent slow path
guards validate dynamic value tags, shapes, schemas, methods, fields, and ProgramVersion assumptions
guard failure is a normal slow-path transition, not a correctness failure
optimized code must charge or preserve ExecutionBudget behavior
optimized code must report or preserve GC roots before allocation, calls, and safe points
optimized code must preserve debugger-visible source locations, frame state, and safe suspension points
optimized code must not bypass PatchTx, reflection policy, permissions, or host access checks
hot reload invalidates version-owned caches and compiled code at safe points
dynamic type hints and TypeFacts guide optimization but are not correctness guarantees
```

The non-JIT performance target is intentionally part of the post-MVP roadmap:
an optimized bytecode interpreter should aim for Lua 5.x comparable performance
on representative gameplay workloads. LuaJIT and Node.js are useful reference
ceilings for hot scalar loops and future JIT work, but they are not the first
release target.

## Performance Roadmap

### Phase 1: Measurement And Baselines

```text
official microbenchmarks and gameplay-style benchmarks
release-mode benchmark parameters and checksum validation
VM scalar dispatch, function-call, heap, stdlib, record, string, and PatchTx cases
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
should be benchmark-driven and must not make host patching, hot reload,
reflection, or diagnostics less reliable.

Precompiled `.vbc` bytecode artifacts improve startup, deployment validation, and
reload/load latency. They do not by themselves improve the execution speed of
an already-loaded function, because that function already runs as bytecode.

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
PatchTx preview and host error breakpoints
Debug Adapter Protocol boundary for IDE integration
hot reload breakpoint rebinding through ProgramVersion metadata
```

Debugger support must stay disableable for normal gameplay execution. Optimized
interpreter paths, inline caches, and later JIT code must preserve the metadata
needed to reconstruct a bytecode-equivalent debug frame.

### Phase 5: Cranelift JIT

```text
baseline native compilation for restricted hot functions
tag, shape, schema, method, field, and version guards
side exits or deoptimization back to the bytecode VM
compiled frame root maps for GC, debugging, and deoptimization
budget checks in compiled code or side exits to checked VM helpers
host calls routed through existing NativeCallContext and PatchTx helpers
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

Default gameplay script settings:

```text
allow_io = false
allow_network = false
allow_random = false, or only through ctx.rng
allow_time_now = false; use ctx.now
host_write = only objects provided by the event context
reflect_write = disabled by default or tightly controlled
```

### Budgets

```text
instruction budget
memory budget
max call depth
max patch count
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
PatchTx tests
ABI diff tests
```

### Integration Tests

```text
script reads host field
script writes host field through PatchTx
reflect.set creates PatchTx
player.level += 1 creates Add patch
lambda parameter facts are inferred from array/map receiver facts
host schema fields are available through TypeRegistry
hot reload replaces function body
old call frame uses old version
new call frame uses new version
ABI mismatch rejects update
```

### Example Tests

```text
examples/game_server_demo
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
cargo run -p vela_cli -- examples/game_server_demo/scripts/level_up.vela
```

Later:

```bash
cargo bench --workspace
cargo fuzz run parser
```
