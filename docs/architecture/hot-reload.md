# Script Hot Reload

Hot reload replaces script code, not Rust implementations or host objects.
The post-D contract retains ordinary packages, compatible script state, stable
ABI checks and **exact old executable generations**.

## Immutable Generation Pins

`ProgramVersion` owns the linked artifact and same-generation verified MIR.
Stable names/IDs are semantic and ABI keys, not executable addresses. Dense
function/method handles, state slots, caches, profile sites and physical plans
are qualified by their immutable owner.

A new public entry resolves through the active generation. A frame pins its
entry owner until exit; a closure pins its creation owner. Nested script and
native dispatch use the pinned artifact, never the active generation's
similarly named declaration or reused handle. Retained closures must never be
silently rebound by stable name.

## Safe Points And Activation

The host stages a candidate and activates it at an explicit synchronous safe
point, for example between application calls. Source ingestion, compilation and
linking failures are structured Engine errors. Linked ABI/policy outcomes are
reported by the reload staging path.

Activation is transactional per Runtime: publish the candidate code, state maps,
staged cells and execution-data ownership only after all checks succeed. A
rejected or stale candidate leaves the active image, state, dispatch, caches and
plans unchanged. Activation does not switch bytecode midway through a frame.

## State And Top-Level Effects

Module declarations include imports, constants, functions, types, traits,
attributes and VM-owned `state` with restricted initializers. There are no
extern host-state bindings. Arbitrary module-level I/O, random operations or
application mutations are not a reload initialization mechanism.

State compatibility compares stable `StateId` and exact normalized contracts.
Existing compatible cells survive without rerunning their initializers. Added
state is staged before activation; incompatible contracts reject. Renaming a
state declaration is remove plus add. Removing a public state export or making
it private rejects; private removal and private-to-public promotion remain
separate from storage preservation.

Initializer edits are reported as new-Runtime behavior rather than resetting
existing state. Initializer fingerprints include reachable static script helper
calls, nested closure/default executables and cycle-safe graph traversal.
Staged graph copying is transaction-budgeted and preserves internal aliases and
cycles; it is not a new external serialization contract.

Old-generation slot maps retain state needed by old closures, including state
removed from the active generation. Reclamation follows live generation
ownership transitively; a removed closure-valued state root must not keep itself
alive indefinitely. Reload checks may reclaim dead generations even when no
update is pending. Script GC and retained-value release must agree with this
liveness computation.

## ABI Checks

Body/local/private-helper changes can reload without changing exported
contracts. Function/method parameter and return contracts, stable identities,
type/variant/field contracts, effects, access and package policy participate in
compatibility checks. Permission expansion needs explicit host approval, not a
silent intersection with existing grants.

Do not assume a schema change implies structural migration of live values.
Stable field/variant identities support compatibility comparison, not arbitrary
field deletion, ID reuse or object migration. Source spans in rejected ABI
reports identify candidate declarations when available.

## Packages

Reload rebuilds the ordinary manifest/source graph and reapplies its canonical
root fingerprint. Package identity, public dependency contracts and capability
requirements stay checked before image advance. Ordinary dependency body edits
can update new calls while old closures retain their original graph owner.
There is no Provider selection fingerprint, Service generation, Rust-default
composition, publication/folding/rollback or host-object migration.

## Caches And Physical Plans

Generation-shared execution data is weakly registered by exact executable
identity. Fresh activation gets fresh caches/profile ownership; old frames and
closures keep their matching data. Rejected/stale candidates cannot clear or
rebase active counters.

Superinstructions, scalar blocks, range-loop plans, source points, coverage and
physical profile layouts follow the same artifact lifetime. A compatible reload
may choose different interpreter plans without changing semantic ABI, but the
executable checksum/generation changes. There is no JIT compilation contract.

## Regression Proof

[Engine reload tests](../../crates/vela_engine/tests/hot_reload.rs) cover retained
closures across handle-layout changes, old state removed from the new image,
and old native dispatch after accepted/rejected updates. Ordinary package and
runtime safe-point tests complement them; see
[validation](../validation.md). Historical removed-feature success tests do not
establish current support. [Current progress](../progress.md) and the
[acceptance report](../archive/runtime-simplification-acceptance.md) record
completed A–E software gates, separately from incomplete strict/native
Input/Render coverage.
