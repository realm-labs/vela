# Vela Primitive Types, Type Hints, and Runtime Guard Refactor Plan

> **Track:** clean architecture continuation  
> **Compatibility policy:** breaking changes are allowed. Do not preserve `int` / `float` compatibility.  
> **Primary goals:**  
> 1. Replace the old `int` / `float` model with explicit primitive scalar types such as `i64`, `u32`, `i8`, `f64`, `f32`.  
> 2. Add first-class binary data support through `bytes`.  
> 3. Define dynamic type-hint semantics precisely.  
> 4. Add guard metadata and runtime guard execution in a way that is interpreter-friendly now and JIT-friendly later.  
> 5. Give Codex a task-by-task implementation plan with checkboxes and termination conditions.

---

## 0. Current Repository Anchors

This plan is based on the current repository shape after the clean identity/linker refactor.

Current relevant architecture:

- `crates/vela_def/src/lib.rs`
  - Already owns `DefKind`, `DefPath`, and typed semantic IDs backed by `DefId(u128)`.
  - `DefId` is generated from canonical `DefPath` input with `blake3`.
- `crates/vela_registry/src/lib.rs`
  - Already has `DefinitionRegistry`, `Def`, typed definitions, `RegistryCompileView`, and `DebugNameTable`.
- `crates/vela_stdlib/src/manifest.rs`
  - Stdlib manifest currently registers `Null`, `Bool`, `Int`, `Float`, `String`, `Array`, `Map`, `Set`, `Function`, `Closure`, `Range`, `Option`, and `Result`.
  - It still uses string type hints such as `"int"`, `"float"`, `"any"`.
- `crates/vela_stdlib_runtime/src/lib.rs`
  - Maps manifest-derived stdlib function/method IDs to runtime implementation keys.
- `crates/vela_bytecode/src/linked.rs`
  - Already has dense handles: `NativeHandle`, `ScriptFunctionHandle`, `MethodDispatchHandle`, `TypeHandle`, `VariantHandle`, `FieldSlot`.
  - `LinkedProgram` already stores debug names in a side table.
  - Linked instructions already use handles for native/script/method/type/variant paths.
- `crates/vela_bytecode/src/lib.rs`
  - `Constant` currently has `Int(i64)` and `Float(f64)`.
  - `UnlinkedInstructionKind::CallNative` still carries `name` and `FunctionId`; linked bytecode lowers this to `NativeHandle`.
- `crates/vela_syntax/src/ast.rs`
  - `Literal` currently has `Int(String)`, `Float(String)`, `String(String)`, but no literal suffix representation and no byte literal.
- `crates/vela_syntax/src/lexer.rs`
  - Numeric lexing supports decimal, hex, and binary integer text, but not type suffixes such as `i8`, `u32`, `f32`.
  - String lexing supports normal strings, not byte strings.
- `crates/vela_bytecode/src/compiler/value_types.rs`
  - Value type flow still uses strings and returns `"int"` / `"float"` for literals.
- `crates/vela_vm/src/value.rs`
  - Runtime `Value` currently has `Int(i64)` and `Float(f64)`.
- `crates/vela_vm/src/owned_value.rs`
  - `OwnedValue` currently has `Int(i64)` and `Float(f64)`.
- `crates/vela_vm/src/heap.rs`
  - `HeapValue` has `String`, `Array`, `Map`, `Set`, `Record`, `Enum`, `Closure`, `Iterator`, `PathProxy`, but no `Bytes`.
- `crates/vela_vm/src/numeric_ops.rs`
  - Numeric ops currently only handle `Int + Int` and `Float + Float`.
- `crates/vela_host/src/value.rs`
  - `HostValue` currently has `Int(i64)` and `Float(f64)`.
  - Host write-through arithmetic still follows the old two-type numeric model.
- `crates/vela_engine/src/native.rs`
  - `TypeHint` currently has `Int` and `Float` enum variants.
  - Native and host method registration APIs still expose the old type names.
- `docs/architecture/language.md`
  - The detailed language architecture still lists `int` and `float` as
    supported value categories.
- `docs/architecture/runtime.md`
  - The detailed runtime architecture still documents `Value::Int` and
    `Value::Float`.

---

## 1. Design Decisions

### 1.1 Remove `int` and `float`

The old names are removed:

```text
int    removed
float  removed
```

Use explicit primitive names instead:

```text
bool
i8 i16 i32 i64
u8 u16 u32 u64
f32 f64
string
bytes
null
```

Default literal behavior:

```text
12       defaults to i64 when it escapes without a more specific expected type
12.0     defaults to f64 when it escapes without a more specific expected type
12i8     is exactly i8
12u32    is exactly u32
12.0f32  is exactly f32
0xff     defaults to i64 unless context-typed
0xffu8   is exactly u8
b"abc"   is bytes
```

No `int` / `float` aliases in the first implementation. If aliases are ever desired later, add them as explicit `TypeAliasDef` entries, not as hidden language rules.

Source `any` remains legal as explicit erased dynamic metadata:

```text
no hint  no contract
any      explicit no contract / erased dynamic
```

Both forms avoid contract guards. Passing an `any` or unhinted value into a
more specific contract site still requires the callee or write site to guard.

### 1.2 Type hints are contracts, not conversions

Core rule:

```text
Type hints never convert values.
Type hints establish runtime contracts.
The compiler may reject statically known contract violations early.
```

Examples:

```vela
fn f(x: i64) {}

f(12);      // OK: unsuffixed integer literal is context-typed as i64
f(12i64);   // OK
f(12i8);    // compile error: statically known i8 cannot satisfy i64
f(12.0);    // compile error: statically known f64 cannot satisfy i64
f("12");    // compile error: statically known string cannot satisfy i64
```

Dynamic unknown values are allowed but guarded:

```vela
fn f(x: i64) {}

fn g(value) {
    f(value); // compile OK, emits/uses runtime guard at f's checked entry
}
```

If `value` is not actually `i64` at runtime, the call fails with a runtime type contract error.

### 1.3 Static mismatch vs dynamic guard

Use this decision table everywhere a type hint applies.

| Expression static classification | Expected type | Result |
|---|---:|---|
| exact same type | `T` | OK, no runtime guard |
| exact different type | `T` | compile error |
| unsuffixed integer literal | integer type and value fits | OK, contextualize literal |
| unsuffixed integer literal | float or non-numeric type | compile error |
| unsuffixed float literal | `f32`/`f64` and value fits | OK, contextualize literal |
| unsuffixed float literal | integer or non-float type | compile error |
| suffixed literal | same type | OK |
| suffixed literal | different type | compile error |
| dynamic / unhinted value | `T` | compile OK + runtime guard |
| `any` / erased dynamic value | `T` | compile OK + runtime guard |

