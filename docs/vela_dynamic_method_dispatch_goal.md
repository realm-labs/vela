# Goal: Controlled Dynamic Method Dispatch for Linked Bytecode

## Objective

Implement Vela’s final dynamic method-call semantics for linked bytecode.

Vela should support normal scripting-language behavior:

```vela
for value in values {
    value.some_method(...)
}
```

When the receiver’s static shape/type is known, compiled linked bytecode must keep the current stable-ID fast path.

When the receiver’s static shape/type is unknown but the method name is a static source name, linked bytecode must still compile and link. Runtime must resolve the method from the receiver’s actual runtime type. If the runtime receiver does not support the method, the VM must raise a clear source-spanned runtime error, not reject linking and not produce a generic `ProgramNotLinked`.

When the receiver’s static type is known and the method is provably absent, the compiler may keep reporting a compile-time error. This goal is about preserving dynamic language behavior for unknown receivers, not about delaying all statically knowable errors until runtime.

This is a breaking clean-architecture change. Do not preserve old compatibility paths merely because tests currently expect them.

## Current problem summary

The current pipeline is effectively:

```text
receiver shape/type known
  -> compile to CallMethodId
  -> linker resolves to MethodDispatchHandle
  -> linked bytecode executes

receiver shape/type unknown
  -> compile to name-only CallMethod
  -> linker rejects with UnresolvedMethodName
  -> public runtime path may surface ProgramNotLinked instead of a precise runtime method error
```

The desired final split is:

```text
receiver shape/type known + method exists
  -> compile to CallMethodId
  -> linked bytecode executes the resolved fast path

receiver shape/type known + method is provably absent
  -> compile-time diagnostic is allowed

receiver shape/type unknown + source-static method name
  -> compile to CallDynamicMethod
  -> linked bytecode resolves against the runtime receiver
```

That means common dynamic scripting code like this is rejected too early:

```vela
fn f(x) {
    return x.starts_with("q");
}
```

Expected final behavior:

```text
f("quest") => true
f("raid")  => false
f(42)      => runtime method/type error at .starts_with(...)
```

## Non-negotiable design rules

- Do not restore unbounded legacy name lookup.
- Do not make linked bytecode silently fall back to unlinked execution.
- Do not implement dynamic linked calls by directly calling the old string fallback as the final architecture.
- Do not hide link errors by storing `linked_program: None` and surfacing `ProgramNotLinked` later.
- Static known receiver calls must remain `MethodId` / `MethodDispatchHandle` fast paths.
- Static known receiver calls to provably missing methods may remain compile-time errors.
- Dynamic calls must be first-class linked bytecode, with explicit runtime receiver guards and cache invalidation.
- Runtime errors must keep source spans.
- Host methods must still go through registered stable IDs, HostAccess, capability/effect checks, and schema epoch guards.
- Dynamic method names in this goal are source-static names only, not runtime-computed method names.

## Final semantics

```vela
fn starts_with_q(value) {
    return value.starts_with("q");
}
```

Expected behavior:

```text
starts_with_q("quest") => true
starts_with_q("raid")  => false
starts_with_q(42)      => runtime method/type error with source span
```

The same call site may see different receiver runtime types over time. The dynamic dispatch cache must guard against receiver type/shape/schema mismatches and safely fall back to resolution on miss.

## Phase Completion Rules

Each phase lists minimum exit tests. Those tests prove that phase's architecture
path and are not the full final method surface. The final feature surface is
defined by the final acceptance criteria and Phase 9 conformance, docs, and
benchmark coverage.

When a phase proves a mechanism with a representative subset, any remaining
standard, script, or host method families must either be covered by Phase 9 or
recorded as explicit follow-up gaps before this goal can be marked complete.

---

# Execution plan

## [x] Phase 0 — Write the architecture contract first

Create an architecture note, for example:

```text
docs/architecture/dynamic_method_dispatch.md
```

It must define:

- static receiver known → `CallMethodId` / resolved linked dispatch
- static receiver known + method absent → compile-time diagnostic is allowed
- static receiver unknown + source-static method name → linked dynamic method call
- runtime receiver classification
- method resolution order
- cache guard model
- runtime error model
- hot reload invalidation expectations
- named/default argument expectations for dynamic calls

The note must explicitly say that this is not legacy fallback compatibility. It is the final controlled dynamic dispatch design.

Verification:

```bash
cargo fmt --all -- --check
cargo test -p vela_bytecode -p vela_vm
```

Commit message:

