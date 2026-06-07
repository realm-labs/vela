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
        .filter(|a| a.kind == invoice.kind)
        .map(|a| PaymentAdjustment {
            code: a.code,
            amount: a.amount,
        })

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
Option::None expected absence in business or lookup logic
Result::Err  recoverable failure with a script-visible reason
VM error    unrecoverable trap, script bug, contract violation, budget failure, or sandbox denial
```

Script and standard-library APIs should prefer `Option` for expected missing
data and `Result` for expected recoverable failure. They should not use `null`
as the normal "not found" or "failed" result:: `null` remains the value for
statement-only blocks, no-result native calls, reflection metadata gaps, and
host/Rust nullable interop.

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
