# Rust/Vela Interop Post-Review Acceptance — 2026-07-17

> **Superseded in part:** the later
> [Actor Runtime authority review](rust-vela-interop-actor-runtime-review-2026-07-17.md)
> reopens the optional replacement authority conclusion. This report remains
> historical evidence for ordinary interop and the unaffected replacement
> generation, linking, contract, return, macro, activation, rollback, and
> no-retry proofs; it is not current final acceptance for optional replacement.

This report originally closed the correction tasks in
[`rust-vela-interop-model-plan.md`](../rust-vela-interop-model-plan.md) and
supersedes the incomplete completion conclusion recorded by
[`rust-vela-interop-post-review-2026-07-17.md`](rust-vela-interop-post-review-2026-07-17.md).
The original
[`rust-vela-interop-acceptance-2026-07-17.md`](rust-vela-interop-acceptance-2026-07-17.md)
remains the acceptance evidence for ordinary interop. This report adds the
missing production proof for optional replaceable dispatch.

## Closure result

All findings from the post-implementation review are closed:

| Review finding | Accepted correction and proof |
|---|---|
| Separate Runtime execution and nested deadlock | Root-owned `SharedRuntime` instances execute over an immutable `SharedImage`; active sync and async override calls re-enter the current `ExecutionSession`. `nested_replaceable_call_reenters_the_active_runtime_session`, `independent_roots_share_code_without_sharing_a_runtime_lock`, and `nested_async_replaceable_calls_pin_generation_and_release_on_cancel` pass. |
| Fresh budgets and lost execution policy | Nested calls inherit the active artifact, heap, state view, HostAccess, capabilities, effect ceiling, tracing, cancellation, and remaining budgets. `nested_replaceable_call_consumes_the_root_remaining_budget` proves that a nested hit cannot replenish the root budget. |
| Controller-free dispatch generations | Candidates and generations carry opaque controller/layout identity. `same_shaped_controllers_reject_foreign_generations_and_candidates` rejects cross-controller staging, activation, and rollback even for equal slot counts. |
| Lossy override contract validation | Engine compilation resolves override declarations to stable registered slots and imports the complete callable contract. Staging compares parameter and boundary modes, return/error and borrowed-return families, asyncness, types, and normalized effects while allowing only an implementation effect subset. Compilation, coherent-artifact, and effect-subset regression tests pass. |
| Restricted return mapping | Replaceable entries reuse the ordinary generated return adapter for plain values, boundary-safe business `Result`, and direct-origin borrowed returns inside supported `Option`, `Result`, and tuple containers. Invalid projection provenance and conflicting exclusive aliases are rejected without fabricating Rust references. |
| Manual business integration | `#[methods]` emits deterministic replaceable slot bundles. The representative `host_business_macro_hides_slots_authority_and_handler_proxy_plumbing` fixture generates paths, authority, dense indices, registration, and trait forwarding while preserving ordinary Handler and Service call shapes and adjacent Rust methods. |

The corrected implementation therefore has one execution, budget, artifact,
contract, and return-conversion model for ordinary and replaceable calls. A
Vela error propagates without retrying the displaced Rust body, and active
roots remain pinned while activation changes only future roots.

## Public and architecture audit

- Ordinary Rust/Vela call sites remain generated and typed. Business
  signatures do not mention `HostRef`, `PathProxy`, lease guards,
  `CallArgs`, or `OwnedValue`.
- The explicit low-level replaceable attribute remains macro-generation
  machinery; the domain-neutral fixture proves that business authors need not
  assign paths, authority, indices, bundles, or proxies by hand.
- No `Mutex<Runtime>` execution path, ambient process-global Runtime,
  duplicate interpreter, fresh nested budget, fallback replay, or new
  `unsafe` block was introduced.
- The active file-size audit found no new unreviewed source file above 1200
  lines. Existing exceptions remain listed in
  `docs/architecture/file-size-exceptions.md`.
- Tree-sitter regeneration produced no tracked diff, and the final worktree
  was clean before this report was written.

## Runnable examples

The public examples passed and printed:

```text
replaceable_handler fallback=41/10 active=42/1 adjacent=41 rollback=41/10
replaceable_service_method fallback=41 active=42 adjacent=41 rollback=41
```

These outputs prove fallback, activated override, adjacent direct-Rust
behavior, and rollback through the documented group bundle surface.

## Reproducible benchmark checkpoint

Command:

```bash
cargo bench -p vela_engine --bench interop -- --quick
```

Windows x86_64 release-profile checkpoint, 1,000 iterations per row:

| Replaceable row | ns/call |
|---|---:|
| empty-slot fallback | 3.9 |
| local override hit | 6,006.2 |
| partial stage/activate/first call | 9,287.8 |

These host-specific numbers confirm that the benchmark remains runnable and
that the empty-slot path retains its dense lookup/branch shape. They are not a
cross-host regression comparison with the earlier Apple Silicon checkpoint.

## Final validation

The following gates passed on the corrected implementation:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features --no-fail-fast
cargo clippy --manifest-path examples/Cargo.toml --all-targets --all-features -- -D warnings
cargo test --manifest-path examples/Cargo.toml --all-features --no-fail-fast
cargo run --manifest-path examples/Cargo.toml --bin replaceable_handler
cargo run --manifest-path examples/Cargo.toml --bin replaceable_service_method
cargo doc --workspace --all-features --no-deps
cargo bench --workspace --all-features --no-run
cargo check --manifest-path fuzz/Cargo.toml --bins
cargo bench -p vela_engine --bench interop -- --quick
node editors/vscode/scripts/validate-package.js
(cd editors/tree-sitter-vela && npx --yes tree-sitter-cli@0.25.10 generate)
git diff --exit-code -- editors/tree-sitter-vela/src
npm --prefix site run test:syntax
npm --prefix site run test:docs
npm --prefix site run build
```

The MSVC linker emitted its informational import-library message during some
builds; Rust clippy still passed with `-D warnings`. The site build completed
with zero Astro errors, warnings, or hints; Vite retained its existing chunk
size advisory.

## Final status

At the time of this report, `F-REVIEW-1` through `F-REVIEW-7` and
`G-REVIEW-1` through `G-REVIEW-2` were recorded complete. The later Actor
Runtime authority review supersedes the optional replacement final-acceptance
conclusion and opens `I-RECON-1..6`. Ordinary interop remains accepted.
