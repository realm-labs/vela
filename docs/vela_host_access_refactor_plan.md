# Vela Host Access Refactor Plan

## Purpose

This plan describes a clean-architecture refactor of Vela's host access model.
It assumes **no internal compatibility requirement**. Existing path-first APIs can
be deleted, tests can be rewritten, and examples can be updated to the new model.

The goal is to replace the current execution-time `HostPath + Vec<PathSegment>`
model with a resolved, cache-ready host access model:

```text
Compiled host target plan
    -> runtime target instance
    -> resolved adapter access
    -> immediate HostAccess operation
```

The final architecture should preserve Vela's product semantics:

- Scripts never receive real Rust `&T` or `&mut T` references.
- Host writes remain immediate write-through operations.
- Stale host references are rejected using `HostRef` generation checks.
- Reflection, diagnostics, permissions, hot reload, and schema invalidation remain explicit.
- The VM does not know how Rust structs, ECS worlds, actor state, databases, or mocks are traversed.

Status note: checked items below reflect the repository state inspected on
2026-06-09. Keep this plan as a checklist, not a changelog: when a checkbox is
marked, it should point to current code, tests, docs, or validation output.

## Terminal Condition

This refactor is finished only when every item in both the task checklist and
the architecture acceptance checklist is checked, and the final validation
commands pass:

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace`
- [ ] focused host-boundary benchmarks have durable before/after conclusions in
  `docs/performance.md`, or the benchmark deferral is explicitly documented.
- [ ] `docs/progress.md` moves the HostPath/HostAccess M19.5 gap to completed.
- [ ] `docs/architecture/host-and-registration.md` describes only the resolved
  host access model, with `HostPath` documented as diagnostic, reflection, or
  embedding materialization rather than the hot adapter API.

## Current Problem

The current core adapter contract is path-first:

```rust
pub trait ScriptStateAdapter {
    fn global_ref(&self, name: &str) -> HostResult<HostRef>;
    fn global_ref_by_slot(&self, slot: GlobalSlot, name: &str) -> HostResult<HostRef>;

    fn read_path(&self, path: &HostPath) -> HostResult<HostValue>;
    fn write_path(&mut self, path: &HostPath, value: HostValue) -> HostResult<()>;
    fn remove_path(&mut self, path: &HostPath) -> HostResult<()>;
    fn call_method(
        &mut self,
        path: &HostPath,
        method: HostMethodId,
        args: &[HostValue],
    ) -> HostResult<HostValue>;
}
```

`HostPath` currently combines several responsibilities:

```rust
pub struct HostPath {
    pub root: HostRef,
    pub segments: Vec<PathSegment>,
}

