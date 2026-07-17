# Rust/Vela Interop Actor Runtime Reconciliation Acceptance — 2026-07-17

This report closes `I-RECON-1` through `I-RECON-6` in the
[Rust/Vela unified interop plan](../rust-vela-interop-model-plan.md). It
supersedes the optional-replacement authority finding in the
[Actor Runtime authority review](rust-vela-interop-actor-runtime-review-2026-07-17.md).
Ordinary interop, Batches A-G, the first post-review correction, and the
unaffected replacement generation, linking, contract, return, activation,
rollback, and no-retry proofs remain accepted baseline and were not reopened.

## Closure result

| Task | Accepted correction and proof |
|---|---|
| I-RECON-1 | `DispatchRoot` owns only an immutable `DispatchGeneration` and `CallOptions`. `DispatchInvocation<'turn>` borrows `&'turn mut SharedRuntime` from the Actor turn. Nested replacement through `NativeCallContext` re-enters the active `ExecutionSession`. |
| I-RECON-2 | All production generated entries, handwritten fixtures, examples, and the benchmark use the borrowed authority. `SharedDispatchRuntime`, root/invocation Runtime ownership, lock-based constructors, and compatibility paths were deleted in checkpoint `af2942012`. |
| I-RECON-3 | Existing sync/async nested-call tests retain the active artifact, generation, heap/state view, HostAccess, remaining budget, effect ceiling, capabilities, tracing, cancellation, leases, return mapping, and error behavior. No replacement failure retries the Rust fallback. |
| I-RECON-4 | The p9-shaped Handler/Service fixture splits `P9Actor` into a hidden `P9Turn` and script-visible `P9Context`. Business traits still take `&mut P9Actor`; authors supply no Runtime, session, HostRef, lease guard, path, or dense slot. Fallback calls mutate the context to 20 and active Vela overrides mutate it to 2. |
| I-RECON-5 | Structural scans find no Actor Runtime mutex in production dispatch. Two Actor turns borrow separate Runtimes over one immutable generation, a pending Actor does not block another, their persistent Vela state remains isolated, and cancellation, unpolled drop, and panic unwind release scoped authority. |
| I-RECON-6 | Focused interop, full workspace, examples, runnable replacement, docs/site, benchmark build/run, fuzz build, editor grammar, and safe-Rust audit gates pass. This report closes Gate I. |

## Final authority model

The Actor mailbox or equivalent host turn owns the only mutable Runtime for
that Actor. A root pins immutable dispatch selection. When a generated entry
hits an override, its hidden named authority lends the current Actor Runtime to
one scoped invocation:

```text
Actor turn owns &mut Runtime
  -> generated replaceable entry reads immutable DispatchRoot
  -> DispatchInvocation<'turn> borrows &mut Runtime
  -> root Runtime::call / Runtime::call_async
  -> nested NativeCallContext re-entry uses the active ExecutionSession
```

The selected override target owns linked identity and immutable deployment
selection, never a Runtime. There is no Actor Runtime mutex, ambient Runtime,
second mutable Runtime, compatibility adapter, or dual execution path. The
scoped future retains the Actor turn borrow across suspension and releases it
through normal safe-Rust drop or unwind.

## Structural and behavioral audit

The production audit used exact searches for `SharedDispatchRuntime`,
`Arc<Mutex<SharedRuntime`, `Mutex<SharedRuntime`, Runtime-bearing dispatch
roots, and added `unsafe`. The former authority has no source match. Mutexes
that retain internal `RuntimeValueRoots` are not Runtime execution boundaries
and remain outside this correction. The implementation diff adds no `unsafe`,
and `DispatchRoot` remains `Clone` because it contains immutable generation
selection and call options only.

The focused proof matrix passes:

- `independent_actor_turns_overlap_on_one_immutable_generation` keeps one
  Runtime mutably borrowed while another Actor invokes the same generation.
- `pending_actors_overlap_and_keep_vela_state_isolated` suspends Actor A,
  completes Actor B, then proves per-Actor persistent state values `1`, `1`,
  and `2`.
- `nested_replaceable_call_reenters_the_active_runtime_session` proves nested
  same-session execution.
- `nested_replaceable_call_consumes_the_root_remaining_budget` proves no fresh
  nested budget.
- `nested_async_replaceable_calls_pin_generation_and_release_on_cancel` proves
  generation inheritance and cancellation release.
- `panic_and_unpolled_drop_release_actor_turn_authority` proves release after
  an unpolled future drop and native panic unwind, then calls the same Runtime
  successfully.
- `host_business_macro_hides_slots_authority_and_handler_proxy_plumbing`
  proves the natural Handler/Service authoring surface and script-visible
  business context.

The active file-size audit places reconciliation-only tests in
`dispatch/reconciliation_tests.rs` (109 lines) and keeps
`dispatch/tests.rs` at 1,159 lines. No cache/profile ownership file changed in
the implementation checkpoints.

## Runnable examples

The replacement examples passed and printed:

```text
replaceable_handler fallback=41/10 active=42/0 adjacent=41 rollback=41/10
replaceable_service_method fallback=41 active=42 adjacent=41 rollback=41
```

The low-level handler example deliberately hides its authority context from
the Vela ABI. The p9 business-macro proof separately demonstrates visible
business-context mutation while retaining hidden Actor Runtime authority.

## Reproducible benchmark checkpoint

Apple Silicon macOS release-profile quick runs, 1,000 iterations per row:

| Benchmark | ns/operation |
|---|---:|
| empty-slot Rust fallback | 2.6 |
| local Vela override | 2,655.9 |
| partial stage/activate/first call | 4,620.0 |
| VM `vm_state_read_write` mean | 47,187 |

The VM row reported min 46,708 ns, median/p95 47,667 ns, the expected
checksum, and no cache activity. These host-specific values are reproducible
checkpoints, not cross-host regression claims. The empty slot retains its
dense indexed lookup and predictable branch shape.

## Final validation

The following gates passed on the reconciled implementation:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features --no-fail-fast
cargo clippy --manifest-path examples/Cargo.toml --all-targets --all-features -- -D warnings
cargo test --manifest-path examples/Cargo.toml --all-features --no-fail-fast
cargo run --manifest-path examples/Cargo.toml --bin replaceable_handler
cargo run --manifest-path examples/Cargo.toml --bin replaceable_service_method
cargo bench --workspace --all-features --no-run
cargo doc --workspace --all-features --no-deps
cargo check --manifest-path fuzz/Cargo.toml --bins
cargo bench -p vela_vm --bench baseline -- vm_state_read_write --quick
cargo bench -p vela_engine --bench interop -- --quick
node editors/vscode/scripts/validate-package.js
(cd editors/tree-sitter-vela && npx --yes tree-sitter-cli@0.25.10 generate)
git diff --exit-code -- editors/tree-sitter-vela/src
npm --prefix site run test:syntax
npm --prefix site run test:docs
npm --prefix site run build
```

The site build completed with zero Astro errors, warnings, or hints; Vite kept
its existing chunk-size advisory. Tree-sitter regeneration produced no tracked
source diff. Miri is not distributed for the installed stable
`aarch64-apple-darwin` toolchain, so `cargo miri --version` reports the
component unavailable. The relevant boundary remains safe Rust, its diff adds
no `unsafe`, and the drop/cancellation/unwind behavior is exercised directly.

## Final status

`I-RECON-1` through `I-RECON-6` are complete. Optional replacement again
meets the production interop contract, Gate I is closed, and M20 may proceed
with the separate
[Actor Runtime/cache execution plan](actor-runtime-cache-execution-plan.md).
No cache/profile ownership work began during this reconciliation.
