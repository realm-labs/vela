# Rust/Vela Interop Post-Implementation Review — 2026-07-17

This review supersedes only the completion conclusion of
[`rust-vela-interop-acceptance-2026-07-17.md`](rust-vela-interop-acceptance-2026-07-17.md).
The original report remains valid evidence that its listed tests, examples,
audits, and benchmarks passed. The repository-wide gates also remained green
during this review.

## Corrected status

Ordinary Rust/Vela exports, generated typed bindings, HostRef/lease safety,
borrowed-return handling for ordinary exports, early release, and
NativeCallContext same-session re-entry remain accepted. The optional
replaceable-dispatch extension is reopened and the unified plan is not complete
until its review findings are closed.

## Findings

### 1. Replaceable invocation creates a separate Runtime execution

`VelaOverrideTarget` owns `Arc<parking_lot::Mutex<Runtime>>` and
`DispatchInvocation::{call, call_async}` locks it before calling
`Runtime::{call, call_async}`. An override that reaches another replaceable
entry in the same Runtime attempts to reacquire the same non-reentrant lock.
The synchronous path can deadlock, while both paths bypass the active
same-session re-entry authority and serialize otherwise independent roots that
share the target Runtime.

Each hit also constructs fixed default `CallOptions`, which replenishes
execution, memory, and depth budgets and does not inherit the caller's heap,
state view, HostAccess, effect ceiling, tracing, cancellation, lease
provenance, or pinned linked artifact. This violates fixed constraints 13-15
and the never-complete condition forbidding a second execution or budget path.

### 2. Dispatch generations have no controller identity

`stage_from` checks only target-table length. `activate` and `rollback` accept
any public `DispatchCandidate` or `Arc<DispatchGeneration>` without proving
that it came from the same controller or slot layout. A candidate from one
controller can therefore be installed into another controller with the same
slot count. The stored `VelaOverrideTarget::slot` is not checked during target
lookup or invocation.

### 3. Staging validates a lossy projection of the callable contract

The current validator compares capability equality, treats shared and
exclusive host modes as interchangeable, and compares only the return type.
It does not compare the normalized effect ceiling, return/error mode, or
borrowed-return origin/freeze/access contract. Capability equality also rejects
a valid implementation that uses a strict subset of the target effects, while
different effects with the same capability projection can be accepted.

### 4. Replaceable returns are restricted to `VmResult<T>`

The replaceable macro rejects ordinary returns, boundary-safe
`Result<T, E>`, and business result aliases before reusing the ordinary
callable adapter. A `VmResult<&T>` expansion attempts
`DispatchInvocation::call::<&T>`, but `FromScriptArg` intentionally converts
owned boundary representations such as `HostRef`; it cannot safely manufacture
a Rust reference. The supported ordinary borrowed-return lease path is not
connected to replacement.

### 5. Override targets are resolved as staging-time strings

The compiler stores the attribute path as `Option<String>`. Staging resolves it
through a path-to-index map, so an unknown host target can finish Vela
compilation and fail only while creating a candidate. The target
`CallableContract` is not imported to infer and validate the override
signature during semantic compilation/linking.

### 6. The promised host-business-macro authoring surface is not demonstrated

The public examples use the low-level replaceable attribute directly, require
authored `path`, `authority`, and dense `index`, manually collect slot
descriptors, and manually construct a controller. This is useful mechanism
coverage but does not prove the intended Handler/Service business integration
in which the host macro generates those details and callers retain their
ordinary call shape.

## Validation observed during review

The following commands passed:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features --no-fail-fast
cargo test -p vela_engine dispatch::tests -- --nocapture
cargo run --manifest-path examples/Cargo.toml --bin replaceable_handler
cargo run --manifest-path examples/Cargo.toml --bin replaceable_service_method
```

The green result establishes a stable baseline. It does not close the findings
because the existing suite lacks the nested/session, ownership, complete ABI,
return-family, borrowed-return, and business-macro cases named above.
