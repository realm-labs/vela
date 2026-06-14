## Language Semantics

The first grammar draft lives in [grammar.ebnf](grammar.ebnf). It is the syntax
target for the parser milestones before semantic validation and lowering.

Parser implementations should preserve source spans for every token and AST
node. A future LSP needs this for diagnostics, completion replacement ranges,
go-to-definition, rename, hover, and incremental reparsing. The compiler may
lower into a simpler AST/HIR, but the syntax layer should keep a lossless CST or
equivalent token tree with comments and newlines.

Example script. `Account`, `Invoice`, and event names are host-registered
domain concepts, not builtin language or stdlib items:

```rust
use billing::account::Account
use billing::invoice::Invoice

struct PaymentAdjustment {
    code
    amount
}

enum WorkflowState {
    None
    Active { workflow_id, count }
    Finished { workflow_id }
}

trait Auditable {
    fn audit(self, message)
}

#[event("invoice.paid")]
pub fn on_invoice_paid(ctx, account, invoice) {
    account.balance += invoice.amount

    if account.balance >= ctx.config.preferred_balance {
        account.status = "preferred"
        ctx.emit("account.preferred", account.id, account.balance)
    }

    let adjustments = ctx.config.payment_adjustments
        .iter()
        .filter(|a| a.kind == invoice.kind)
        .map(|a| PaymentAdjustment {
            code: a.code,
            amount: a.amount,
        })
        .collect_array()

    for adjustment in adjustments {
        account.ledger.add(adjustment.code, adjustment.amount)
    }

    for index, adjustment in adjustments {
        ctx.log("adjustment.index", index)
    }

    match account.workflow {
        WorkflowState::Active { workflow_id, count } => {
            account.workflow = WorkflowState::Active {
                workflow_id,
                count: count + 1,
            }
        }
        _ => {}
    }
}
```

### Equality And Ordering

Semantic object equality and ordering are opt-in. `Eq` is the closed builtin
trait for user-object `==`/`!=`, and `Ord` is the closed builtin trait for
user-object ordering and sorting. User records/structs do not receive implicit
structural equality or ordering; they must implement the builtin trait
explicitly or use explicit `#[derive(Eq)]` / `#[derive(Eq, Ord)]` when every
field satisfies the required trait.

Missing `Eq` or `Ord` support is a compile-time diagnostic when statically
known and a source-spanned runtime error for dynamic values. `Hash`,
`PartialEq`, and `PartialOrd` are not script-visible builtin traits in the
first slice. `f32` and `f64` keep primitive comparison behavior where it
already exists, but float sorting and float `Eq`/`Ord` derivation are deferred
until a later partial-comparison or total-float-order design.

Identity comparison for script heap objects and host refs remains separate
from semantic `Eq`. It must not read host state.

`==` and `!=` must not recursively materialize and deep-compare object graphs.
If Vela adds deep structural comparison later, it should be an explicit,
budgeted helper rather than the default equality operator.

### Module Identity

Vela source files do not declare their own module names. There is no
`module ...` item in the language.

Single-file compilation is the lightweight script-entry mode:

```text
engine.compile_file("scripts/level_up.vela")
entry function: main
```

The file name is not part of the module identity in this mode.

Directory compilation is the module-graph mode used for imports, reflection,
dependency impact, and hot reload:

```text
scripts/game/main.vela   -> game::main
scripts/game/reward.vela -> game::reward
scripts/config.vela      -> config
```

Imports use the same static path syntax:

```rust
use game::reward::grant
```

The final path segment is the declaration name; the preceding segments are the
module path. Public declarations are imported from their owning module, and a
directory-compiled entrypoint is called by its fully qualified function name,
such as `game::main::main`.

### Dynamic Type Boundary

The language is dynamically typed, with lightweight hints and metadata.
Primitive type hints, numeric literal typing, and contract guard rules are
defined in
[Primitive types, type hints, and guards](primitives-type-hints-and-guards.md).
Source type names `int` and `float` are removed; use explicit scalar names such
as `i64`, `u32`, `f32`, and `f64`.

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
i8 / i16 / i32 / i64
u8 / u16 / u32 / u64
f32 / f64
string
bytes
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

