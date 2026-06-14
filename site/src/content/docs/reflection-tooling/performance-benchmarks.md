---
title: "Performance And Benchmarks"
description: "How Vela benchmark results are captured and interpreted."
---

Vela performance work is measurement-first. Correctness, budgets, GC roots,
HostAccess routing, reflection policy, hot reload ownership, and diagnostics
take priority over raw speed.

## Benchmark Harnesses

Common benchmark commands are:

```bash
cargo bench -p vela_vm --bench baseline
cargo bench -p vela_vm --bench baseline -- --quick
cargo bench -p vela_vm --bench external_compare -- --quick
cargo bench -p vela_engine --bench hot_reload -- --quick
```

Filters are passed after `--`, for example:

```bash
cargo bench -p vela_vm --bench external_compare -- --quick string
```

## External Comparison Modes

The mixed comparison harness reports runtime mode explicitly:

```text
runtime=vela    mode=internal_hot_loop
runtime=lua54   mode=embedded_hot_loop
runtime=rhai    mode=embedded_hot_loop
runtime=node    mode=process_hot_loop
runtime=python3 mode=process_hot_loop
```

Rows with different modes are directional comparisons, not one absolute
fairness leaderboard.

## Optimization Loop

Accepted performance work should follow this loop:

```text
capture baseline -> profile hotspot -> make one focused change ->
capture candidate -> compare against baseline -> keep only with evidence
```

The helper scripts under `tools/perf/` retain raw key=value output and compare
candidate rows with checksum validation.

## Durable Results

Routine local captures belong under `perf-results/`. Small intentional
guardrails can live under `perf-baselines/`. Current benchmark rules and durable
baseline summaries are maintained in `docs/performance.md`.