### 1.4 Numeric operations require identical concrete numeric types

No implicit numeric conversions.

```vela
1i32 + 2i32    // OK -> i32
1i32 + 2i64    // compile error if statically known
a + b          // runtime error if a and b resolve to different concrete numeric types
1i64 + 2.0     // compile error if statically known
```

The result type is the same as the operand type.

### 1.5 Deferred unsuffixed literals in dynamic operator contexts

This is required to keep dynamic numeric code usable.

Problem:

```vela
fn inc(x) {
    return x + 1;
}
```

If `1` is immediately defaulted to `i64`, then `inc(1i8)` fails as `i8 + i64`.

Decision:

```text
An inline unsuffixed numeric literal inside a dynamic numeric operator can be deferred.
It is not lowered to i64/f64 until a real concrete context exists.
```

Runtime behavior:

```vela
fn inc(x) {
    return x + 1;
}

inc(1i8);   // OK: deferred integer literal 1 becomes i8
inc(1u32);  // OK: deferred integer literal 1 becomes u32
inc(1i64);  // OK: deferred integer literal 1 becomes i64
inc(1.0);   // runtime error: integer literal does not become float
```

Bound literals are not deferred:

```vela
fn inc_strict(x) {
    let one = 1;      // one defaults to i64
    return x + one;
}

inc_strict(1i8);     // runtime error: i8 + i64
```

Static typed parameter context still contextualizes at compile time:

```vela
fn inc_i8(x: i8) {
    return x + 1;    // 1 is compiled as i8
}
```

### 1.6 Guard kinds

There are two different guard categories.

```rust
enum GuardKind {
    Contract,
    Specialization,
}
```

#### Contract guard

Source of truth for language semantics:

- function parameter type hints;
- return type hints;
- typed let binding;
- typed global write;
- typed record/enum field construction;
- later writes to typed record/enum fields;
- host/native boundary contracts.

Failure means runtime error:

```text
VmErrorKind::TypeContractViolation
```

#### Specialization guard

Implementation-level performance assumption:

- inline cache observed type;
- JIT speculation;
- hot dynamic operator specialization.

Failure means fallback or deopt, not a language error.

```text
guard failure -> generic interpreter/stub/deopt
```

### 1.7 Guard plans must be linked and JIT-friendly

Do not guard by string name or registry lookup.

Bad:

```text
expected_type_name == "i64"
registry.lookup(type_id)
```

Good:

```rust
enum PrimitiveTag {
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
}

enum TypeGuardPlan {
    Primitive(PrimitiveTag),
    Type(TypeHandle),
    Variant(VariantHandle),
    Shape {
        ty: TypeHandle,
        shape_id: ShapeId,
    },
    HostType(TypeHandle),
}
```

This lets the interpreter execute a guard as a small tag/handle comparison and lets a future JIT lower it to compare-and-branch.

### 1.8 Checked and unchecked function entries

Each function with contracts should conceptually support:

```text
checked entry:
    validates parameter guards
    jumps into body

unchecked entry:
    assumes caller already proved parameter contracts
```

Implementation can be metadata-only at first, but the representation must support it later.

Call lowering policy:

| Call site | Entry |
|---|---|
| statically proven argument types satisfy callee hints | unchecked |
| dynamic/unhinted values may reach callee | checked |
| reflection/host/C API/dynamic function value call | checked |
| JIT inlined proven call | unchecked body |

---

## 2. Target Runtime Type Model

### 2.1 Scalar representation

Replace `Value::Int(i64)` and `Value::Float(f64)` with a scalar enum. The
primitive/scalar tag types should live in a dependency-light shared crate
(`vela_common` or an equivalent low-level crate), not only in `vela_vm`, because
bytecode constants, host values, owned values, C ABI tags, serde conversion,
registry metadata, diagnostics, and guard plans all need the same vocabulary.

```rust
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ScalarValue {
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    F32(f32),
    F64(f64),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum NumericTag {
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
}
```

Then:

```rust
pub enum Value {
    Missing,
    Null,
    Bool(bool),
    Scalar(ScalarValue),
    Range(RangeValue),
    HeapRef(GcRef),
    HostRef(HostRef),
}
```

Mirror the same in:

```rust
OwnedValue::Scalar(ScalarValue)
HostValue::Scalar(ScalarValue)
Constant::Scalar(ScalarValue)
```

Embedding type hints should also move away from old enum variants:

```rust
TypeHint::Primitive(PrimitiveTag::I64)
TypeHint::Primitive(PrimitiveTag::F64)
```

Convenience constructors such as `TypeHint::i64()` are fine, but do not keep
`TypeHint::Int` or `TypeHint::Float` compatibility variants.

### 2.2 Bytes representation

`bytes` is first-class but heap-backed.

```rust
OwnedValue::Bytes(Vec<u8>)
Constant::Bytes(Vec<u8>)
HeapValue::Bytes(Vec<u8>)
```

`Value` stores bytes via `HeapRef`, like string/array/map.

`bytes[index]` returns `u8`.

### 2.3 Primitive type registry

Add primitive metadata to the registry type definition.

Suggested shape:

```rust
pub enum PrimitiveKind {
    Null,
    Bool,
    SignedInt { bits: u8 },
    UnsignedInt { bits: u8 },
    Float { bits: u8 },
    String,
    Bytes,
}
```

`TypeDef` should expose:

```rust
primitive: Option<PrimitiveKind>
```

or equivalent.

Stdlib manifest should register:

```text
Null
Bool
I8 I16 I32 I64
U8 U16 U32 U64
F32 F64
String
Bytes
Array Map Set Function Closure Range Option Result
```

Canonical names in source should be lowercase:

```text
null bool i8 i16 i32 i64 u8 u16 u32 u64 f32 f64 string bytes
```

The manifest may internally store display names however the project prefers, but type hints in source should resolve lowercase primitive names.

---

## 3. Literal Semantics

### 3.1 Literal forms

Add support for:

```text
12
12i8
12i16
12i32
12i64
12u8
12u16
12u32
12u64

0xff
0xffu8
0b1010u16

12.0
12.0f32
12.0f64

"abc"
b"abc"
b"\x00\xff"
```

