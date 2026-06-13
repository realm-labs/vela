# Performance Baselines

This directory is for checked-in benchmark guardrail baselines. Commit only
small, intentional key=value captures that are used by a documented regression
check. Routine local captures and profiler output belong under `perf-results/`,
which is ignored by git.

Baseline files should include the command, commit, toolchain, target, profile,
warmup, repeats, iterations, checksums, and the raw benchmark rows emitted by
the harness.
