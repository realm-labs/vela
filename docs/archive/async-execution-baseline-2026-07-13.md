# Async Execution Pre-Change Baseline

This checkpoint records the pre-change baseline for
`docs/async-execution-model-plan.md`. Measurements are comparison inputs for
the async execution batches, not current performance targets.

## Environment

```text
date: 2026-07-13
host: macOS 26.5.2, aarch64
rustc: 1.97.0 (2d8144b78 2026-07-07)
cargo: 1.97.0 (c980f4866 2026-06-30)
git: d57d035e1
profile: release for benchmarks
```

## Validation

The full pre-change validation passed:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Focused behavior also passed:

```bash
cargo test -p vela_vm call_depth_budget_stops_recursive_scripts
cargo test -p vela_vm callback_execution_unit_limit_has_a_stable_edge
cargo test -p vela_engine runtime_call_args_host_mut_dispatches_root_and_child_host_methods
cargo test -p vela_engine runtime_calls_provider_trait_impl_method
cargo test -p vela_engine old_frame_keeps_old_provider_generation_and_new_call_uses_new_generation
cargo test -p vela_engine runtime_event_end_safe_point_keeps_nested_calls_on_old_version_until_return
```

## Representative Runtime Measurements

The VM quick baseline used two repeats, eight iterations, and two warmups:

```bash
cargo bench -p vela_vm --bench baseline -- --quick \
  scalar_branch_loop script_call_small_args script_call_wide_args callback_collections
```

| Workload | Mode | Minimum | Mean | Median |
|---|---:|---:|---:|---:|
| scalar branch loop | interpreter | 113,333 ns | 113,562 ns | 113,791 ns |
| scalar branch loop | budgeted | 114,125 ns | 114,708 ns | 115,292 ns |
| small-argument script calls | interpreter | 913,917 ns | 1,010,667 ns | 1,107,417 ns |
| wide-argument script calls | interpreter | 971,459 ns | 1,014,979 ns | 1,058,500 ns |
| collection callbacks | interpreter | 11,538,083 ns | 12,287,791 ns | 13,037,500 ns |
| collection callbacks | cache enabled | 9,309,042 ns | 9,341,583 ns | 9,374,125 ns |

Checksums matched across the interpreter/profile/cache comparison rows.

The existing call-heavy and hot-reload benchmarks produced:

```bash
cargo bench -p vela_engine --bench generation_memory -- call-heavy
cargo bench -p vela_engine --bench hot_reload -- --quick
```

```text
call-heavy small artifact: 335,875 ns for 2,000 calls
call-heavy 201-function artifact: 741,417 ns for 2,000 calls
call-heavy artifact-size ratio: 2.207
hot-reload accepted update: min 21,168,459 ns, mean 24,106,958 ns
hot-reload ABI rejection: min 18,981,750 ns, mean 19,020,083 ns
```

Quick-run noise is expected. Batch D must rerun comparable rows on the same
machine/toolchain and investigate material synchronous regressions without
restoring recursive execution or duplicate drivers.