### 3.2 Literal AST

Replace:

```rust
Literal::Int(String)
Literal::Float(String)
```

with structured literals.

Example:

```rust
pub enum Literal {
    Null,
    Bool(bool),
    Integer(IntegerLiteral),
    Float(FloatLiteral),
    String(String),
    Bytes(Vec<u8>),
}

pub struct IntegerLiteral {
    pub text: String,
    pub radix: IntRadix,
    pub suffix: Option<IntegerSuffix>,
}

pub enum IntegerSuffix {
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
}

pub struct FloatLiteral {
    pub text: String,
    pub suffix: Option<FloatSuffix>,
}

pub enum FloatSuffix {
    F32,
    F64,
}
```

### 3.3 Literal errors

Compile-time errors:

- invalid suffix;
- integer literal too large for suffixed type;
- integer literal too large for contextual expected type;
- signed-min literals such as `-128i8` and `-9223372036854775808i64`
  must be accepted through unary-minus-aware constant evaluation rather than
  rejecting the positive magnitude before unary lowering;
- float literal not representable for suffixed/contextual type if the parser/compiler chooses to enforce representability;
- byte string invalid escape;
- `b"..."` contains Unicode escape or non-byte scalar if not explicitly supported.

Recommended byte string escapes:

```text
\n \r \t \0 \" \\ \xNN
```

Do not allow `\u{...}` in byte strings initially.

---

## 4. Type Hint and Guard Semantics

### 4.1 Contract locations

Type hints create contracts in these places:

- function parameters;
- function returns;
- lambda parameters if type hints exist;
- typed `let`;
- typed `global`;
- record/struct fields;
- enum payload fields;
- later assignment to typed record/struct fields;
- later assignment to typed enum payload fields when mutation is supported;
- host/native function parameters and returns;
- C API/serde decode boundaries where a typed target is requested.

### 4.2 Compile-time known mismatch

If the compiler can prove the expression has an exact type different from the expected hint, emit a compile error.

Examples:

```vela
fn f(x: i64) {}

f("x");     // compile error
f(1.0);     // compile error
f(1i8);     // compile error
```

This applies consistently to all types, not just primitives:

```vela
fn f(x: Header) {}

f("x");     // compile error
```

### 4.3 Dynamic unknown values

If the expression is dynamic/unhinted, emit a runtime guard.

```vela
fn f(x: i64) {}

fn g(v) {
    f(v);   // runtime guard at f checked entry
}
```

### 4.4 Guard elimination

No guard should be emitted when the compiler has already proven the value satisfies the contract.

Examples:

```vela
let x: i64 = 12;       // no runtime guard
let y: i64 = 12i64;    // no runtime guard
```

Guards should be eliminated when:

- literal contextualization proves the type;
- a value comes from a previously guarded binding;
- a value comes from a typed parameter inside its body;
- a typed record field is read from an object whose construction/write invariants are guaranteed;
- the call site is statically proven and uses unchecked entry.

### 4.5 Runtime guard error

Add a dedicated VM error.

Suggested shape:

```rust
pub enum VmErrorKind {
    TypeContractViolation {
        expected: String,      // debug name; or DebugNameId if available in context
        actual: String,        // runtime type display
        context: TypeContractContext,
    },
    ...
}

pub enum TypeContractContext {
    FunctionParam {
        function: String,
        param: String,
    },
    ReturnValue {
        function: String,
    },
    LocalBinding {
        name: String,
    },
    Global {
        name: String,
    },
    Field {
        owner: String,
        field: String,
    },
    NativeParam {
        function: String,
        param: String,
    },
}
```

Implementation may internally use `DebugNameId` and resolve to strings at diagnostic creation time.

---

## 5. Bytecode and Linked Representation

### 5.1 Add type guard metadata

Prefer metadata for function parameter contracts:

```rust
pub struct LinkedParam {
    pub name: DebugNameId,
    pub guard: Option<TypeGuardPlan>,
}

pub struct LinkedCodeObject {
    pub params: Vec<LinkedParam>,
    pub return_guard: Option<TypeGuardPlan>,
    ...
}
```

If changing `params: Vec<DebugNameId>` is too large for one task, introduce parallel metadata first:

```rust
pub param_guards: Vec<Option<TypeGuardPlan>>,
pub return_guard: Option<TypeGuardPlan>,
```

Then clean it later.

### 5.2 Add guard instructions for non-boundary checks

Use explicit instructions for local/global/field dynamic guards:

```rust
InstructionKind::GuardType {
    value: Register,
    expected: TypeGuardPlan,
    context: GuardContext,
}
```

or a more specialized family:

```rust
GuardLocal
GuardGlobal
GuardField
```

Keep it simple first: one `GuardType` with context metadata is enough.

### 5.3 Deferred literal instructions

Add dynamic literal operator instructions rather than materializing default `i64/f64` constants.

Suggested unlinked/linked instructions:

```rust
BinaryIntLiteral {
    op: NumericBinaryOp,
    dst: Register,
    value: Register,
    literal: IntegerLiteralValue,
    literal_side: LiteralSide,
}

BinaryFloatLiteral {
    op: NumericBinaryOp,
    dst: Register,
    value: Register,
    literal: FloatLiteralValue,
    literal_side: LiteralSide,
}
```

Where:

```rust
enum LiteralSide {
    Left,
    Right,
}

enum NumericBinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
}
```

For commutative operations this is straightforward. For `Sub`, `Div`, `Rem`, and comparisons, side matters.

### 5.4 Compile-time literal contextualization

For known operand type:

```vela
fn inc_i8(x: i8) {
    return x + 1;
}
```

Compile `1` as `i8` and emit ordinary typed/dynamic binary op, not deferred literal.

For dynamic operand type:

```vela
fn inc(x) {
    return x + 1;
}
```

Emit `BinaryIntLiteral`.

### 5.5 Future typed op lowering

Do not require typed opcodes in the first implementation, but keep the representation ready for them.

Possible future instructions:

```rust
AddI64
AddU32
AddF64
LessI32
...
```

Initial implementation can keep generic `Add/Sub/...` and use scalar tag dispatch.

---

## 6. Numeric Runtime Semantics

### 6.1 Exact type matching

Generic numeric operations:

```text
same concrete scalar type -> execute
different concrete scalar types -> type mismatch runtime error
non-numeric value -> type mismatch runtime error
```

### 6.2 Overflow

Default arithmetic should be checked.

