# Rust/Vela Interop Acceptance — 2026-07-17

> **Status correction:** a post-implementation review reopened the optional
> replaceable-dispatch portion of the plan. This report remains evidence for
> the tests, audits, examples, and benchmarks that passed, but its final
> completion conclusion is superseded by
> [`rust-vela-interop-post-review-2026-07-17.md`](rust-vela-interop-post-review-2026-07-17.md).

This report records the original acceptance-run evidence for
[`rust-vela-interop-model-plan.md`](../rust-vela-interop-model-plan.md). The
plan's durable product contract remains in the main document; this archive
keeps the test, audit, and measurement detail out of `progress.md`.

## Acceptance evidence

| Contract area | Primary evidence |
|---|---|
| Ordinary Rust exports, inferred/additional effects, module grouping, inherent methods, and registration | `vela_macros::interop_exports` and macro UI fixtures |
| Local and external trait exports | `explicit_trait_impl_exports_install_ufcs_method_thunks`, `declaration_only_external_trait_adapter_calls_existing_impl`, and protocol metadata fixtures |
| Exact identity, alias preflight, conversion rollback, and cleanup | `host_exports_acquire_distinct_exclusive_arguments_and_write_through`, `host_exports_allow_two_shared_aliases`, `host_exports_reject_mixed_aliases_before_authored_rust_runs`, and the authored error/panic/conversion-failure fixtures |
| Borrowed-return owner freezing and escape rules | shared/exclusive, Option/Result, tuple-sibling, state/root escape, and root-cleanup fixtures in `vela_macros::interop_exports` and MIR validation tests |
| Automatic and explicit early release | lexical/branch last-use, before-await, resume-after-await, alias-group `host::release`, expired-reference, and bare-release rejection fixtures |
| Async lease retention and cancellation | async function/method lease tests and future-drop cleanup tests in `interop_exports_async_and_traits` |
| Generated Rust-to-Vela bindings | `vela_engine::binding` schema/source diagnostics and stable-ID tests, `vela_bindgen` deterministic generation tests, and the bindgen compile fixture |
| Nested reborrow and effect ceiling | active-binding same-session re-entry, canonical-provenance reborrow tests, unrelated-pointer and shared-to-exclusive rejection tests, nested capability/effect-ceiling denial tests |
| Policy versus ABI and reload | callable fingerprint tests, binding body-reload compatibility, parameter/mode/return/effect/async mismatch tests, and Runtime policy-denial tests |
| Optional override dispatch | dispatch activation, rollback, old-root pinning, arbitrary partial delta, receiver method, no-fallback-on-error, and source-backed incompatibility tests |
| End-to-end authoring | `interop_round_trip`, `replaceable_handler`, and `replaceable_service_method` runnable example tests |

## Public-surface audit

- The ordinary documented path is generated build-time bindings. Its Rust call
  site contains no authored `HostRef`, `CallArgs`, `OwnedValue`, proxy, lease,
  or runtime target string.
- Low-level dynamic APIs remain available for reflection, generic tooling, and
  intentionally dynamic examples, but are presented after the generated path.
- Rust exports use ordinary values, `&T`, `&mut T`, `&self`, and `&mut self`.
  Generated adapters own all boundary conversion and lease ceremony.
- `&mut T` is documented as trusted field-level Rust authority for that
  invocation; direct Vela path writes still use fine-grained `HostAccess`.

## Architecture audit

- `crates/vela_macros/src/export/signature.rs` is the sole Rust export
  signature classifier used by functions, modules, methods, traits, external
  trait adapters, and replaceable entries.
- No ambient inventory, linker-section, constructor, or process-global export
  discovery is used. Hosts register generated bundles explicitly.
- Linked ordinary and override calls use stable identities and prepared
  targets. Runtime strings remain only in explicitly dynamic APIs and source
  metadata/diagnostics, not the generated linked call path.
- Callable and binding fingerprints contain normalized ABI/effects, not live
  Runtime grants, allowlists, or arbitrary business permission strings.
- Only `host::release` is reserved; a bare `release` call has a negative test.
- Borrow leases are released deterministically by last-use lowering, explicit
  release, and root/future cleanup. GC timing is not a correctness condition.
- The implementation has one VM `ExecutionSession`; generated root and active
  bindings are authority carriers over that execution path, not duplicate
  interpreters or budget models.
- The file-size audit has no unreviewed active source above 1200 lines.
  Cohesive macro emission and test responsibilities were split, and remaining
  exceptions are listed in `architecture/file-size-exceptions.md`.
- No new unbudgeted loop, retry, fallback replay, or owner-lifetime path was
  introduced.

## Reproducible benchmark checkpoint

Command:

```bash
cargo bench -p vela_engine --bench interop -- --quick
```

Apple Silicon debug-host checkpoint, 1,000 iterations per row:

| Row | ns/call |
|---|---:|
| direct Rust scalar | 0.5 |
| Vela-to-Rust scalar | 13,983.4 |
| Vela-to-Rust shared host | 13,410.5 |
| Vela-to-Rust exclusive host | 20,158.8 |
| generated Rust-to-Vela root | 11,469.2 |
| Vela-Rust-Vela round trip | 13,564.9 |
| replaceable empty-slot fallback | 2.7 |
| replaceable local override hit | 2,852.8 |
| partial stage/activate/first call | 3,001.1 |

These rows establish the baseline; no optimization was accepted without a
measured regression. The benchmark target also compiles under the full
all-features bench-build gate.

## Final gates

The completion commit was validated with the commands in `docs/validation.md`:
root formatting, all-feature workspace clippy/tests/docs, all-feature examples
clippy/tests, runnable examples, benchmark compilation, parser fuzz-target
compilation, and documentation-site syntax/docs/build checks.