```text
docs: define controlled dynamic method dispatch semantics
```

After commit, change this checkbox to `[x]`.

---

## [x] Phase 1 — Make dynamic method bytecode explicit

Refactor bytecode naming so unknown-receiver method calls are not represented as ambiguous “unresolved methods”.

Recommended shape:

```rust
UnlinkedInstructionKind::CallDynamicMethod {
    dst: Register,
    receiver: Register,
    method: String,
    args: Vec<DynamicCallArgument>,
}
```

Keep the existing resolved path:

```rust
UnlinkedInstructionKind::CallMethodId {
    dst,
    receiver,
    method,
    method_id,
    args,
}
```

Add a dedicated dynamic call argument representation that preserves source argument names instead of prematurely rejecting or erasing them:

```rust
DynamicCallArgument {
    name: Option<String>,
    value: Register,
}
```

Do not reuse the existing `CallArgument` shape for this until it can preserve
names. The current resolved-call argument representation is already
signature-materialized and cannot represent unknown target signatures without
losing named/default argument information.

Compiler behavior:

- If script receiver type is known and script method ID resolves, emit `CallMethodId`.
- Else if value receiver type is known and std/value method ID resolves, emit `CallMethodId`.
- Else if receiver type is known and no method can resolve, a compile-time diagnostic is allowed.
- Else emit `CallDynamicMethod`.
- Do not reject unknown-receiver named arguments in the compiler. Preserve names for runtime method resolution.

Update bytecode verification so `CallDynamicMethod` is a valid unlinked instruction with contiguous argument registers and a method-call cache site.

Verification tests:

- A source like `fn f(x) { return x.starts_with("q"); }` compiles to `CallDynamicMethod`.
- A source like `fn f() { return "quest".starts_with("q"); }` still compiles to the resolved fast path where type facts allow it.
- Existing resolved method-call tests still pass.

Commit message:

```text
bytecode: represent unknown receiver calls as dynamic methods
```

After commit, change this checkbox to `[x]`.

---

## [x] Phase 2 — Add first-class linked dynamic method IR

Add a linked instruction that is separate from resolved `CallMethod`.

Recommended shape:

```rust
InstructionKind::CallDynamicMethod {
    dst: Register,
    receiver: Register,
    method_name: DebugNameId,
    cache_site: Option<CacheSiteId>,
    args: Vec<DynamicCallArgumentLinked>,
}
```

Do not overload `LinkedMethodDispatchKind::Value` or `LinkedMethodDispatchKind::Script` for unresolved calls. Resolved dispatch and dynamic dispatch should remain architecturally distinct.

Linker behavior:

- `CallMethodId` continues to intern a `MethodDispatchHandle`.
- `CallDynamicMethod` links successfully into `InstructionKind::CallDynamicMethod`.
- Remove or retire `LinkError::UnresolvedMethodName` from source-level dynamic method semantics.
- Any remaining unresolved link errors should mean real invalid bytecode/registry problems, not “receiver type unknown”.

VM behavior in this phase may initially return a source-spanned `UnknownMethod` for dynamic calls until the resolver lands, but linking must succeed.

Verification tests:

- `fn f(x) { return x.starts_with("q"); }` links successfully.
- `fn f(x) { return x.trim(); }` links successfully.
- `fn f() { return 42.trim(); }` may remain a compile-time error if the compiler proves `i64` has no `trim`.
- Running unresolved dynamic call returns a VM runtime error with the call span.
- Public runtime creation/call path must not turn this into `ProgramNotLinked`.

Commit message:

```text
linker: link dynamic method calls into linked bytecode
```

After commit, change this checkbox to `[x]`.

---

## [x] Phase 3 — Add runtime receiver classification and std/value dynamic resolver

Create a focused resolver module, for example:

```text
crates/vela_vm/src/dynamic_method_resolution.rs
```

Define receiver classification:

```rust
enum DynamicReceiverKind {
    String,
    Bytes,
    Array,
    Map,
    Set,
    Option,
    Result,
    Range,
    ScriptRecord { type_name: String },
    ScriptEnum { type_name: String },
    Host { type_name: String, host_type_id: ... },
    Unsupported,
}
```

Classification must inspect the actual runtime value with the context required
to understand it:

```rust
fn classify_dynamic_receiver(
    receiver: &Value,
    heap: Option<&HeapExecution<'_>>,
    host: Option<&HostExecution<'_>>,
) -> DynamicReceiverKind
```