```text
i8 + i8 overflow -> runtime error
u8 - u8 underflow -> runtime error
constant-folded overflow -> compile error
```

Add explicit wrapping APIs later or in stdlib phase:

```text
i8::wrapping_add
u32::wrapping_mul
u64::rotate_left
```

### 6.3 Float behavior

Recommended:

- `f32` ops produce `f32`;
- `f64` ops produce `f64`;
- no implicit `f32` <-> `f64`;
- integer and float do not mix;
- division by zero behavior should be explicit:
  - current VM treats float zero division as `DivisionByZero`;
  - keep that behavior unless deliberately changed.

### 6.4 Bitwise operators

Because binary work needs integer bit manipulation, add bitwise syntax and ops either in this refactor or immediately after:

```text
& | ^ ~ << >>
```

Rules:

- only integer scalar types;
- operands must have identical integer type, except shift amount policy must be explicitly chosen;
- recommended shift policy:
  - left operand determines result type;
  - shift amount must be unsigned integer or same type; choose one and document;
  - shift count out of range -> runtime error;
  - wrapping shifts only through explicit stdlib methods.

If syntax is too much for first phase, provide stdlib methods first:

```text
u32::bit_and(a, b)
u32::shift_left(a, amount)
```

---

## 7. Bytes Semantics

### 7.1 `bytes` type

`bytes` is immutable.

```vela
let magic: bytes = b"\x89PNG\r\n\x1a\n";
let first: u8 = magic[0];
```

### 7.2 First bytes API

Minimum stdlib methods:

```text
bytes.len() -> i64
bytes.is_empty() -> bool
bytes.slice(start: i64, end: i64) -> bytes
bytes.get(index: i64) -> u8

bytes.read_i8(offset: i64) -> i8
bytes.read_u8(offset: i64) -> u8

bytes.read_i16_le(offset: i64) -> i16
bytes.read_u16_le(offset: i64) -> u16
bytes.read_i32_le(offset: i64) -> i32
bytes.read_u32_le(offset: i64) -> u32
bytes.read_i64_le(offset: i64) -> i64
bytes.read_u64_le(offset: i64) -> u64
bytes.read_f32_le(offset: i64) -> f32
bytes.read_f64_le(offset: i64) -> f64

bytes.read_i16_be(offset: i64) -> i16
bytes.read_u16_be(offset: i64) -> u16
bytes.read_i32_be(offset: i64) -> i32
bytes.read_u32_be(offset: i64) -> u32
bytes.read_i64_be(offset: i64) -> i64
bytes.read_u64_be(offset: i64) -> u64
bytes.read_f32_be(offset: i64) -> f32
bytes.read_f64_be(offset: i64) -> f64

bytes.to_hex() -> string
bytes::from_hex(text: string) -> Result
```

Use explicit endianness. Never use host machine endianness implicitly.

Index and count policy:

```text
bytes[index]      index is i64
bytes.len()       returns i64
bytes.slice(a,b)  offsets are i64
negative index    runtime error
out of bounds     runtime error or Option/Result only for explicit get/read APIs
```

Keep bytes, array, string, and range indexing/count APIs aligned on `i64` in
this refactor. If unsigned indexes are desired later, add explicit conversion
or overload-free helper APIs rather than making indexing polymorphic.

### 7.3 Mutable bytes

Do not include mutable `byte_buffer` in the first primitive refactor unless needed.

Future:

```text
byte_buffer::new()
buffer.push_u8(value)
buffer.write_u32_le(offset, value)
buffer.freeze() -> bytes
```

---

## 8. Codex Task Plan

Each task below is meant to be executable independently or as a small PR. The
checkboxes are intentionally explicit. The order is chosen to avoid
compatibility shims: remove the old shared value and type-hint vocabulary first,
then build registry, syntax, guards, and runtime behavior on the new model.

---

### Phase 0: Update the active architecture contract

#### Task 0.1: Add primitive/type-hint architecture document and wire it in

- [ ] Add `docs/architecture/primitives-type-hints-and-guards.md`.
- [ ] Include the decisions from Sections 1-7 of this document.
- [ ] State clearly that `int` and `float` are removed.
- [ ] State clearly that type hints are contracts, not conversions.
- [ ] State clearly that static known mismatches are compile errors and dynamic unknown mismatches are runtime guard errors.
- [ ] State clearly that `any` is explicit erased dynamic metadata and creates no contract by itself.
- [ ] Add examples for `f(12)`, `f(12i8)`, `f(12.0)`, `f(x)`.
- [ ] Add examples for `fn inc(x) { x + 1 }`.
- [ ] Reference the new document from `docs/architecture.md`.
- [ ] Update `docs/architecture/language.md` so supported value categories use `i64`/`f64` defaults and explicit primitive scalar names, not `int`/`float`.
- [ ] Update `docs/architecture/runtime.md` so the documented value layout uses `ScalarValue`.
- [ ] Update `docs/architecture/stdlib-and-embedding.md` examples that mention `int`, `float`, `parse_int`, or `parse_float`.

**Termination condition:**

- [ ] The document exists and is referenced from current architecture docs.
- [ ] No active architecture page contradicts the `int`/`float` removal.
- [ ] No implementation changes are required in this phase.

---

### Phase 1: Shared primitive and value model

#### Task 1.1: Add shared primitive/scalar types

- [x] Add `PrimitiveTag`, `NumericTag`, and `ScalarValue` to `vela_common` or another dependency-light shared crate.
- [x] Include primitive tags for:
  - [x] `Null`
  - [x] `Bool`
  - [x] `I8/I16/I32/I64`
  - [x] `U8/U16/U32/U64`
  - [x] `F32/F64`
  - [x] `String`
  - [x] `Bytes`
- [x] Add helpers:
  - [x] `ScalarValue::numeric_tag()`
  - [x] `ScalarValue::primitive_tag()`
  - [x] display/debug names for diagnostics.

**Termination condition:**

- [x] Shared tags compile without depending on VM, host, registry, or engine crates.
- [x] Unit tests cover scalar tags and display names for all primitive scalar types.

#### Task 1.2: Replace runtime and boundary numeric variants together

