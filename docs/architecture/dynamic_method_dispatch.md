# Dynamic Method Dispatch

Vela method calls are receiver-dispatched. Linked bytecode must keep statically
known calls on stable IDs while still allowing ordinary dynamic-language calls
when the receiver type is not known at compile time.

This is the final controlled dynamic dispatch design. It is not compatibility
for legacy name-only fallback, unlinked execution, monkey patching, or
runtime-computed method names.

## Compile-Time Split

Method lowering uses the receiver fact available at the call site:

```text
receiver known + method exists
  -> unlinked CallMethodId(method_id)
  -> linked MethodDispatchHandle
  -> VM resolved fast path

receiver known + method provably absent
  -> compile-time diagnostic is allowed

receiver unknown + source-static method name
  -> unlinked CallDynamicMethod(method_name, source args)
  -> linked dynamic method instruction
  -> VM resolves against the runtime receiver
```

The known-receiver path must not be weakened. Calls such as a typed script
record method, a statically known standard value method, or a registered host
method continue to compile through `MethodId` and link to a
`MethodDispatchHandle` or equivalent host-access target.

The unknown-receiver path is also first class. A source call like
`value.starts_with("q")` must not be represented as an ambiguous unresolved
method, and a linked program must not reject it merely because the compiler did
not know the receiver type.

## Linked Bytecode Contract

Linked bytecode has separate representations for:

```text
resolved method dispatch
dynamic method dispatch
```

Resolved dispatch contains only linked handles, slots, IDs, and cache-site
operands needed by the fast path. Dynamic dispatch contains the source-static
method debug name, source argument registers and names, and a dynamic
method-call cache site. It must not overload resolved `CallMethod` variants
with "not yet resolved" state.

Linking a dynamic method call succeeds when the bytecode is structurally valid.
Unresolved source-level dynamic methods are runtime dispatch failures, not
linker failures. Linker failures are reserved for invalid bytecode, missing
registered definitions that are required by stable IDs, absent native
implementations, corrupt handles, or registry mismatches.

Successful runtime/program construction should contain valid linked bytecode.
The normal public path must not hide dynamic method link rejection behind a
later `ProgramNotLinked` error.

## Runtime Receiver Classification

Dynamic dispatch begins by classifying the actual runtime receiver with the
execution context needed for that value:

```text
string
bytes
array
map
set
Option enum
Result enum
range
script record or enum type
host ref with registered host type metadata
unsupported value
```

Heap values require heap/type metadata for precise classification. Host refs
require host execution and registered schema metadata. A resolver that only
looks at `Value` may classify scalar primitives, but it must not pretend that
heap or host receivers can be fully resolved without their owning context.

## Resolution Order

Dynamic resolution is controlled and registry-backed:

1. Resolve standard/value methods for the receiver category through stable
   standard method IDs or existing standard method targets.
2. Resolve linked script impl methods by runtime script type and source method
   name, then dispatch through the same linked script method path used by
   resolved calls.
3. Resolve host methods through registered host metadata and stable
   `HostMethodId`, then execute through the existing HostAccess host method
   boundary.
4. If no target matches, raise a source-spanned runtime method error.

This order may be implemented by focused resolver modules, but every target
family must resolve to stable IDs, linked handles, or cache-ready targets before
execution. Dynamic dispatch must not call an old raw string fallback as the
final architecture.

Static known receiver calls that are provably absent may still fail during
compilation. Dynamic dispatch is required only when the receiver is unknown or
not sufficiently narrowed by available facts.

## Arguments

Dynamic bytecode preserves source argument order and source argument names.
Named arguments are not rejected merely because the receiver type is unknown.

After runtime target resolution, argument materialization uses the resolved
target signature:

```text
positional arguments keep source order
named arguments are reordered by target parameter name
defaults are filled from the resolved script, standard, or host signature
unknown names produce source-spanned diagnostics
missing required arguments produce source-spanned diagnostics
type guards and host conversions run after materialization
```

Until a target is known, dynamic calls must not erase argument names or
prematurely transform them into signature-specific call slots.

## Cache Guards

Dynamic method inline caches are guarded by both the source method name and the
runtime receiver classification. A monomorphic entry is sufficient for the
first implementation:

```text
method name
receiver guard
resolved target
host schema epoch when the target is host-backed
program/image epoch or equivalent hot-reload validity
```

Guard examples:

```text
standard value receiver kind
script type name and shape when applicable
host type id plus schema epoch
```

On a cache hit, the VM may dispatch directly to the cached target. On a guard
miss, it must run the resolver again and either update the cache or raise the
appropriate runtime error. Guard mismatch is not a language error by itself.

Dynamic caches are runtime/image state, not shared mutable instruction state.
They must be cheap to clear or invalidate when hot reload or host schema
changes can make a cached target stale.

## Runtime Errors

Dynamic method failures are runtime diagnostics attached to the call source
span:

```text
missing method for receiver type
unsupported receiver type
wrong argument arity
unknown named argument
missing required argument
type guard or host conversion failure
HostAccess capability, effect, generation, or schema-epoch denial
```

The diagnostic should name the source method and receiver classification when
that information is available. It must preserve call-stack information and
source spans the same way other VM runtime errors do.

## Host Boundary

Host dynamic method dispatch never bypasses the host safety model:

```text
HostRef / HostPath / PathProxy
HostTargetPlan or linked host method target
HostAccess
ScriptStateAdapter
capability and effect checks
generation and schema epoch checks
```

Host method resolution uses registered host metadata and stable host method
IDs. It does not expose Rust `&mut T` to scripts, does not place host state
under the script GC, and does not mutate host type structure at runtime.

## Hot Reload

Hot reload compatibility remains based on stable IDs, ABI checks, schema
checks, and version-owned metadata. Debug-name IDs used inside one linked
program are image-local handles and are not cross-reload identity.

Accepted hot reloads must invalidate stale dynamic method cache entries before
new calls can reuse a cache-site index. Reused dynamic cache sites repopulate
from the new linked program and current host schema metadata. Rejected reloads
leave the active linked program and its runtime caches unchanged.
