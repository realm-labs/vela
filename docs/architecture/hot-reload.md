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
billing.on_invoice_paid(account, invoice)
```

Internally uses:

```text
FunctionSymbolId("billing.on_invoice_paid")
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
#[event("invoice.paid")]
pub fn on_invoice_paid(ctx, account, invoice) {
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