- [x] Replace `Value::Int` and `Value::Float` with `Value::Scalar`.
- [x] Replace `OwnedValue::Int` and `OwnedValue::Float` with `OwnedValue::Scalar`.
- [x] Replace `HostValue::Int` and `HostValue::Float` with `HostValue::Scalar`.
- [x] Replace `Constant::Int` and `Constant::Float` with `Constant::Scalar`.
- [x] Add `From<i8/i16/i32/i64/u8/u16/u32/u64/f32/f64>` implementations where boundary ergonomics need them.
- [x] Keep ergonomic test constructors if useful, but name them after explicit types (`i64_value`, `f64_value`) rather than old generic `int`/`float`.
- [x] Update owned-value macros and host test helpers.

**Termination condition:**

- [x] VM, host, bytecode, and engine crates compile against the scalar representation.
- [x] Existing behavior is ported to explicit `I64`/`F64` defaults.
- [x] No public value enum keeps `Int` or `Float` compatibility variants.

#### Task 1.3: Add heap bytes after scalar boundaries compile

- [x] Add `OwnedValue::Bytes(Vec<u8>)`.
- [x] Add `Constant::Bytes(Vec<u8>)`.
- [x] Add `HeapValue::Bytes(Vec<u8>)`.
- [x] Add tracing behavior: bytes contain no heap refs.
- [x] Add heap size accounting.
- [x] Add owned/runtime conversion for bytes.
- [x] Add equality/hash behavior if relevant.

**Termination condition:**

- [x] `OwnedValue::Bytes` converts to runtime `Value` and back.
- [x] `LoadConst` can load bytes.
- [x] GC tests include bytes and bytes do not trace false refs.

---

### Phase 2: Registry, stdlib definitions, and embedding hints

#### Task 2.1: Add primitive metadata to registry type definitions

- [x] Add `PrimitiveKind` to `vela_registry` or reuse the shared primitive tag model if it is sufficient.
- [x] Add `primitive: Option<PrimitiveKind>` to `TypeDef` or equivalent.
- [x] Add constructors/helpers for primitive type definitions.
- [x] Add registry query:
  - [x] `type_primitive_kind(TypeId) -> Option<PrimitiveKind>`
  - [x] `primitive_type_id(PrimitiveKind) -> Option<TypeId>` or equivalent.
- [x] Add duplicate checks so primitive type names cannot collide.

**Termination condition:**

- [x] Unit tests can register and query `i64`, `u8`, `f32`, `bytes`.
- [x] Registry duplicate path/key tests still pass.

#### Task 2.2: Replace stdlib `Int` / `Float` types

- [x] Update `crates/vela_stdlib/src/manifest.rs`.
- [x] Remove `StdTypeSpec::new("Int")`.
- [x] Remove `StdTypeSpec::new("Float")`.
- [x] Add:
  - [x] `Null`
  - [x] `Bool`
  - [x] `I8`
  - [x] `I16`
  - [x] `I32`
  - [x] `I64`
  - [x] `U8`
  - [x] `U16`
  - [x] `U32`
  - [x] `U64`
  - [x] `F32`
  - [x] `F64`
  - [x] `String`
  - [x] `Bytes`
- [x] Decide and implement source names:
  - [x] source hint `i64` resolves to stdlib type `I64`;
  - [x] source hint `f64` resolves to stdlib type `F64`;
  - [x] source hint `bytes` resolves to stdlib type `Bytes`.

**Termination condition:**

- [x] `standard_registry()` resolves all primitive types.
- [x] `int` and `float` no longer resolve as valid stdlib types.
- [x] Stdlib registration tests pass after updates.

#### Task 2.3: Replace embedding `TypeHint::Int` / `TypeHint::Float`

- [x] Replace `TypeHint::Int` and `TypeHint::Float` with `TypeHint::Primitive(PrimitiveTag)`.
- [x] Add convenience constructors such as `TypeHint::i64()` and `TypeHint::f64()` if they improve call sites.
- [x] Update native function, host method, macro, reflection, and validation tests to use explicit primitive hints.
- [x] Ensure descriptor serialization and reflection metadata expose canonical lowercase names.
- [x] Do not keep old enum variants, aliases, or hidden conversions.

**Termination condition:**

- [x] Public embedding APIs can declare every primitive scalar and `bytes`.
- [x] `TypeHint::Int` and `TypeHint::Float` no longer exist.
- [x] Native/host descriptor tests pass with explicit primitive names.

#### Task 2.4: Update stdlib signatures

- [x] Replace `"int"` return/param hints with `"i64"` where default integer semantics are intended.
- [x] Replace `"float"` return/param hints with `"f64"` where default float semantics are intended.
- [x] Keep `"any"` only where explicit erased dynamic metadata is intended.
- [x] Leave `bytes` method specs for Phase 7.2; this phase did not add bytes stdlib APIs.

**Termination condition:**

- [x] No stdlib manifest param or return type uses `"int"` or `"float"`.
- [x] Existing stdlib tests pass after expected updates.

---

### Phase 3: Syntax and literal parsing

#### Task 3.1: Update AST literal model

- [x] Replace `Literal::Int(String)` with `Literal::Integer(IntegerLiteral)`.
- [x] Replace `Literal::Float(String)` with `Literal::Float(FloatLiteral)`.
- [x] Add `Literal::Bytes(Vec<u8>)` or `Literal::Bytes(ByteLiteral)`.
- [x] Add suffix enums:
  - [x] `IntegerSuffix`
  - [x] `FloatSuffix`
- [x] Update pattern literals accordingly.

**Termination condition:**

- [x] Parser tests compile after AST updates.
- [x] Existing integer/float/string literal tests are updated.

#### Task 3.2: Extend lexer for numeric suffixes

- [x] Lex `12i8`, `12i16`, `12i32`, `12i64`.
- [x] Lex `12u8`, `12u16`, `12u32`, `12u64`.
- [x] Lex `12.0f32`, `12.0f64`.
- [x] Preserve radix forms:
  - [x] `0xffu8`
  - [x] `0b1010u16`
- [x] Reject invalid suffixes with lexer diagnostics.
- [x] Ensure suffix text is not tokenized as a following identifier.

**Termination condition:**

- [x] Lexer tests cover every suffix.
- [x] Lexer rejects `12i128`, `12usize`, `12abc`, `12.0i32`.

#### Task 3.3: Add signed-min and contextual literal validation

- [x] Ensure `-128i8`, `-32768i16`, `-2147483648i32`, and `-9223372036854775808i64` are accepted through unary-minus-aware constant evaluation.
- [x] Reject positive suffixed literals that exceed their type.
- [ ] Reject contextual unsuffixed integer literals that do not fit the expected type.
- [ ] Reject contextual unsuffixed float literals that do not satisfy the expected float policy.

