## Hot Reload First

### Immutable Generation Pins

`ProgramVersion` owns one same-generation verified-MIR bundle and linked
artifact. A closure stores its creation-generation executable owner together
with its dense handle, and a frame pins the owner selected at call entry until
that frame exits. An accepted safe-point reload changes only the generation
used for subsequent entry calls. Existing frames and host-retained closures
continue on their old code, including old nested calls and old cache/profile
sidecars. Stable function names and IDs are ABI comparison keys, never an
implicit closure-migration mechanism.

### Core Model

```rust
pub struct Runtime {
    image: RuntimeImage,
    hot_reload: HotReloadRuntime,
    state: RuntimeState,
}

pub struct ProgramVersion {
    pub id: ProgramVersionId,
    artifact: Arc<LinkedArtifact>,
    verified_mir: Arc<OwnedVerifiedMirBundle>,
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

At public entry-call time:

```text
stable function name/ID -> active ProgramVersion -> owner-qualified handle
```

Hot reload replaces the active immutable generation at a safe point. Closure
and nested calls do not repeat this lookup: they resolve their dense handle
against the immutable linked owner pinned by the closure or active frame.

### Old Stack And New Stack

Rules:

```text
currently executing old functions continue on old CodeObject values
new calls use new CodeObject values
old generations are released after old frames, closures, and retained values exit
updates take effect only at safe points
```

Each runtime keeps cache entries and profile counters in generation-keyed
sidecars. Accepted reload activates a fresh sidecar atomically. Old sidecars
remain available while an old owner is retained and are pruned through weak
generation tokens at later safe points; a sidecar never retains executable
code by itself.

The same immutable artifact also maps verified MIR functions to linked handles.
Future M22 compilation may consume the read-only restricted-JIT input on a
ProgramVersion without rebuilding HIR or analysis. Published machine code must
belong to that generation; tier selection remains runtime-local, and old code
is invalidated by owner lifetime rather than name rebasing.

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
