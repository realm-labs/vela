# Service Hard Switch S0 Deletion Inventory

This inventory freezes the callable-slot implementation before S1 removes it.
It was prepared from `master` at parent commit `34a74ef0c`. The active target is
the unified service-generation model in
[`rust-vela-service-hard-switch-plan.md`](../rust-vela-service-hard-switch-plan.md).
No item below is an extension point.

## Ownership And Replacement Matrix

| Area | Current files | S1 owner and action | Later replacement or retained fact |
|---|---|---|---|
| Stable slot identities | `crates/vela_common/src/lib.rs` | `vela_common`: delete `ReplaceableSlotId` and `InterceptSlotIndex` | S4 introduces semantic `ServiceId` and `ServiceMethodId`; neither reuses a dense authored slot index. |
| Vela override schema and linking | `crates/vela_bytecode/src/binding_schema.rs`, `crates/vela_bytecode/src/binding_schema/tests.rs` | `vela_bytecode`: delete override targets, resolution, checksum input, and parser/link diagnostics | Ordinary callable contracts remain. S5 links sparse `service_impl` declarations against imported service schemas. |
| Runtime slot dispatch | `crates/vela_engine/src/dispatch.rs`, `crates/vela_engine/src/dispatch/tests.rs` | `vela_engine`: delete slot descriptors, tables, controller, root, target lookup, staging, activation, and slot-specific tests | `ProgramVersion`, linked artifact ownership, no-retry semantics, and conditional whole-generation publication remain neutral facts; S4-S5 add service generations without copying the slot table. |
| Actor Runtime authority and re-entry | `crates/vela_engine/src/context.rs`, slot-specific portions of `crates/vela_engine/src/dispatch.rs` | `vela_engine`: remove `DispatchAuthority`, `DispatchInvocation`, and override-target entry methods; retain the existing `NativeCallContext` and same-session Runtime path | S4 generated service invocation receives explicit root authority; S5 nested calls reuse the active session and pinned service generation. |
| Borrowed return reconstruction | `crates/vela_engine/src/dispatch/returning.rs`, `crates/vela_engine/src/dispatch/returning_tests.rs` | `vela_engine`: move reusable provenance/container validation under neutral interop ownership before deleting slot-specific names and diagnostics | S2-S5 generated typed thunks reuse owner-frozen, root-scoped borrowed-return validation. |
| Engine registration and source plumbing | `crates/vela_engine/src/builder.rs`, `crates/vela_engine/src/engine.rs`, `crates/vela_engine/src/error.rs`, `crates/vela_engine/src/source.rs` | `vela_engine`: delete slot storage, `register_replaceable_slots`, layout validation, override linking, and slot-specific errors | Ordinary export registration stays. S2 seals `TypeBinding`; S4 registers service bundles; S5 service-link failures use service-specific structured diagnostics. |
| Replaceable proc macro | `crates/vela_macros/src/lib.rs`, `crates/vela_macros/src/export/mod.rs`, `crates/vela_macros/src/export/replaceable.rs`, `crates/vela_macros/src/export/signature.rs` | `vela_macros`: delete the public attribute, body rewriting, index/path parsing, fallback emission, and replaceable-only classifiers | Ordinary export signature classification stays. S2 adds type-binding derives; S4 adds `service` and `service_set` generation without rewriting authored default bodies. |
| Method-group integration | `crates/vela_macros/src/methods.rs` | `vela_macros`: remove `#[replaceable]` extraction, rewritten methods, and generated slot bundles while retaining ordinary exported methods | S4 service traits emit one service registration bundle and hidden dispatch surface. |
| Macro compile fixture | `crates/vela_macros/tests/ui/interop/pass/replaceable_entries.rs` | `vela_macros`: delete from the UI pass set in S1 | S4 adds service macro pass/fail UI coverage for supported and rejected signatures. |
| Engine behavior fixtures | `crates/vela_engine/src/dispatch/business_macro_tests.rs`, `crates/vela_engine/src/dispatch/reconciliation_tests.rs`, `crates/vela_engine/src/dispatch/returning_tests.rs`, `crates/vela_engine/src/dispatch/tests.rs` | `vela_engine`: delete slot authoring tests after retaining neutral lease/re-entry/return regressions in their owning modules | S4-S6 recreate only service-generation acceptance cases: Rust defaults, sparse patching, nested generation pinning, async cancellation, borrowed returns, and rollback. |
| Runnable examples | `examples/src/bin/replaceable_handler/**`, `examples/src/bin/replaceable_service_method/**`, their entries in `examples/tests/runnable_examples.rs`, and historical entries in `examples/README.md` | `examples`: delete both bins and test expectations in S1 | `service_hard_switch_fixture` is the S0 Rust-default baseline and becomes the generated service example in S4-S7. |
| Benchmarks | slot portions of `crates/vela_engine/benches/interop.rs`, `actor_concurrency.rs`, and `actor_memory.rs` | `vela_engine`: freeze existing results, then delete slot setup and rows in S1 without extending them | Ordinary interop, Actor isolation, pending overlap, allocation, and generation lifetime facts remain. S4-S7 add direct trait, Rust-default service, active Vela, nested service, and whole-generation rows. |
| Active authoring docs | `docs/rust-vela-interop.md`, `docs/rust-vela-interop-model-plan.md`, `docs/performance.md`, `docs/decisions.md`, `docs/progress.md`, `examples/README.md` | `docs`: retain historical explanations only where clearly marked superseded/frozen; remove deleted API guidance at S1 | `docs/rust-vela-service-hard-switch-plan.md` and future service authoring docs are authoritative. Archived reports remain unchanged. |

## S0 Fixture Coverage

`examples/src/bin/service_hard_switch_fixture/main.rs` is the migration fixture.
At S0 it intentionally contains no Vela dependency, patch branch, target
string, slot, `CallArgs`, `HostRef`, or manual boundary adapter. It covers:

- `InventoryService` and `RewardService` plus an object-safe async
  `GrantHandlerService` baseline;
- one mutable `HostActor` reached through a root `HostTurn`;
- owned DTOs with nested `Vec` and `BTreeMap` values;
- borrowed slice and map service arguments;
- `ServiceResult` propagation;
- handler to inventory to reward nested service calls; and
- an adjacent Rust-only inventory method used after the async root completes.

S4 may replace the temporary boxed-future dispatch spelling with the generated
hidden async surface, but the authored business behavior and deterministic
output remain the fixture contract.

## Frozen Performance Evidence

The existing historical numbers remain in `docs/performance.md` and the
2026-07-17/2026-07-18 archive reports. The current `interop` benchmark owns the
direct Rust, ordinary Rust/Vela boundary, generated Rust-to-Vela, same-session
re-entry, empty-slot, active-slot, and staging rows. `actor_memory` and
`actor_concurrency` own Actor isolation, allocation, pending overlap, and
generation-sharing rows.

S0 still requires dedicated frozen measurements for compact HostRef alias
copy, prepared static path access, registered methods, representative atomic
argument preflight, nested reborrow, borrowed return/release, and host-backed
bulk collections. Those rows are deliberately not inferred from broad VM or
Actor benchmarks; the remaining S0 benchmark task must give them stable names,
allocation counts where required, deterministic checksums, and quick/stable
sampling modes before S1 deletes the slot implementation.

## S1 Zero-Hit Audit

After the retained neutral helpers have moved and the old implementation has
been deleted, S1 uses the audit command from the execution plan. Matches are
allowed only in `docs/archive/**` and in the hard-switch plan's deletion
history. There will be no deprecated aliases, feature-gated compatibility
surface, or alternate override syntax.
