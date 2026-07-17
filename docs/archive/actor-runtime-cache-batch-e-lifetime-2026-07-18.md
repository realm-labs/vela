# Actor Runtime Cache Batch E Lifetime Acceptance — 2026-07-18

## Result

Batch E is accepted. Fresh reload generations own fresh execution data; old
frames, closures, suspended calls, and pinned dispatch roots keep exact old
generation behavior; dead execution data is reclaimed at the first ordinary
safe point after its last owner disappears. Actor mutable state remains
isolated while eligible metadata is shared. Failure, cancellation, drop, and
panic paths retain no cache lock, Actor authority, lease, or permanent old
generation owner.

## Proof matrix

| Gate | Proof |
| --- | --- |
| E1 fresh publication | Cache reload tests show empty new record, dynamic-method, native, host-access, callback, iterator, and linked-state views. Profile reload tests show a distinct generation with zeroed counters. Immutable linked method dispatch is republished with the new artifact. |
| E2 exact old view | A retained closure executes after reload, populates only its old method cache, increments only its old aggregate profile, and leaves the active cache untouched. Existing nested-frame, suspended-async, removed-state closure, provider, and pinned-dispatch tests retain old code and generation selection. |
| E3 reclamation | The old execution-data `Weak` token remains live while the closure owns its artifact, then becomes dead after the closure is dropped and one `check_reload()` safe point runs with no pending update. The Actor generation count falls from two to one; the still-live Engine registry does not retain the old object. |
| E4 Actor isolation | Two `SharedRuntime` Actors have distinct Runtime IDs, heaps, root registries, extern bindings, persistent state, and retained-value authority while their execution-data `Arc` is pointer-equal. A retained value from one Runtime is rejected by the other. Pending override Actors preserve independent state while executing the same shared generation. |
| E5 failure and unwind | Failed dynamic resolution publishes no partial entry and the same slot subsequently populates successfully. A native panic after target caching unwinds, then the same Runtime and cache slot execute successfully. Existing suspended-call cancellation, unpolled-future drop, native error, lease rollback, and replacement panic tests prove authority and leases are released. |

Per-site cache read/write guards are scoped entirely to copying or replacing an
`Option<T>`. Generic resolution, host access, native invocation, suspension,
and callbacks run after the guard is dropped. Poison recovery is explicit for
both read and write guards; no script or host callback executes while a cache
lock is held.

## Validation

```text
cargo test -p vela_engine --all-features --no-fail-fast
  563 unit tests passed
  10 args integration tests passed
  8 hot-reload integration tests passed
  4 prelude integration tests passed
  2 reflection integration tests passed
  6 doctests passed
```

The run includes the existing nested async dispatch cancellation/pinning,
pending-Actor overlap, panic/unpolled drop, lease rollback, provider old-frame,
reload safe-point, and Runtime reuse coverage alongside the new exact cache,
profile, isolation, failure, unwind, and weak-reclamation proofs.
