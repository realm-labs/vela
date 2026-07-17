# Rust/Vela Interop Actor Runtime Authority Review — 2026-07-17

This review records a cross-document and code mismatch discovered after
[the post-review acceptance report](rust-vela-interop-post-review-acceptance-2026-07-17.md)
was published. It reopens only the optional replacement invocation-authority
conclusion. Ordinary interop and the unaffected replacement generation,
linking, contract, return, macro, activation, rollback, and no-retry proofs
remain accepted baseline.

## Finding

The accepted architecture is one logical Vela Runtime per Actor. The Actor
mailbox or equivalent turn already supplies exclusive access, so ordinary and
replacement execution borrow the Actor's mutable Runtime directly. Neither the
Actor Runtime nor an override target may use `Arc<Mutex<Runtime>>` as the
execution boundary.

The accepted implementation does not yet satisfy that contract:

- `vela_engine::dispatch` defines `SharedDispatchRuntime` as
  `Arc<Mutex<SharedRuntime>>`;
- `DispatchRoot` and `DispatchInvocation` retain that mutable Runtime owner;
- sync replacement locks the Runtime before `Runtime::call`;
- async replacement retains the mutex guard across the scoped call future;
- the documented and generated business fixtures construct and clone this
  lock-based authority.

The focused `independent_roots_share_code_without_sharing_a_runtime_lock` test
proves that two roots over one immutable image have independent Runtime locks.
The nested sync/async tests prove same-session re-entry, remaining-budget and
generation inheritance, and cancellation release. Those are valuable baseline
proofs, but they establish independent locks rather than the required absence
of an Actor Runtime lock.

## Cause

The Actor-owned Runtime architecture and cache-ownership documents were not in
the execution context used for the preceding interop correction. The work
therefore optimized the earlier root-owned `SharedRuntime` model and the final
acceptance report compared the result against that stale ownership assumption.
This is a plan/acceptance gap, not permission to weaken the later architecture
to match the implementation.

## Required correction

The current execution owner is the
[Rust/Vela interop model plan](../rust-vela-interop-model-plan.md#post-acceptance-actor-runtime-authority-reconciliation--2026-07-17).
Its `I-RECON-1..6` tasks must:

1. freeze one actor-turn-scoped sync/async invocation authority;
2. hard-switch all production replacement callers and generated entries;
3. delete `SharedDispatchRuntime` and the mutable Runtime fields and lock-based
   call paths it supports;
4. preserve same-session policy, generation, state, return, and error behavior;
5. prove overlapping Actors, pending async isolation, state isolation, nested
   budget/generation inheritance, cancellation, panic, and dropped futures;
6. rerun complete acceptance and publish a new reconciliation report.

The correction must not add an ambient or process-global Runtime, a second
mutable Runtime, a reentrant Runtime mutex, compatibility adapters, dual
execution paths, fresh nested budgets, or business-facing Runtime/session
plumbing.

## Milestone impact

- Ordinary Rust/Vela interop remains accepted.
- Optional replacement is reopened until `I-RECON-1..6` pass.
- State-storage Batch G remains accepted.
- The Actor Runtime/cache execution plan is queued; Batch A must not begin
  until a new interop reconciliation acceptance report closes Gate I.
- M20.5 remains queued after M20.

This review changes documentation status only. It does not claim that the
implementation correction or its final validation has completed.
