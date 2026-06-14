# Primitive Types, Type Hints, And Guards

This document is the active contract for primitive scalar values, `bytes`,
type hints, and runtime guard metadata. It replaces the old two-type numeric
model. Source type names `int` and `float` are removed; they are not aliases.

## Primitive Model

Vela is dynamic by default, but primitive values use explicit concrete names:

```text
null
bool
char
i8 i16 i32 i64
u8 u16 u32 u64
f32 f64
string
bytes
```

Source type hints use lowercase only for scalar/literal primitive contracts.
The public spelling for erased, text/binary, collection, callable, and
Option/Result contracts is capitalized:

```text
Any
String Bytes
Array Map Set Range Iterator Function Closure
Array<T> Set<T> Map<K, V> Iterator<T>
Option<T>
Result<T, E>
```

Only builtin type-hint contracts may carry type arguments:
`Array<T>`, `Set<T>`, `Map<K, V>`, `Iterator<T>`, `Option<T>`, and
`Result<T, E>`. This is not a general script generic system. `Map<K, V>` keys
and `Set<T>` elements must satisfy the runtime `ValueKey` keyability contract:
immutable leaf values compare by value, script heap objects and host refs
compare by identity, and transient values such as `PathProxy` are rejected.
User/schema/host generics such as `Player<T>`, scalar hints such as
`String<T>`, callable signatures such as `Function<T>`, and non-keyable
container key contracts such as `Set<PathProxy>` are rejected.

Unsuffixed integer literals default to `i64` only when they escape without a
more specific expected type. Unsuffixed float literals default to `f64` only
when they escape without a more specific expected type.

```vela
12       // i64 if unconstrained
12i8     // exactly i8
12u32    // exactly u32
0xffu8   // exactly u8
12.0     // f64 if unconstrained
12.0f32  // exactly f32
b"abc"   // bytes
'x'      // char
```

The shared low-level vocabulary is:

```rust
pub enum PrimitiveTag {
    Null,
    Bool,
    Char,
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
```

`Value`, `OwnedValue`, `HostValue`, bytecode `Constant`, type hints, serde/C
API boundaries, and guard plans must use this shared primitive vocabulary.
`bytes` is heap-backed for runtime slots and owned as byte buffers at
embedding/constant boundaries.

## Type Hints

No type hint means no contract. `Any` is explicit erased dynamic metadata and
also creates no contract by itself. Passing an unhinted or `Any` value into a
more specific site may still require that target site to guard.

Type hints are contracts, never conversions:

```text
exact same type       -> accepted, no runtime guard
exact different type  -> compile error
dynamic or erased     -> accepted with a runtime contract guard
```

Literal context can satisfy a contract without conversion:

```vela
fn f(x: i64) {}

f(12)    // OK: the unsuffixed literal is context-typed as i64
f(12i8)  // compile error: i8 is not i64
f(12.0)  // compile error: f64 is not i64
f(x)     // OK if x is dynamic; checked by a runtime guard
```

Contract locations include function parameters and returns, lambda parameters,
typed `let`, typed `global` binding insertion/update at embedding boundaries,
script record and enum fields, later writes to typed fields, host/native
function parameters and returns, and typed serde/C API decode boundaries.

## Numeric Operations

Numeric operators require identical concrete scalar types. There are no hidden
integer widening, integer-to-float, or float-width conversions.

```vela
1i32 + 2i32  // OK, result is i32
1i32 + 2i64  // compile error if statically known
1i64 + 2.0   // compile error if statically known
```

If operands are dynamic, the VM checks concrete scalar tags at runtime and
raises a type mismatch when they differ. Default arithmetic is checked:
integer overflow and unsigned underflow are runtime errors, and constant-folded
overflow is a compile error. Explicit wrapping and conversion APIs belong in
the standard library; operators do not perform implicit conversion.

Inline unsuffixed numeric literals in dynamic numeric operators may be deferred
instead of immediately becoming `i64` or `f64`:

```vela
fn inc(x) {
    return x + 1
}

inc(1i8)   // 1 is contextualized to i8 at runtime
inc(1u32)  // 1 is contextualized to u32 at runtime
inc(1i64)  // 1 is contextualized to i64 at runtime
inc(1.0)   // runtime error: integer literal does not become float
```

Bound literals are not deferred:

```vela
fn inc_strict(x) {
    let one = 1
    return x + one
}
```

Here `one` defaults to `i64` if unconstrained.

## Bytes

`bytes` is the binary-data primitive category. It is immutable and heap-backed
at runtime.

```vela
let header = b"\x00\xff"
let first = header[0] // u8
```

Byte string escapes are limited to byte-producing forms such as `\n`, `\r`,
`\t`, `\0`, `\"`, `\\`, and `\xNN`. Unicode escapes are not part of the initial
byte string surface. Byte APIs must use explicit endian operations rather than
host-endian reads.

## Guard Kinds

Contract guards are language semantics. They validate type-hint contracts and
fail with a runtime type contract error.

Specialization guards are implementation assumptions for inline caches, JIT
speculation, or hot dynamic operator specialization. They fail by falling back
or deoptimizing, not by raising a language contract error.

```rust
pub enum GuardKind {
    Contract,
    Specialization,
}
```

## Guard Plans

Guard plans must be linked and hot-path friendly. They compare primitive tags,
type handles, variant handles, shape IDs, or host type handles. They must not
perform string comparison or registry lookup on the hot path.

```rust
pub enum TypeGuardPlan {
    Primitive(PrimitiveTag),
    Type(TypeHandle),
    Variant(VariantHandle),
    Shape { ty: TypeHandle, shape_id: ShapeId },
    HostType(TypeHandle),
}
```

Debug names remain available through side tables for diagnostics, reflection,
and source reports.

## Function Entries

Functions with parameter contracts have a checked entry that validates runtime
guards and an unchecked body entry that assumes the caller already proved the
contracts. The first implementation may encode this as metadata, but the
representation must support this distinction.

```text
statically proven script call  -> unchecked body
dynamic script call            -> checked entry
reflection/host/C API call     -> checked entry
future inlined proven call     -> unchecked body
```

Return guards execute before returning from functions with return contracts.
Local and field guards use explicit guard bytecode or equivalent linked
metadata when the compiler cannot statically prove the contract. Typed global
declarations are bound by the host/runtime rather than assigned directly in
script source, so their insertion/update contracts must be enforced at embedding
boundaries.