### Iteration

Vela uses one internal iteration model:

```text
Iterable  value or view that can create an iterator
Sequence  repeatable iterable/view that creates a fresh iterator each traversal
Iterator  one-shot cursor; `next()` advances its internal state
```

These are runtime and analysis concepts, not script-language generic types.
Scripts may write `Iterator<T>` only as a builtin type-hint contract at
function, field, local, or embedding boundaries; it does not introduce
script-defined generic iterator types.

`for value in source` evaluates `source` once, creates or consumes an iterator
through the runtime iteration boundary, then repeatedly advances that iterator
until it is done. Existing iterator values are one-shot: looping over an
iterator consumes the same cursor state observed by later `next()` calls.

Indexed `for index, value in source` remains syntax-level loop lowering. It
does not allocate an eager `enumerate()` adapter. Proven `i64` ranges may keep a
specialized bytecode fast path when it preserves the same observable iteration
semantics.

Arrays and sets are repeatable sequences. Their `iter()` and `values()` methods
create one-shot iterators over values. Maps are repeatable sequences whose
direct iteration and `iter()` yield values in key order; `keys()`, `values()`,
and `entries()` expose explicit key, value, and `MapEntry` views. Ranges are
repeatable sequences and may use specialized `i64` loop lowering when the
compiler can prove the range facts.

String iteration is explicit and UTF-8-aware. `for ch in text` uses the same
character traversal source as `text.chars()`, yielding `char` values.
`text.bytes()` yields UTF-8 bytes as `u8`. String `len()`, `find()`, and
`slice(start, end)` remain byte-indexed.

Iterator adapters such as `map`, `filter`, `take`, and `skip` are lazy and
one-shot. Terminal methods such as `next`, `count`, `any`, `all`, `find`, and
`collect_array` consume the iterator cursor. `collect_array()` is the core
terminal that materializes an output collection; lazy adapters do not allocate
intermediate arrays.

Script generics are not supported. Only selected builtin type-hint contracts
may carry type arguments:

```text
Array<T>          allowed as a builtin array contract
Set<T>            allowed for set-keyable T: null, bool, i64, f64, String
Map<String, V>    allowed because runtime maps are string-keyed
Iterator<T>       allowed as a builtin iterator contract
Option<T>         allowed as a builtin Option contract
Result<T, E>      allowed as a builtin Result contract
Player<T>         not supported
Map<K, V>         not supported when K is not String
Set<Player>       not supported because records are not set-keyable in this slice
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
Option::None expected absence in business or lookup logic
Result::Err  recoverable failure with a script-visible reason
VM error    unrecoverable trap, script bug, contract violation, budget failure, or sandbox denial
```

Script and standard-library APIs should prefer `Option` for expected missing
data and `Result` for expected recoverable failure. They should not use `null`
as the normal "not found" or "failed" result:: `null` remains the value for
statement-only blocks, no-result native calls, reflection metadata gaps, and
host/Rust nullable interop.

### Strings

Single-line strings use `"..."` and process ordinary escapes such as `\n`,
`\t`, `\"`, `\\`, and Unicode escapes. Triple-quoted strings use `"""..."""`
and preserve their body text exactly, including newlines and indentation.

String interpolation is explicit. `f"..."` and `f"""..."""` may contain
`{expr}` interpolation parts, while `{{` and `}}` produce literal braces.
Plain strings never interpolate. Interpolation compiles to a dedicated
format-string bytecode instruction rather than numeric `+`, so it does not
change the meaning of the addition operator.

Control-flow expressions produce values. Empty or statement-only blocks
evaluate to `null`, and expression-valued `if` without an `else` evaluates to
`null` on the untaken branch.

### Dynamic Traits / Protocols

Traits are runtime capabilities or protocols, not Rust traits.

Supported:

```text
inherent impl methods on script types
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