`Value::HeapRef` cannot be classified without heap access, and `Value::HostRef`
cannot be classified for host dispatch without host/registry metadata. Phase 3
may classify script and host receivers as unsupported placeholders, but it
must not pretend that `&Value` alone is sufficient for the final architecture.

Implement the first resolver slice for std/value methods:

```rust
fn resolve_standard_dynamic_method(
    receiver: &Value,
    method: &str,
    heap: Option<&HeapExecution<'_>>,
) -> Option<DynamicMethodTarget>
```

The target must resolve to stable IDs or existing standard cache targets, not raw ad hoc string calls.
This phase should support only standard/value methods. Script and host dynamic
method resolution remain separate phases so their ABI, HostAccess, and hot
reload boundaries do not leak into the first resolver slice.

Examples:

- `String + "starts_with"` → `MethodId(String::starts_with)`
- `String + "trim"` → `MethodId(String::trim)`
- `Array + "len"` → `MethodId(Array::len)`
- `Map + "get"` → `MethodId(Map::get)`
- unsupported receiver + method → no target

Then implement:

```rust
dispatch_linked_dynamic_method_call(...)
```

for standard/value methods.

Runtime behavior:

```vela
fn f(x) {
    return x.starts_with("q");
}
```

- `f("quest")` returns `true`.
- `f("raid")` returns `false`.
- `f(42)` raises a source-spanned runtime method/type error.

Verification tests:

- dynamic string predicate: `String.starts_with`
- dynamic string transform: `String.trim`
- dynamic array method: `Array.len`
- dynamic map method: `Map.get`
- dynamic option method: `Option.is_some`
- dynamic result method: `Result.is_ok`
- wrong receiver runtime type
- wrong argument arity/type
- source span is attached

Commit message:

```text
vm: resolve dynamic std value methods by receiver type
```

After commit, change this checkbox to `[x]`.

---

## [ ] Phase 4 — Add linked script method dynamic resolution

Linked bytecode needs a script method lookup table.

Add a linked-program side table, for example:

```rust
LinkedProgram {
    ...
    script_method_dispatches_by_type_and_name:
        BTreeMap<(DebugNameId, DebugNameId), MethodDispatchHandle>,
}
```

or another clean keyed structure that allows runtime lookup by receiver runtime type name and source method name.
If `DebugNameId` is used, the architecture note must state that it is
image-local to one `LinkedProgram`; it is not a stable cross-reload identity.
Hot reload compatibility still relies on stable `MethodId` and ABI checks, not
debug-name handles.

Linker behavior:

- While linking `UnlinkedProgram::script_methods()`, create normal resolved dispatch entries.
- Also populate dynamic lookup entries:
  - `(script type name, method name) -> MethodDispatchHandle`
- Do not require the compiler to know the receiver type for these entries to exist.

Runtime behavior:

- Dynamic receiver is script record or enum.
- Resolver gets runtime type name from the heap value.
- Resolver looks up `(type_name, method_name)` in linked program.
- If found, dispatch through the same linked script method call path used by resolved method calls.
- If not found, continue to other target families or raise `UnknownMethod`.

Verification tests:

```vela
struct Label {
    text: string,
}

impl Label {
    fn starts_with(self, prefix: string) -> bool {
        return self.text.starts_with(prefix);
    }
}

fn f(x) {
    return x.starts_with("q");
}
```

Expected:

- `f(Label { text: "quest" })` returns `true`.
- `f(Label { text: "raid" })` returns `false`.
- `f(42)` returns runtime error.
- Static `Label` receiver still uses resolved `CallMethodId`.

Add a heterogeneous array test:

```vela
fn main() {
    let values = [
        Label { text: "quest" },
        Label { text: "raid" },
        "quick",
    ];

    let count = 0;
    for value in values {
        if value.starts_with("q") {
            count += 1;
        }
    }

    return count;
}
```

Expected result: `2`.

Commit message:

```text
vm: resolve dynamic script methods in linked programs
```

After commit, change this checkbox to `[x]`.

---

## [ ] Phase 5 — Add host dynamic method resolution through registered metadata

Dynamic calls on `HostRef` must not bypass HostAccess or capability checks.

Add a host dynamic method lookup path using registered host metadata and stable `HostMethodId`.

Preferred architecture:

- Linker or engine builds a host method lookup table from the definition registry:
  - `(host type id/name, method name) -> HostMethodId`