**Termination condition:**

- [x] Signed-min tests pass for every signed integer primitive.
- [ ] Out-of-range literal tests report compile diagnostics with source spans.

#### Task 3.4: Add byte string lexing

- [x] Lex `b"..."` as byte string, not identifier `b` followed by string.
- [ ] Support escapes:
  - [x] `\n`
  - [x] `\r`
  - [x] `\t`
  - [x] `\0`
  - [x] `\"`
  - [x] `\\`
  - [x] `\xNN`
- [x] Reject invalid byte escapes.
- [x] Reject Unicode escapes in byte strings unless explicitly supported.

**Termination condition:**

- [x] Lexer/parser tests cover valid and invalid byte strings.
- [x] `b"\xff"` produces bytes `[255]`.

---

### Phase 4: Compiler type facts and expected-type checking

#### Task 4.1: Replace string value type facts

- [x] Replace `ValueTypeFlow` string type names with a typed representation:
  - [x] `RuntimeTypeFact`
  - [x] `PrimitiveTag`
  - [ ] `TypeId`
  - [x] or equivalent.
- [x] Remove `"int"` and `"float"` from `type_hint_value_type`.
- [x] Add primitive type resolution through `RegistryCompileView`.
- [x] Preserve dynamic/unknown state explicitly.

**Termination condition:**

- [x] Value type flow can represent exact `i8`, `i64`, `u32`, `f32`, `f64`, `string`, `bytes`.
- [x] No code path relies on `"int"` or `"float"` strings.

#### Task 4.2: Add static expression classification

- [ ] Implement:
  - [ ] `StaticExprType::Exact(TypeRef)`
  - [ ] `StaticExprType::UnsuffixedIntegerLiteral`
  - [ ] `StaticExprType::UnsuffixedFloatLiteral`
  - [ ] `StaticExprType::Dynamic`
- [ ] Classify literals without prematurely defaulting them.
- [ ] Classify unhinted params as dynamic.
- [ ] Classify hinted params as exact after guard.
- [ ] Classify local bindings based on known type facts.

**Termination condition:**

- [ ] Tests cover literal classification and unhinted param dynamic classification.

#### Task 4.3: Add expected-type check API

- [ ] Implement a single API used by:
  - [ ] function calls;
  - [ ] typed let;
  - [ ] return values;
  - [ ] record field construction;
  - [ ] later record field writes;
  - [ ] enum payload construction;
  - [ ] later enum payload writes when supported;
  - [ ] global writes.
- [ ] API behavior:
  - [ ] exact same type -> OK, no guard;
  - [ ] exact mismatch -> compile error;
  - [ ] compatible unsuffixed literal -> contextualize;
  - [ ] dynamic -> emit guard;
  - [ ] suffix mismatch -> compile error.

**Termination condition:**

- [ ] `fn f(x: i64) {}` with `f(12)` passes.
- [ ] `f(12i8)`, `f(12.0)`, `f("12")` fail at compile time.
- [ ] `fn g(x) { f(x); }` compiles and emits a guard path.

#### Task 4.4: Add compile errors for static type contract violations

- [ ] Add `CompileErrorKind::TypeContractMismatch` or diagnostics equivalent.
- [ ] Include:
  - [ ] expected type;
  - [ ] actual type;
  - [ ] source span;
  - [ ] context.
- [ ] Add literal out-of-range errors for contextual/suffixed literals.

**Termination condition:**

- [ ] Error messages are clear enough for tests.
- [ ] Compile-time mismatch tests assert error kind.

---

### Phase 5: Bytecode guard representation

#### Task 5.1: Add guard plan types

- [ ] Add `PrimitiveTag`.
- [ ] Add `TypeGuardPlan`.
- [ ] Add `GuardContext`.
- [ ] Make guard plans linked, not registry/string based.
- [ ] Ensure guard plans can be verified.

**Termination condition:**

- [ ] Linked verifier rejects invalid type/variant handles in guards.
- [ ] Guard plan debug names are available for diagnostics.

#### Task 5.2: Add parameter and return guard metadata

- [ ] Add param guard metadata to `UnlinkedCodeObject`.
- [ ] Link param guard metadata into `LinkedCodeObject`.
- [ ] Add return guard metadata.
- [ ] Keep default/capture layout unchanged unless necessary.

**Termination condition:**

- [ ] A function with `x: i64` has a linked parameter guard.
- [ ] A function with `-> i64` has a linked return guard.
- [ ] A function with no hints has no guards.

#### Task 5.3: Add guard instruction for local/global/field contracts

- [ ] Add unlinked guard instruction or compiler marker.
- [ ] Link to `InstructionKind::GuardType`.
- [ ] VM can execute it.
- [ ] Compiler emits it only for dynamic unknown values.

**Termination condition:**

- [ ] `let x: i64 = dynamic_value;` emits a guard.
- [ ] `typed_record.amount = dynamic_value;` emits a guard.
- [ ] `let x: i64 = 12;` emits no guard.
- [ ] `let x: i64 = "x";` compile-errors.

#### Task 5.4: Add deferred literal operator instructions

- [ ] Add `BinaryIntLiteral` and `BinaryFloatLiteral` instruction forms.
- [ ] Include literal side for non-commutative operations.
- [ ] Include precomputed fit information for integer literals if useful.
- [ ] Update verifier.

**Termination condition:**

- [ ] `fn inc(x) { x + 1 }` compiles to a deferred int literal op.
- [ ] `fn inc_i8(x: i8) { x + 1 }` compiles with literal contextualized to i8, not deferred.
- [ ] Bound `let one = 1; x + one` uses concrete `i64`.

---

### Phase 6: VM execution

#### Task 6.1: Execute contract guards

- [ ] Add runtime type extraction:
  - [ ] primitive tag;
  - [ ] type handle for records/enums if available;
  - [ ] bytes/string tags.
- [ ] Execute parameter guards at checked entry.
- [ ] Execute return guards before returning from a function.
- [ ] Execute `GuardType` instruction for locals/globals/fields.
- [ ] Add `VmErrorKind::TypeContractViolation`.

**Termination condition:**

- [ ] Dynamic mismatch at function entry produces runtime contract error.
- [ ] Dynamic mismatch at return produces runtime contract error.
- [ ] Static mismatch remains compile error.

#### Task 6.2: Add checked/unchecked entry support

- [ ] Add call metadata or instruction variants to distinguish:
  - [ ] checked script call;
  - [ ] unchecked script call.