pub enum PathSegment {
    Field(FieldId),
    Index(u32),
    Key(String),
    VariantField(FieldId),
}
```

That shape is flexible and correct, but it is not ideal for hot execution. It
causes repeated path materialization, segment traversal, dynamic key/index
conversion, path cloning for write operations, and adapter-side matching on every
access.

The refactor should make `PathSegment` a diagnostic/materialization concept, not
the hot execution model.

## Target Architecture

The clean target is:

```text
HostPath is not the adapter API.
PathSegment is not the VM hot operand.
HostTargetPlan is the compiled operand.
HostTargetInstance is the runtime access object.
ResolvedHostAccess is the adapter execution handle.
HostDiagnosticPath is for errors, reflection display, and debug output.
```

### New Core Types

Create a new module, likely `crates/vela_host/src/target.rs`:

```rust
use vela_common::{FieldId, HostMethodId, HostTypeId};
use crate::path::HostRef;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct HostTargetPlan {
    pub root_type: HostTypeId,
    pub parts: HostPathParts,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum HostPathPart {
    Field(FieldId),
    VariantField(FieldId),
    ConstIndex(u32),
    ConstKey(String),
    DynIndex { arg: u8 },
    DynKey { arg: u8 },
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct HostPathParts {
    // Use inline storage for 0-4 common segments and Vec fallback for long paths.
    // This can reuse the existing PathKeySegments idea but without HostRef.
    inner: HostPathPartsStorage,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostPathArg<'a> {
    Index(u32),
    Key(&'a str),
}

#[derive(Clone, Copy, Debug)]
pub struct HostTargetInstance<'a> {
    pub root: HostRef,
    pub plan: &'a HostTargetPlan,
    pub args: &'a [HostPathArg<'a>],
}
```

Important design choice:

```text
HostTargetPlan is shape-based and reusable.
HostTargetInstance contains the concrete HostRef.
```

Do not include `object_id` or `generation` in the cacheable target plan. Those
belong to the per-access instance.

### New Resolved Access Types

Create `crates/vela_host/src/resolved.rs`:

```rust
use vela_common::{HostMethodId, HostTypeId};
use crate::target::{HostTargetInstance, HostTargetPlan};
use crate::value::HostValue;
use crate::error::HostResult;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct HostSchemaEpoch(pub u64);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum HostAccessOp {
    Read,
    Write,
    Mutate(HostMutationOp),
    Remove,
    Call(HostMethodId),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum HostMutationOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Push,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct HostAccessSpec<'a> {
    pub op: HostAccessOp,
    pub plan: &'a HostTargetPlan,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ResolvedHostAccess {
    pub adapter_kind: ResolvedHostAccessKind,
    pub schema_epoch: HostSchemaEpoch,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ResolvedHostAccessKind {
    GenericPath,
    DirectField(u32),
    DirectMethod(u32),
    AdapterLocal(u32),
}
```

`ResolvedHostAccessKind` should not expose Rust references or unsafe pointers.
The workspace forbids unsafe code, so derive-generated access should use safe
Rust match tables or function thunks.

## Replace the Adapter Trait

Delete the path-first methods from `ScriptStateAdapter`. Replace them with a
resolve-then-execute model.

Suggested shape:

```rust
pub trait ScriptStateAdapter {
    fn host_schema_epoch(&self) -> HostSchemaEpoch;

    fn global_ref(&self, global: GlobalBinding<'_>) -> HostResult<HostRef>;

    fn resolve_host_access(
        &self,
        spec: HostAccessSpec<'_>,
    ) -> HostResult<ResolvedHostAccess>;

    fn read_host(
        &self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
    ) -> HostResult<HostValue>;

    fn write_host(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        value: HostValue,
    ) -> HostResult<()>;

    fn mutate_host(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        op: HostMutationOp,
        rhs: HostValue,
    ) -> HostResult<()>;

    fn remove_host(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
    ) -> HostResult<()>;

    fn call_host(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        method: HostMethodId,
        args: &[HostValue],
    ) -> HostResult<HostValue>;
}
```

Suggested `GlobalBinding`:

```rust
#[derive(Clone, Copy, Debug)]
pub struct GlobalBinding<'a> {
    pub name: &'a str,
    pub slot: Option<GlobalSlot>,
}
```

Do not keep `read_path`, `write_path`, `remove_path`, or `call_method` as default
fallback methods. If a generic adapter still wants path-style traversal
internally, it can materialize a diagnostic path privately inside its own
implementation.

## Refactor `HostAccess`

`HostAccess` should become a policy and routing boundary, not a path construction
boundary.

Responsibilities:

- Add source spans to errors.
- Enforce immediate write-through semantics.
- Resolve cache entries or ask the adapter to resolve access.
- Route read/write/mutate/remove/call to the adapter.
- Materialize diagnostic paths only for errors.

Suggested shape:

```rust
pub struct HostAccess;

impl HostAccess {
    pub fn read(
        &self,
        adapter: &dyn ScriptStateAdapter,
        cache: &mut dyn HostAccessCache,
        target: HostTargetInstance<'_>,
        source_span: Option<Span>,
    ) -> HostResult<HostValue> {
        let access = cache.resolve_or_insert(adapter, HostAccessOp::Read, target)?;
        adapter
            .read_host(access, target)
            .map_err(|error| error.with_source_span_if_absent(source_span))
    }

    pub fn write(
        &mut self,
        adapter: &mut dyn ScriptStateAdapter,
        cache: &mut dyn HostAccessCache,
        target: HostTargetInstance<'_>,
        value: HostValue,
        source_span: Option<Span>,
    ) -> HostResult<()> {
        let access = cache.resolve_or_insert(adapter, HostAccessOp::Write, target)?;
        adapter
            .write_host(access, target, value)
            .map_err(|error| error.with_source_span_if_absent(source_span))
    }
}
```

## Inline Cache Model

Add host access cache support to the VM inline cache trait.

Current `VmInlineCaches` only has global read slot support. Extend it with host
access operations:

```rust
pub trait VmInlineCaches {
    fn len(&self) -> usize;

    fn global_read_slot(&self, site: CacheSiteId) -> Option<GlobalSlot>;
    fn set_global_read_slot(&self, site: CacheSiteId, slot: GlobalSlot);

    fn host_access(&self, site: CacheSiteId) -> Option<HostInlineCacheEntry>;
    fn set_host_access(&self, site: CacheSiteId, entry: HostInlineCacheEntry);
}
```

Cache entry:

```rust
#[derive(Clone, Debug)]
pub struct HostInlineCacheEntry {
    pub root_type: HostTypeId,
    pub plan_id: HostTargetPlanId,
    pub op: HostAccessOp,
    pub schema_epoch: HostSchemaEpoch,
    pub resolved: ResolvedHostAccess,
}
```

Cache hit guard:

```text
root.type_id == entry.root_type
plan_id == entry.plan_id
op == entry.op
adapter.host_schema_epoch() == entry.schema_epoch
```

Do not guard on:

- `HostObjectId`
- `generation`
- source span
- diagnostic names
- register numbers

The adapter still validates stale object generations during execution.

## Refactor Bytecode

Replace the many host instruction variants with a smaller host instruction
family.

Current instruction family includes:

```text
GetHostField
GetHostPath
SetHostField
SetHostPath
AddHostField
SubHostField
MulHostField
DivHostField
RemHostField
AddHostPath
SubHostPath
MulHostPath
DivHostPath
RemHostPath
PushHostPath
RemoveHostPath
CallHostMethod
```

Replace with:

```rust
pub enum InstructionKind {
    HostRead {
        dst: Register,
        root: Register,
        target: HostTargetPlanId,
        dynamic_args: Vec<Register>,
        cache_site: CacheSiteId,
    },

    HostWrite {
        root: Register,
        target: HostTargetPlanId,
        dynamic_args: Vec<Register>,
        src: Register,
        cache_site: CacheSiteId,
    },

    HostMutate {
        root: Register,
        target: HostTargetPlanId,
        dynamic_args: Vec<Register>,
        op: HostMutationOp,
        rhs: Register,
        cache_site: CacheSiteId,
    },

    HostRemove {
        root: Register,
        target: HostTargetPlanId,
        dynamic_args: Vec<Register>,
        cache_site: CacheSiteId,
    },

    HostCall {
        dst: Option<Register>,
        root: Register,
        target: HostTargetPlanId,
        dynamic_args: Vec<Register>,
        method: HostMethodId,
        args: Vec<Register>,
        cache_site: CacheSiteId,
    },
}
```

Add target plans to `CodeObject`:

```rust
pub struct CodeObject {
    // existing fields...
    pub host_targets: Vec<HostTargetPlan>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct HostTargetPlanId(pub u32);
```

Compiler lowering should intern host target plans per `CodeObject` so repeated
accesses to the same shape reuse the same `HostTargetPlanId`.

## Dynamic Index and Key Handling

Do not embed `Value(Register)` in host path parts.

Current `HostPathSegment::Value(Register)` is too broad. It requires runtime
conversion from a VM `Value` into either an integer index or string key while
constructing a full `HostPath`.

Use explicit dynamic argument parts instead:

```rust
HostPathPart::DynIndex { arg: 0 }
HostPathPart::DynKey { arg: 1 }
```

The instruction carries the source registers:

```rust
dynamic_args: vec![r4, r5]
```

VM execution converts only those dynamic arguments:

```rust
fn host_arg_from_value(value: Value, heap: Option<&HeapExecution<'_>>) -> VmResult<HostPathArg<'_>> {
    match value {
        Value::Int(index) => {
            let index = u32::try_from(index).map_err(|_| type_mismatch("host index"))?;
            Ok(HostPathArg::Index(index))
        }
        Value::HeapRef(_) => {
            let key = expect_borrowed_string(value, heap)?;
            Ok(HostPathArg::Key(key))
        }
        _ => Err(type_mismatch("host path dynamic argument")),
    }
}
```

Prefer borrowed string views where possible. Materialize owned strings only when
an adapter needs to store them or produce diagnostics.

## Refactor `PathProxy`

`PathProxy` currently owns `HostPath`. Replace that with a target handle.

Suggested shape:

```rust
pub struct PathProxy {
    root: HostRef,
    target: HostTargetPlan,
    args: SmallVec<HostPathArgOwned, 2>,
}
```

Or, if path proxies must be VM-managed objects independent of one `CodeObject`:

```rust
pub struct PathProxy {
    root: HostRef,
    target: HostTargetPlan,
    args: Vec<HostPathArgOwned>,
}
```

Owned path args:

```rust
pub enum HostPathArgOwned {
    Index(u32),
    Key(String),
}
```

PathProxy operations should call `HostAccess::{read, write, mutate, remove, call}`
with a `HostTargetInstance`, not clone a `HostPath`.

## Refactor Direct Host Objects

Current direct host object traits are still path-first:

```rust
fn read_host_path(&self, path: &HostPath) -> HostResult<HostValue>;
fn write_host_path(&mut self, path: &HostPath, value: HostValue) -> HostResult<()>;
fn call_host_method(&mut self, path: &HostPath, method: HostMethodId, args: &[HostValue])
    -> HostResult<HostValue>;
```

Replace with resolved target access.

Suggested split:

```rust
pub trait ScriptHostObject {
    fn host_type_id(&self) -> HostTypeId;

    fn resolve_host_target(
        &self,
        spec: HostAccessSpec<'_>,
    ) -> HostResult<ResolvedHostAccess>;

    fn read_resolved_host(
        &self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
    ) -> HostResult<HostValue>;

    fn write_resolved_host(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        value: HostValue,
    ) -> HostResult<()>;

    fn call_resolved_host(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        method: HostMethodId,
        args: &[HostValue],
    ) -> HostResult<HostValue>;
}
```

For generated structs, derive macros should generate safe resolver tables:

```rust
match (op, root_type, field_chain) {
    (HostAccessOp::Read, PLAYER_TYPE, [FIELD_INVENTORY, FIELD_GOLD]) => {
        Ok(ResolvedHostAccess::direct_field(PLAYER_INVENTORY_GOLD_READ))
    }
    _ => Err(missing_path(...)),
}
```

The actual read/write thunk remains safe Rust:

```rust
fn read_player_inventory_gold(
    player: &Player,
    _target: HostTargetInstance<'_>,
) -> HostResult<HostValue> {
    Ok(HostValue::Int(player.inventory.gold))
}
```

For nested container fields, generated code should call normal safe accessors:

```rust
fn read_player_scores_index(
    player: &Player,
    target: HostTargetInstance<'_>,
) -> HostResult<HostValue> {
    let index = target.arg_index(0)? as usize;
    let value = player.scores.get(index).ok_or_else(|| missing_target(target))?;
    value.into_host_value()
}
```

## Refactor Mock Adapter First

Rewrite `MockStateAdapter` as the reference adapter for the new architecture.

Suggested internal storage:

```rust
pub struct MockStateAdapter {
    objects: BTreeMap<HostObjectKey, u32>,
    values: BTreeMap<MockValueKey, HostValue>,
    method_returns: BTreeMap<HostMethodId, HostValue>,
    method_calls: Vec<MockMethodCall>,
    denied: BTreeSet<MockDeniedAccess>,
    schema_epoch: HostSchemaEpoch,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MockValueKey {
    root: HostRef,
    target: HostTargetPlan,
    args: Vec<HostPathArgOwned>,
}
```

This keeps test behavior simple while forcing the new target model through every
host access path.

Tests to add first:

```text
mock_read_resolved_static_field
mock_write_resolved_static_field
mock_rejects_stale_generation
mock_rejects_denied_read
mock_rejects_denied_write
mock_records_resolved_method_call
mock_dynamic_index_key_identity
mock_diagnostic_path_materializes_on_error
```

## VM Refactor Flow

Execution for `HostRead` should be:

```rust
let root = expect_host_ref(frame.read(root)?, "host_read")?;
let plan = code.host_target(target)?;
let dynamic_args = materialize_host_args(&frame, dynamic_arg_registers, heap.as_deref())?;
let instance = HostTargetInstance {
    root,
    plan,
    args: dynamic_args.as_slice(),
};
let value = host.access.read(
    host.adapter,
    inline_caches,
    instance,
    instruction.span,
)?;
let value = runtime_value_from_host(value, heap.as_deref_mut(), budget.as_deref_mut())?;
frame.write(dst, value)?;
```

Execution for `HostWrite` should be:

```rust
let root = expect_host_ref(frame.read(root)?, "host_write")?;
let value = value_to_host(frame.read(src)?, "host_write", heap.as_deref())?;
let plan = code.host_target(target)?;
let dynamic_args = materialize_host_args(&frame, dynamic_arg_registers, heap.as_deref())?;
let instance = HostTargetInstance { root, plan, args: dynamic_args.as_slice() };
host.access.write(host.adapter, inline_caches, instance, value, instruction.span)?;
```

Execution for `HostMutate` should not read/compute/write inside the VM. Route the
mutation operation to `HostAccess` and then to the adapter:

```rust
host.access.mutate(
    host.adapter,
    inline_caches,
    instance,
    op,
    rhs_value,
    instruction.span,
)?;
```

This keeps adapter-defined collection and scalar mutation semantics behind the
host boundary.

## Compiler Refactor

Update host field/path lowering:

### Old model

```text
GetHostField root field
GetHostPath root [Field, Value(Register), Field]
```

### New model

```text
HostRead root target_plan_id dynamic_arg_registers cache_site
```

Compiler should:

1. Build a `HostTargetPlan` from known type facts and field/index syntax.
2. Intern it in `CodeObject.host_targets`.
3. Emit dynamic argument registers separately.
4. Allocate a `CacheSiteKind::HostPathRead` or `HostPathWrite` site.
5. Preserve field/method names in debug metadata, not in hot operands.

For simple field access:

```vela
player.level
```

Plan:

```rust
HostTargetPlan {
    root_type: PLAYER_TYPE,
    parts: [HostPathPart::Field(FIELD_LEVEL)],
}
```

For nested access:

```vela
player.inventory.gold
```

Plan:

```rust
HostTargetPlan {
    root_type: PLAYER_TYPE,
    parts: [
        HostPathPart::Field(FIELD_INVENTORY),
        HostPathPart::Field(FIELD_GOLD),
    ],
}
```

For dynamic index:

```vela
player.scores[i]
```

Plan:

```rust
HostTargetPlan {
    root_type: PLAYER_TYPE,
    parts: [
        HostPathPart::Field(FIELD_SCORES),
        HostPathPart::DynIndex { arg: 0 },
    ],
}
```

Instruction:

```rust
HostRead {
    dst,
    root,
    target,
    dynamic_args: vec![i_register],
    cache_site,
}
```

## Diagnostics

Create `HostDiagnosticPath`:

```rust
pub struct HostDiagnosticPath {
    pub root: HostRef,
    pub segments: Vec<HostDiagnosticSegment>,
}

pub enum HostDiagnosticSegment {
    Field(FieldId),
    Index(u32),
    Key(String),
    VariantField(FieldId),
}
```

Provide materialization:

```rust
impl HostTargetInstance<'_> {
    pub fn to_diagnostic_path(&self) -> HostDiagnosticPath;
}
```

Use this only for:

- `HostErrorKind::MissingPath`
- permission diagnostics
- reflection display
- tests that intentionally inspect paths
- debug traces

Do not use it in hot successful reads/writes.

## Hot Reload and Schema Epochs

Add a host schema epoch that changes when host type layout, permissions, method
resolution, field IDs, index capabilities, or reflection schema changes in a way
that can invalidate resolved access.

Where to store:

```rust
Runtime / ProgramVersion / TypeRegistry / HostAdapter state
```

The cache guard should compare the adapter-reported epoch with the cached epoch.

On hot reload:

- accepted reload with compatible host schema can keep epoch if target IDs remain valid;
- rejected reload keeps previous epoch;
- schema-changing reload increments epoch and invalidates host access caches;
- ProgramVersion-owned profile/cache metadata should not leak across schema-incompatible versions.

## File-by-File Refactor Checklist

### `crates/vela_host/src/path.rs`

- Keep `HostRef`.
- Move old `HostPath`, `PathSegment`, and `HostPathKey` into diagnostic or legacy-removal scope.
- Add or move new target types into `target.rs`.
- Delete root-inclusive `HostPathKey` as a cache key.
- If keeping a key type, make it shape-based: `HostTargetKey { root_type, parts, op }`.

### `crates/vela_host/src/adapter.rs`

- Replace path-first trait with resolved-access trait.
- Remove default compatibility fallback methods.
- Add `host_schema_epoch`.
- Replace string-only globals with `GlobalBinding { name, slot }`.

### `crates/vela_host/src/access.rs`

- Replace `read_path`, `set_path`, `add_path`, etc. with `read`, `write`, `mutate`, `remove`, `call`.
- Take `HostTargetInstance` instead of `HostPath`.
- Resolve through cache before adapter execution.
- Keep source span wrapping.
- Keep stale generation helper or move it to a validation module.

### `crates/vela_host/src/proxy.rs`

- Replace `PathProxy { path: HostPath }` with `PathProxy { root, target, args }`.
- Update `field`, `index`, and `key` builders to extend target plans and args.
- Remove methods that clone `HostPath`.

### `crates/vela_host/src/mock.rs`

- Rewrite as first implementation of the new adapter trait.
- Store values by `MockValueKey` based on root + target + owned args.
- Validate generation on every access.
- Add tests for dynamic args and diagnostic materialization.

### `crates/vela_host/src/object.rs`

- Replace `read_host_path_from` / `write_host_path_from` with resolved target access.
- For maps/vectors/sets, use `HostPathPart` and dynamic args rather than offset into `PathSegment`.
- Keep safe Rust only.

### `crates/vela_bytecode/src/lib.rs`

- Add `HostTargetPlanId`.
- Add `host_targets: Vec<HostTargetPlan>` to `CodeObject`.
- Replace host instruction family with `HostRead`, `HostWrite`, `HostMutate`, `HostRemove`, `HostCall`.
- Add cache site IDs to each host instruction.
- Delete `HostPathSegment::Value(Register)`.

### `crates/vela_bytecode/src/cache_site.rs`

- Keep `HostPathRead` and `HostPathWrite`, or rename to `HostRead` and `HostWrite`.
- Add `HostMutate`, `HostRemove`, and `HostCall` cache kinds if useful.

### `crates/vela_bytecode/src/verification.rs`

Add verification for:

- `HostTargetPlanId` is in bounds.
- Dynamic arg count matches the target plan's dynamic placeholders.
- Dynamic arg indexes are contiguous and valid.
- Cache site kind matches instruction kind.
- Host call method ID exists when static metadata is available.
- Host target plan root type is valid when type metadata is available.

### `crates/vela_vm/src/host_paths.rs`

- Delete full `HostPath` materialization as the normal path.
- Replace with dynamic argument conversion helpers.
- Keep diagnostic materialization helpers if they belong in VM; preferably move them to `vela_host`.

### `crates/vela_vm/src/host_access.rs`

- Rewrite around `HostTargetInstance`.
- Delete `read_host_field`, `set_host_field`, `read_host_path`, `set_host_path` split if bytecode is collapsed.
- Add `execute_host_read`, `execute_host_write`, `execute_host_mutate`, `execute_host_remove`, `execute_host_call`.

### `crates/vela_vm/src/host_mutations.rs`

- Replace field/path split with target-instance mutation.
- Move mutation arithmetic decisions to host adapter or keep scalar arithmetic in `HostAccess` only if the semantic contract requires it.
- Prefer adapter-defined mutation for collections and host-specific behavior.

### `crates/vela_vm/src/execution.rs`

- Replace host instruction match arms with the smaller host instruction family.
- Keep the main VM loop thin.
- Delegate all host behavior to `host_access.rs`.

### `crates/vela_macros`

- Generate `HostTargetPlan` resolver metadata.
- Generate safe direct access thunks.
- Generate method resolver entries by `HostMethodId`.
- Generate diagnostics metadata separately from hot operands.

### `crates/vela_reflect`

- Use target plans for controlled reads/writes/calls.
- Materialize diagnostic paths only for user-facing messages.
- Ensure permissions are checked during resolution and/or execution.

### `docs/architecture/host-and-registration.md`

- Replace path-first adapter contract with resolved target contract.
- Explain `HostTargetPlan`, `HostTargetInstance`, and `ResolvedHostAccess`.
- Clarify `HostDiagnosticPath`.
- Preserve the rule that scripts never receive real Rust mutable references.

### `docs/progress.md`

- Move HostPath/HostAccess key work from remaining gap to completed once done.
- Mention collapsed host bytecode family and resolved adapter access.

### `docs/performance.md`

- Add benchmark rows before/after:
  - host field read
  - host field write
  - host RMW
  - nested host path read
  - nested host path write
  - dynamic index host read
  - host method call

## Refactor Checklist

Use these as separate tasks or PRs. Do not ask Codex to do the entire refactor
in one pass unless the repository is small enough for a single coherent patch.
Mark a top-level task only when all of its subitems and acceptance commands are
complete.

### Task 1: Introduce Target and Resolved Types

- [x] Add `HostTargetPlan`, `HostPathPart`, `HostTargetInstance`,
  `HostPathArg`, and owned dynamic args in `vela_host`.
- [x] Add `HostAccessSpec`, `ResolvedHostAccess`, `HostSchemaEpoch`, and
  `HostMutationOp`.
- [x] Add diagnostic path materialization from `HostTargetInstance`.
- [x] Keep the new types safe Rust only.
- [x] Record or rerun acceptance:

```text
cargo test -p vela_host target
cargo test -p vela_host resolved
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

### Task 2: Replace `ScriptStateAdapter`

- [x] Replace the path-first trait contract with `global_ref(GlobalBinding)`,
  `resolve_host_access`, `read_host`, `write_host`, `mutate_host`,
  `remove_host`, and `call_host`.
- [x] Remove path-first methods from the `ScriptStateAdapter` trait itself.
- [x] Add `host_schema_epoch`.
- [x] Rewrite `MockStateAdapter` around resolved access and target identity for
  successful operations.
- [x] Remove or quarantine `MockStateAdapter` path convenience helpers so tests
  no longer treat path-first access as the primary API.
- [x] Update host access tests to use `HostTargetPlan` and
  `HostTargetInstance`, not `HostPath`, except for explicit diagnostic or
  embedding materialization tests.
- [x] Record or rerun acceptance:

```text
cargo test -p vela_host
cargo fmt --all -- --check
cargo clippy -p vela_host --all-targets -- -D warnings
```

### Task 3: Refactor HostAccess Boundary

- [x] Route `read`, `write`, `mutate`, `remove`, and `call` through
  `HostTargetInstance` and `ResolvedHostAccess`.
- [x] Keep source-spanned error wrapping.
- [x] Keep immediate write-through semantics.
- [x] Delete path-construction helpers such as `read_path`, `read_path_at`,
  `remove_path`, and `call_method` from the normal `HostAccess` surface, or
  move them behind explicit diagnostic/embedding conversion names.
- [x] Record or rerun acceptance:

```text
cargo test -p vela_host
cargo test -p vela_vm host_access
```

### Task 4: Collapse Host Bytecode Instructions

- [x] Add `HostRead`, `HostWrite`, `HostMutate`, `HostRemove`, and `HostCall`.
- [x] Add `host_targets` to `CodeObject`.
- [x] Add and use `HostTargetPlanId`.
- [x] Verify target bounds, dynamic arg count, contiguous dynamic arg indexes,
  and cache-site kind matching for the collapsed host family.
- [x] Delete or fully retire legacy host instruction variants such as
  `GetHostPath`, `SetHostPath`, and `AddHostPath`.
- [x] Delete `HostPathSegment::Value(Register)` once no remaining legacy
  instruction needs it.
- [x] Record or rerun acceptance:

```text
cargo test -p vela_bytecode
cargo test -p vela_bytecode verification
```

### Task 5: Rewrite VM Host Execution

- [x] Execute the collapsed host family through `HostTargetPlanId` plus dynamic
  arg registers.
- [x] Convert dynamic index/key registers into `HostPathArg` values for the
  collapsed family.
- [x] Route collapsed host operations through the focused VM host access
  boundary.
- [x] Delete normal execution-time `HostPath` materialization for all successful
  hot host reads/writes.
- [x] Remove legacy host instruction execution arms once all callers compile to
  the collapsed family.
- [ ] Record or rerun acceptance:

```text
cargo test -p vela_vm host
cargo test -p vela_vm execution
cargo test --workspace
```

### Task 6: Update Compiler Lowering

- [x] Lower host field, host path, host mutation, host remove, push, and host
  method calls to the collapsed host instruction family.
- [x] Intern `HostTargetPlan` values into each `CodeObject`.
- [x] Emit dynamic arg registers separately from target shape.
- [ ] Audit that diagnostic names are not retained as hot operands where a
  stable ID, slot, or target plan is available.
- [x] Remove tests that still expect legacy host bytecode, unless the test is
  explicitly covering legacy removal.
- [ ] Record or rerun acceptance:

```text
cargo test -p vela_bytecode compiler
cargo test -p vela_engine
cargo test --workspace
```

### Task 7: Refactor Direct Host Object and Macros

- [x] Add resolved target access methods to direct host object support.
- [ ] Generate `HostTargetPlan` resolver metadata from macros.
- [ ] Generate safe direct access thunks for reads, writes, mutations, and
  method calls.
- [ ] Ensure generated diagnostics metadata is separate from hot operands.
- [ ] Record or rerun acceptance:

```text
cargo test -p vela_macros
cargo test -p vela_engine host
cargo test --workspace
```

### Task 8: Reflection and Diagnostics Cleanup

- [x] Add `HostDiagnosticPath` and materialization from target instances.
- [x] Keep reflection able to show readable host paths.
- [ ] Update reflection reads, writes, and calls to use target plans internally
  where practical.
- [ ] Remove test assumptions that successful hot host access creates owned
  `HostPath` values.
- [ ] Record or rerun acceptance:

```text
cargo test -p vela_reflect
cargo test --workspace reflection diagnostics
```

### Task 9: Delete Old Path-First API

- [ ] Delete obsolete path-first adapter, access, bytecode, VM, and test helper
  surfaces that are not explicit diagnostic/embedding conversion APIs.
- [ ] Keep `HostRef` and diagnostic path materialization.
- [ ] Update docs and examples to show the resolved host access model only.
- [ ] Do not add compatibility aliases.
- [ ] Record or rerun acceptance:

```text
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

### Task 10: Benchmark and Performance Report

- [ ] Add or update host-boundary benchmark rows for host field read/write.
- [ ] Add or update rows for nested host path read/write.
- [ ] Add or update rows for RMW mutation.
- [ ] Add or update rows for dynamic index/key access.
- [ ] Add or update rows for host method calls.
- [ ] Update `docs/performance.md` only with durable conclusions, not routine
  logs.
- [ ] Record or rerun acceptance:

```text
cargo bench -p vela_vm --bench baseline -- --quick
cargo bench -p vela_vm --bench external_compare -- --quick
```

## Validation Gates

Use these gates after each meaningful stage:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

For focused work:

```bash
cargo test -p vela_host
cargo test -p vela_bytecode
cargo test -p vela_vm
cargo test -p vela_engine
cargo test -p vela_reflect
cargo test -p vela_macros
```

For performance-sensitive host access changes:

```bash
cargo bench -p vela_vm --bench baseline -- --quick
```

## Architecture Acceptance Criteria

The refactor is done when all of these are true:

- [x] `ScriptStateAdapter` no longer exposes `read_path`, `write_path`,
  `remove_path`, or `call_method`.
- [x] VM bytecode no longer stores `Vec<HostPathSegment>` for host access
  instructions.
- [x] VM execution no longer materializes `HostPath` on successful hot
  reads/writes.
- [x] Host dynamic indexes and keys are passed as explicit dynamic args
  everywhere.
- [x] Host inline caches key on root type, operation, target plan, and schema
  epoch.
- [x] Cache keys do not include object ID or generation.
- [x] Stale generation validation still happens during adapter execution.
- [x] `HostAccess` still preserves immediate write-through semantics.
- [x] Source-spanned diagnostics still work.
- [x] Reflection can still show readable host paths.
- [ ] Direct host object access uses safe generated Rust only. The resolved
  object surface exists, but macro-generated resolver/thunk coverage still
  needs completion or audit.
- [x] The refactor crates remain safe Rust. The existing `vela_c_api` FFI crate
  is the explicit workspace exception and is outside this host access refactor.
- [ ] Tests and examples use the new architecture rather than compatibility
  shims. Several tests still assert through `HostPath` convenience helpers.

## Anti-Goals

Do not do these:

- Do not introduce raw `&mut T` exposure to scripts.
- Do not use unsafe field offsets or pointer arithmetic.
- Do not keep path-first adapter methods as compatibility fallbacks.
- Do not include `HostObjectId` or generation in reusable cache keys.
- Do not embed VM registers inside host target shape.
- Do not make `HostAccess` a transaction or rollback journal.
- Do not hide old behavior behind aliases.
- Do not update docs to describe both old and new APIs as valid.

## Suggested Final Architecture Diagram

```text
Compiler
  lowers host access syntax
  interns HostTargetPlan into CodeObject
  emits HostRead / HostWrite / HostMutate / HostRemove / HostCall

VM
  reads root HostRef register
  converts dynamic arg registers to HostPathArg
  creates HostTargetInstance
  delegates to HostAccess

HostAccess
  resolves or reads inline cache
  applies source span policy
  routes operation immediately to adapter

ScriptStateAdapter
  resolves HostAccessSpec to ResolvedHostAccess
  executes read/write/mutate/remove/call
  validates generation and permissions
  owns Rust/ECS/database/mock traversal details

Diagnostics / Reflection
  materialize HostDiagnosticPath only when needed
```

## Minimal End-State Example

Script:

```vela
fn handle(player, reward) {
    player.inventory.gold += reward.gold;
}
```

Compiled shape:

```text
host_targets:
  0: Reward.gold
  1: Player.inventory.gold

instructions:
  HostRead   dst=r3 root=reward target=0 dynamic_args=[] cache_site=c0
  HostMutate root=player target=1 dynamic_args=[] op=Add rhs=r3 cache_site=c1
```

Runtime flow:

```text
read reward root
resolve/cache Reward.gold read
execute adapter read

read player root
resolve/cache Player.inventory.gold Add mutation
execute adapter mutate immediately
```

No owned `HostPath` is constructed on the successful hot path.

## One-Line Summary

Refactor from:

```text
VM builds HostPath -> adapter walks PathSegment every time
```

to:

```text
compiler emits HostTargetPlan -> VM builds HostTargetInstance -> adapter executes ResolvedHostAccess
```