- Runtime classifies `Value::HostRef` through the registered type registry.
- Dynamic resolver maps host receiver + method name to `HostMethodId`.
- Execution uses the existing host method call boundary, not direct reflection or raw adapter calls.
- Capability/effect checks remain enforced.
- Host schema epoch participates in cache guards.
- This support must remain separate from the Phase 3 std/value resolver until
  the HostAccess route, generation checks, permissions, and schema epoch guards
  are all explicit.

Verification tests:

- dynamic call on host ref succeeds when registered method exists
- missing host method gives source-spanned `UnknownMethod`
- permission/capability denial still works
- stale host ref/generation checks still work
- host dynamic resolution records the schema epoch or equivalent guard metadata
  needed by Phase 7 cache invalidation tests

Commit message:

```text
vm: resolve dynamic host methods through registered ids
```

After commit, change this checkbox to `[x]`.

---

## [ ] Phase 6 — Complete dynamic named/default argument semantics

Unknown-receiver dynamic calls must support the final language argument model, not just positional calls.

Implement runtime argument resolution for dynamic calls:

- Preserve source order and names in bytecode.
- After method target resolution, load the target signature.
- Reorder named args according to the resolved method signature.
- Fill defaults where the target declares defaults.
- Reject unknown named args with source-spanned diagnostics.
- Reject missing required args with source-spanned diagnostics.
- Apply existing type guards/contracts after argument materialization.

Phase 6 minimum support must cover script dynamic methods first, because script
method signatures and defaults are fully controlled by linked bytecode. Standard
and host dynamic named/default arguments should be included when their existing
metadata path can be reused without expanding this phase; otherwise they remain
Phase 9/follow-up coverage items.

This phase must work for:

- script impl methods

Verification tests:

```vela
struct Label {
    text: string,
}

impl Label {
    fn wrap(self, prefix: string = "[", suffix: string = "]") -> string {
        return prefix + self.text + suffix;
    }
}

fn f(x) {
    return x.wrap(suffix: "}", prefix: "{");
}
```

Expected:

```text
f(Label { text: "quest" }) => "{quest}"
```

Also test:

- omitted default args
- missing required arg
- unknown named arg
- named args on unsupported dynamic receiver
- named args on one std/value method where registry metadata already exists, if
  this can reuse the same argument materialization path without broadening the
  phase
- named args on one host method, if this can reuse the same argument
  materialization path without broadening the phase

Commit message:

```text
vm: resolve dynamic method arguments after target lookup
```

After commit, change this checkbox to `[x]`.

---

## [ ] Phase 7 — Add guarded dynamic method inline cache

Do not reuse the existing resolved method cache without adding receiver guards. Dynamic dispatch needs its own guarded entry or a clearly extended cache entry.
The first M20 implementation should be monomorphic: one guarded target per
dynamic call site. A small polymorphic inline cache is future work unless
benchmarks prove it is needed.

Recommended cache entry:

```rust
DynamicMethodInlineCacheEntry {
    method_name: DebugNameId,
    receiver_guard: DynamicReceiverGuard,
    target: DynamicMethodTarget,
    schema_epoch: Option<HostSchemaEpoch>,
    program_epoch: Option<...>,
}
```

Guard examples:

```rust
enum DynamicReceiverGuard {
    StdValue { kind: StandardMethodReceiver },
    ScriptType { type_name: DebugNameId, shape_id: Option<ShapeId> },
    HostType { type_id: ..., schema_epoch: HostSchemaEpoch },
}
```

Runtime behavior:

- On cache hit:
  - method name matches
  - receiver guard matches
  - host schema epoch matches where applicable
  - program image/hot reload epoch is valid
  - dispatch target is used directly
- On cache miss:
  - run resolver
  - populate guarded cache
- On guard mismatch:
  - do not error
  - fall back to resolver

Verification tests:

- Use the existing inline-cache test provider/counters or add a small test-only
  cache observer so cache hits and guard misses are directly asserted.
- same call site sees `String` then `String`: second call hits cache
- same call site sees `String` then `Label`: guard miss, resolves script method
- same call site sees `LabelA` then `LabelB`: guard miss with correct result;
  polymorphic caching is not required in this phase
- host schema epoch change invalidates host dynamic cache
- hot reload clears stale dynamic method cache entries
- undersized cache providers are rejected before execution, same as existing cache families

Commit message:

```text
vm: cache dynamic method dispatch with receiver guards
```

After commit, change this checkbox to `[x]`.

---

## [ ] Phase 8 — Remove legacy fallback and tighten link/runtime API

Now remove architecture debt.

Required cleanup:

