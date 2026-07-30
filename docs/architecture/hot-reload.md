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
currently executing old functions continue on old generation-owned code
new calls use the newly published generation
old generations are released after old frames, closures, and retained values exit
updates take effect only at safe points
```

One Engine deployment weakly registers mutable execution data by exact
`ExecutableGenerationId`. Accepted reload publishes a fresh immutable artifact
and therefore fresh cache/profile storage; it never clears or rebases old
slots. Actor-local generation entries retain only state ownership sets and an
`Arc` to the matching shared execution data while old frames, closures,
suspended calls, or retained values can still execute it. Liveness subtracts
linked-artifact owners reachable only from inactive state roots and closes
transitively over state reachable by live generations; a removed closure-valued
state therefore cannot retain its own generation entry. A normal Runtime
reload check performs reclamation even when no update is pending: it first
collects released retained values, then prunes dead generation entries and
their old-only VM/extern state roots. The Engine's weak registry entry
disappears when the last shared execution-data owner is gone.

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
explicit runtime.activate_reload()
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
state with a restricted initializer
extern state declaration
```

Disallow or strictly limit:

```text
register_event(...)
spawn_task(...)
open_file(...)
state_counter += 1
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

State changes compare stable `StateId`, storage kind, and exact normalized type
contract. Existing compatible cells and extern bindings are preserved without
rerunning initializers. Added VM state is initialized in a temporary staging
runtime; added extern state must have a staged, type-compatible host binding.
Storage or type changes reject. A rename is reported as remove plus add, and an
initializer edit is reported even when it does not reject. Added-state values
move from staging to the live heap through one transaction-budgeted graph copy
that preserves aliases and cycles across all staged roots. State preservation
is separate from export compatibility: private removal and private-to-public
promotion are compatible, while removing an existing public state export or
downgrading it to private rejects the update. Initializer change reporting
fingerprints the transitive statically called script-function graph, including
calls reached through nested closure and parameter-default executables. Paired
visited nodes terminate recursive graphs, and unrelated helpers remain outside
the fingerprint, so edits to permitted reachable helpers are reported as
new-Runtime-only behavior.

Activation is transactional per Runtime: the candidate image, slot maps,
staged cells, extern bindings, and sidecar are published together only after
every check succeeds. Old generation slot maps keep removed state reachable
for old frames, closures, retained values, and suspended executions; pruning
occurs only after the final generation owner expires.

Package artifacts retain canonical ordinary roots and selected provider keys.
Reload rebuilds a package snapshot and reapplies that fingerprint before
artifact comparison. Selected provider keys, target types, service method IDs,
package identities, and capability requirements are runtime ABI; unselected
discovery entries are not. Provider-created closures retain their creation
artifact, while logical Runtime provider handles resolve stable keys against
the active image for each new call.

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