- [ ] Static known-safe calls use unchecked path.
- [ ] Dynamic calls use checked path.
- [ ] Reflection/host/C API calls use checked path.

**Termination condition:**

- [ ] Tests prove statically safe call does not execute parameter guard.
- [ ] Tests prove dynamic call does execute parameter guard.

#### Task 6.3: Rewrite numeric operations around `ScalarValue`

- [ ] Replace `numeric_ops.rs` with scalar-aware ops.
- [ ] Implement:
  - [ ] add/sub/mul/div/rem;
  - [ ] negation for signed ints/floats;
  - [ ] comparisons;
  - [ ] equality.
- [ ] Enforce exact matching numeric tags.
- [ ] Enforce checked overflow.
- [ ] Preserve division-by-zero behavior.

**Termination condition:**

- [ ] All scalar types have arithmetic tests.
- [ ] Mixed numeric types fail.
- [ ] Overflow tests pass.

#### Task 6.4: Execute deferred literal operators

- [ ] Runtime contextualizes deferred int literal from the other operand tag.
- [ ] Runtime checks literal fits target tag.
- [ ] Runtime does not convert integer literals to float.
- [ ] Runtime contextualizes deferred float literal only for `f32`/`f64`.
- [ ] Runtime does not convert float literals to integer.

**Termination condition:**

- [ ] `inc(1i8)` returns `2i8`.
- [ ] `inc(1u32)` returns `2u32`.
- [ ] `inc(1i64)` returns `2i64`.
- [ ] `inc(1.0)` fails for `x + 1`.
- [ ] `inc_float(1.0f32)` works for `x + 1.0`.

### Phase 7: Bytes vertical slice

Treat bytes as a separate vertical slice if the scalar/guard migration is too
large for one checkpoint. Do not block scalar primitives on the full bytes API
unless the implementation stays small.

#### Task 7.1: Implement bytes runtime behavior

- [ ] Load bytes constants into heap.
- [ ] Convert bytes owned/runtime values.
- [ ] Implement bytes indexing to `u8`.
- [ ] Implement bytes stdlib methods.
- [ ] Keep bytes immutable.
- [ ] Use `i64` indexes/counts for bytes APIs to match array/string/range conventions.

**Termination condition:**

- [ ] `b"abc"[0]` returns `97u8`.
- [ ] Negative and out-of-bounds indexes produce the chosen runtime error.
- [ ] `read_u32_le` and `read_u32_be` tests pass.

#### Task 7.2: Add bytes stdlib and conversion APIs

- [ ] Add:
  - [ ] `bytes.len() -> i64`
  - [ ] `bytes.is_empty() -> bool`
  - [ ] `bytes.slice(start: i64, end: i64) -> bytes`
  - [ ] `bytes.get(index: i64) -> u8` or an explicitly optional/result-returning variant if chosen
  - [ ] endian-specific scalar reads.
- [ ] Add `bytes.to_hex() -> string`.
- [ ] Add `bytes::from_hex(text: string) -> Result`.
- [ ] Never use host-endian reads.

**Termination condition:**

- [ ] Bytes APIs are manifest-driven and implemented in stdlib runtime.
- [ ] Tests cover indexing, slicing, endian reads, and hex conversion.

---

### Phase 8: Stdlib numeric conversion and wrapping APIs

#### Task 8.1: Add explicit numeric conversion APIs

- [ ] Add infallible widening where truly safe:
  - [ ] `i64::from_i32`
  - [ ] `u64::from_u32`
  - [ ] etc.
- [ ] Add fallible narrowing:
  - [ ] `i8::try_from_i64`
  - [ ] `u8::try_from_u64`
  - [ ] etc.
- [ ] Add float conversions:
  - [ ] `f64::from_f32`
  - [ ] `f32::try_from_f64` or explicit policy.
- [ ] Do not add implicit conversion.

**Termination condition:**

- [ ] Mixed numeric code can be fixed through explicit conversion APIs.
- [ ] Conversion APIs are manifest-driven and registered in stdlib runtime.

#### Task 8.2: Add wrapping/bit APIs

- [ ] Add wrapping arithmetic:
  - [ ] `u8::wrapping_add`
  - [ ] `u32::wrapping_mul`
  - [ ] representative signed variants.
- [ ] Add bit helpers:
  - [ ] `bit_and`
  - [ ] `bit_or`
  - [ ] `bit_xor`
  - [ ] `shift_left`
  - [ ] `shift_right`
  - [ ] `rotate_left`
  - [ ] `rotate_right`
- [ ] Decide whether syntax operators follow now or later.

**Termination condition:**

- [ ] Binary/protocol code can express wrapping and bit manipulation without relying on implicit overflow.

---

### Phase 9: Host, serde, C API, and hot-reload ABI integration

#### Task 9.1: Host/native typed conversions

- [ ] Add host conversion for:
  - [ ] `i8/i16/i32/i64`
  - [ ] `u8/u16/u32/u64`
  - [ ] `f32/f64`
  - [ ] `Vec<u8>` / byte slices to `bytes`.
- [ ] Ensure wrong runtime type produces contract/conversion error.
- [ ] Ensure numeric conversions are explicit, not automatic.
- [ ] Ensure `HostAccess` arithmetic requires matching concrete scalar tags.

**Termination condition:**

- [ ] Host/native tests cover every primitive scalar type.
- [ ] `Vec<u8>` round-trips as `bytes`.

#### Task 9.2: Serde policy

- [ ] Decide bytes representation:
  - [ ] base64;
  - [ ] hex;
  - [ ] or explicit serde config.
- [ ] Decide large unsigned integer JSON behavior.
- [ ] Ensure `u64` is not silently lossy through JSON.
- [ ] Add tests for `u64::MAX` if JSON serialization is supported.

**Termination condition:**

- [ ] Serde tests document bytes and unsigned integer behavior.
- [ ] No silent precision loss is introduced.

#### Task 9.3: C API tags

- [ ] Add C ABI tags for new scalar types.
- [ ] Add bytes result/argument representation.
- [ ] Add cleanup rules for bytes buffers.
- [ ] Update C API tests.

**Termination condition:**

- [ ] C API can pass and return `i32`, `u32`, `f32`, `f64`, and bytes.
- [ ] ABI ownership for bytes is documented.

#### Task 9.4: Hot-reload ABI/schema primitive diffs