- Delete or narrow old name-only unlinked method fallback paths that are no longer part of final semantics.
- Remove `LinkError::UnresolvedMethodName` if it only represented source-level dynamic method calls.
- Ensure source-level unknown receiver method calls never cause linked-program absence.
- Make program/runtime image construction either contain a valid `LinkedProgram`
  or return a construction/link error before execution starts.
- Do not keep `linked_program: Option<LinkedProgram>` as a normal success state if link failure would later become `ProgramNotLinked`.
- Keep `ProgramNotLinked` only for corrupt/internal/test-only states, or remove it if no longer useful.
- Update tests that expected link-time rejection of unknown-receiver method calls
  to expect runtime errors instead. Statically provable missing methods such as
  `42.trim()` may remain compile-time diagnostics.
- Do not refactor unrelated legacy helpers unless a dynamic method test still
  depends on them. Centralize only the dispatch paths needed by the final
  dynamic method architecture.

Verification tests:

- no test expects `LinkError::UnresolvedMethodName` for normal source code
- program/runtime image construction fails early on real link errors
- missing native implementation still fails as a link/build error
- unknown dynamic method fails at runtime with source span
- full workspace tests pass

Commit message:

```text
runtime: make linked program availability explicit
```

After commit, change this checkbox to `[x]`.

---

## [ ] Phase 9 — Update conformance, docs, progress, and benchmarks

Add conformance coverage for final dynamic method semantics:

- dynamic std method on unknown receiver
- dynamic script method on unknown receiver
- dynamic host method on unknown receiver
- heterogeneous array receiver dispatch
- missing method runtime error
- wrong receiver type runtime error
- named/default dynamic args
- cache guard miss correctness
- hot reload cache invalidation

Add a dynamic method coverage audit:

- every standard value method that has a resolved `CallMethodId` path must
  either resolve through dynamic dispatch or be listed as an explicit,
  documented exclusion
- linked script method lookup must cover all script impl methods in the linked
  program, not just the `Label.starts_with` fixture
- host dynamic lookup must cover all registered host methods visible to the
  current runtime/registry, subject to HostAccess permissions and capabilities

Update docs:

- language method-call docs
- bytecode/linker architecture docs
- progress/milestone note
- any playground examples affected by dynamic method behavior

Add or update benchmark rows:

- `dynamic_string_method_monomorphic`
- `dynamic_script_method_monomorphic`
- `dynamic_method_polymorphic`
- `dynamic_method_cache_miss`
- keep an existing static `CallMethodId` benchmark row so the fast path remains
  comparable

Verification:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Commit message:

```text
docs: finalize dynamic method dispatch coverage
```

After commit, change this checkbox to `[x]`.

---

# Final acceptance criteria

All of these must be true before the goal is complete:

- [ ] Static known receiver calls still compile to resolved `CallMethodId`.
- [ ] Static known receiver calls to provably missing methods may remain compile-time errors.
- [ ] Unknown receiver calls compile to explicit dynamic method bytecode.
- [ ] Linked bytecode accepts dynamic method calls.
- [ ] Dynamic std/value methods work at runtime.
- [ ] Dynamic script impl methods work at runtime.
- [ ] Dynamic host methods work through registered IDs and HostAccess.
- [ ] Dynamic method coverage audit shows no registered std/value, script, or
      host method family is accidentally omitted.
- [ ] Missing methods produce source-spanned runtime errors.
- [ ] Wrong receiver types produce source-spanned runtime errors.
- [ ] Named/default args work after dynamic method target resolution.
- [ ] Dynamic dispatch cache is guarded by receiver type/shape/schema epoch as applicable.
- [ ] Hot reload invalidates stale dynamic method cache entries.
- [ ] No normal source-level dynamic method call can cause `ProgramNotLinked`.
- [ ] No test relies on link-time rejection of unknown receiver method calls.
- [ ] Full workspace fmt, clippy, and tests pass.

## Final expected examples

```vela
fn f(x) {
    return x.starts_with("q");
}
```

```text
f("quest") => true
f("raid")  => false
f(42)      => runtime error at `.starts_with(...)`
```

```vela
struct Label {
    text: string,
}

impl Label {
    fn starts_with(self, prefix: string) -> bool {
        return self.text.starts_with(prefix);
    }
}

fn main() {
    let values = [
        "quest",
        Label { text: "raid" },
        42,
    ];

    for value in values {
        value.starts_with("q");
    }
}
```

Expected: the string and `Label` receiver calls dispatch dynamically; the integer receiver fails at runtime with a clear source-spanned method/type error.
