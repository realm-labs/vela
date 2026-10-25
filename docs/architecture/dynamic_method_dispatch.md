# Dynamic Method Dispatch

Method calls dispatch on script/standard value receivers. The reduced runtime
has no Rust-object receiver or transparent host-method boundary. Ordinary
synchronous native module functions remain independent of method dispatch.

## Compile-Time Split

Known receivers with a known method lower through stable `MethodId` and a
linked dispatch handle. Provably absent known methods may be compile errors.
Unknown receivers with a source-static name lower to `CallDynamicMethod`,
preserving the method name and source argument order/names. Structurally valid
dynamic bytecode links; a missing runtime method is a source-spanned execution
error, not rejection of ordinary dynamic-language source by the linker.

## Linked Bytecode Contract

Resolved and dynamic calls have distinct instruction shapes. Resolved operands
use dense handles/slots/IDs; dynamic operands carry a diagnostic/source-static
name, argument registers/names and cache site. No raw string fallback replaces
ID-based resolved calls. Required missing native definitions, corrupt handles
and registry mismatches remain linker failures.

## Runtime Receiver Classification

Classification includes strings, bytes, arrays, maps, sets, Option/Result,
ranges, script records/enums and unsupported values. Heap metadata is required
for precise script type identity. There is no HostRef/path/opaque receiver class.

## Resolution Order

1. Standard value methods resolve through stable standard IDs/targets.
2. Script impl methods resolve by script type and source method name, then use
   the same linked script path as statically resolved calls.
3. Otherwise raise a source-spanned method error.

## Arguments

Preserve named argument spelling/order until a target signature is known.
Linked script methods reorder names, fill defaults and run guards only after
target resolution. Unknown names, missing required arguments and invalid arity
are diagnostics. Standard dynamic method named/default support depends on its
actual registry/materialization contract; do not infer script-method parity
from common syntax or resurrect host receiver metadata to obtain it.

## Cache Guards

Cache entries guard source method name, runtime receiver kind/script shape and
exact executable owner. A mismatch reruns resolution, not a contract trap.
Resolved targets and mutable cache/profile data remain generation-qualified.
Shared execution-data slots are distinct from immutable instructions; optional
lane-local storage requires measured contention and explicit ownership.

## Runtime Errors

Missing/unsupported methods, arity/name/default failures and type contract
mismatches retain the original call span and stack. Dynamic execution cannot
bypass native effects/capabilities, reflection policy, roots or finite budgets.

## Host Boundary

The old HostRef/HostPath/PathProxy/HostTargetPlan, adapter and host receiver
method families were removed in D. No independent opaque runtime survived.
Dynamic dispatch is not a replacement object system or an alias to removed
registration. See [native registration](host-and-registration.md).

## Hot Reload

Stable IDs compare ABI; dense handles/cache slots are local to an artifact.
Activation gives new entries fresh execution data. Old frames/closures retain
their exact artifact and nested dispatch/cache/plan owner. Never clear/rebase
old sites in place or rebind old closures by name. Rejected/stale updates leave
active code and caches unchanged.

## Coverage Audit

Standard dynamic resolution covers String, Bytes, Array, Map, Set, Option,
Result and Range families. Script lookup uses the linked type/name dispatch
map rather than fixture-specific names. Test static/dynamic routes, unknown
receivers, guard misses, source argument mapping, permission/budget failure and
old-generation ownership. [Validation](../validation.md) owns commands.