- [ ] Normalize primitive hints through IDs/tags, not strings, before ABI/schema comparison.
- [ ] Reject `i32 -> i64`, `i64 -> u64`, `f32 -> f64`, `bytes -> string`, and all other primitive changes unless a future explicit compatibility rule is added.
- [ ] Update function, method, trait, and schema ABI tests from `int`/`float` to explicit primitive pairs.
- [ ] Preserve product-level hot reload ABI/schema compatibility checks while removing old internal compatibility names.

**Termination condition:**

- [ ] Hot-reload ABI/schema tests reject primitive contract changes with source-spanned diagnostics.
- [ ] No ABI comparison path relies on raw `"int"` or `"float"` strings.

---

### Phase 10: Verification, tests, and cleanup

#### Task 10.1: Update verifier

- [ ] Verify param guard count matches param count.
- [ ] Verify return guard handles are valid.
- [ ] Verify guard instruction handles are valid.
- [ ] Verify deferred literal instruction operands are valid.
- [ ] Verify bytes constants load correctly.

**Termination condition:**

- [ ] Invalid guard handle tests fail verification.
- [ ] Valid programs pass verification.

#### Task 10.2: Remove old `int` / `float` assumptions

Run and fix:

```bash
rg '"int"'
rg '"float"'
rg 'Value::Int'
rg 'Value::Float'
rg 'OwnedValue::Int'
rg 'OwnedValue::Float'
rg 'HostValue::Int'
rg 'HostValue::Float'
rg 'Constant::Int'
rg 'Constant::Float'
rg 'TypeHint::Int'
rg 'TypeHint::Float'
```

- [ ] Remove all old type names from compiler logic.
- [ ] Remove old `Value` / `OwnedValue` / `HostValue` / `Constant` variants.
- [ ] Remove old `TypeHint` variants.
- [ ] Update docs/examples/tests.

**Termination condition:**

- [ ] Grep finds no old int/float runtime variants.
- [ ] Grep finds no source-level `"int"`/`"float"` type hint support, except migration notes or docs explaining removal.

#### Task 10.3: Add conformance fixtures

Add fixtures for:

- [ ] default integer literal -> `i64`;
- [ ] default float literal -> `f64`;
- [ ] type hinted integer literal contextualization;
- [ ] suffix mismatch compile error;
- [ ] string-to-i64 compile error;
- [ ] dynamic-to-i64 runtime guard success/failure;
- [ ] typed field assignment guard success/failure;
- [ ] `fn inc(x) { x + 1 }` with `i8/u32/i64`;
- [ ] bound literal no longer deferred;
- [ ] mixed numeric runtime error;
- [ ] checked overflow;
- [ ] signed-min literals;
- [ ] bytes literal and indexing;
- [ ] endian reads.

**Termination condition:**

- [ ] Conformance suite passes.
- [ ] Negative compile/runtime fixtures assert the expected phase: compile error vs runtime error.

---

## 9. JIT Readiness Requirements

Even before JIT exists, this refactor must preserve these invariants:

- [ ] Contract guards and specialization guards are represented distinctly.
- [ ] Contract guard failure is a language runtime error.
- [ ] Specialization guard failure is fallback/deopt, not a language error.
- [ ] Guard plans use `PrimitiveTag`, `TypeHandle`, `VariantHandle`, `ShapeId`, not strings.
- [ ] Function param guards are metadata, not buried only as arbitrary bytecode instructions.
- [ ] Static proven calls can be represented as unchecked calls.
- [ ] Dynamic calls use checked calls.
- [ ] Deferred literal operations are explicit bytecode forms, so JIT can specialize them.

Future JIT lowering should be straightforward:

```text
Contract guard:
    compare tag/handle
    branch to runtime type error on failure

Specialization guard:
    compare tag/handle
    branch to fallback/deopt on failure

Typed scalar op:
    direct machine op after guard
```

---

## 10. Final Termination Criteria for the Whole Refactor

The refactor is complete when all of the following are true:

- [ ] `int` and `float` are no longer valid source type hints.
- [ ] Active architecture docs describe the new primitive model and no longer document `int`/`float` as current value categories.
- [ ] Stdlib manifest registers explicit primitive scalar types and `bytes`.
- [ ] Runtime values use `ScalarValue`, not `Int(i64)` / `Float(f64)` variants.
- [ ] `OwnedValue`, `HostValue`, and `Constant` use scalar/bytes representations.
- [ ] Embedding descriptors use `TypeHint::Primitive(PrimitiveTag)`, not `TypeHint::Int` / `TypeHint::Float`.
- [ ] `HeapValue::Bytes` exists and is covered by GC/conversion tests.
- [ ] Numeric literal suffixes work.
- [ ] Signed-min literals such as `-128i8` and `-9223372036854775808i64` work.
- [ ] Byte string literals work.
- [ ] Unsuffixed integer literal defaults to `i64` only when it escapes without context.
- [ ] Unsuffixed float literal defaults to `f64` only when it escapes without context.
- [ ] Inline unsuffixed numeric literals in dynamic operator contexts can be deferred.
- [ ] Type hints are contracts, not conversions.
- [ ] Statically known type contract violations are compile errors.
- [ ] Dynamic unknown contract violations are runtime errors.
- [ ] Typed field writes guard dynamic values and reject statically known mismatches.
- [ ] Numeric operators require identical concrete numeric types.
- [ ] Default arithmetic is checked.
- [ ] Explicit conversion APIs exist for common numeric conversions.
- [ ] Bytes can represent binary data without `Array<i64>` or `Array<u8>`.
- [ ] Guard metadata is linked and verifier-checked.
- [ ] Static safe calls can avoid runtime guard execution.
- [ ] JIT readiness invariants in Section 9 hold.
- [ ] `cargo fmt --all -- --check` passes.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes.
- [ ] `cargo test --workspace` passes.

---

## 11. Short Version for Codex

If a task needs the shortest possible guiding rule, use this:

```text
Vela is dynamic by default.
No type hint means no contract.
A type hint is a runtime contract, never a conversion.
If the compiler can prove the contract is violated, emit a compile error.
If the value is dynamic, emit a linked runtime guard.
Numeric operators require identical concrete numeric types.
Unsuffixed literals are context-typed; inline unsuffixed numeric literals may be deferred in dynamic operators.
The shared scalar model covers Value, OwnedValue, HostValue, Constant, TypeHint, and C API tags.
No int/float aliases: use i64/f64.
Use bytes for binary data.
Make all guard plans handle/tag based so future JIT can lower them directly.
```
