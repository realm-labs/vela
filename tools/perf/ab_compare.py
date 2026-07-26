#!/usr/bin/env python3
"""Compare two benchmark binaries against a per-benchmark noise floor.

Interpreter benchmarks on this project are dominated by code-layout effects:
an unrelated change can move a row by tens of percent, and the same binary
compared against itself has been observed to differ by 31% on the noisiest row
while staying inside 0.1% on the quietest. A fixed percentage threshold
therefore either hides real regressions or invents them, depending on the row.

This runs both binaries interleaved so thermal drift cancels, estimates each
row's own noise by comparing independent runs of the *baseline* against each
other, and reports a delta as significant only when it exceeds that row's
measured noise.

Usage:
    tools/perf/ab_compare.py BASELINE_BIN CANDIDATE_BIN [-- BENCH_ARGS...]

Example:
    tools/perf/ab_compare.py /tmp/before /tmp/after \\
        -- --runtime vela --iterations 5000 --repeats 1 --warmup 1
"""

from __future__ import annotations

import argparse
import re
import statistics
import subprocess
import sys

ROW = re.compile(r"bench=(\S+).*?min_ns=(\d+).*?checksum=(\d+)")
DEFAULT_ARGS = ["--runtime", "vela", "--iterations", "5000", "--repeats", "1", "--warmup", "1"]


def run(binary: str, bench_args: list[str]) -> dict[str, tuple[int, str]]:
    result = subprocess.run(
        [binary, *bench_args], capture_output=True, text=True, check=True
    )
    rows: dict[str, tuple[int, str]] = {}
    for line in result.stdout.splitlines():
        match = ROW.search(line)
        if match:
            rows[match.group(1)] = (int(match.group(2)), match.group(3))
    if not rows:
        sys.exit(f"{binary} produced no benchmark rows; check the bench arguments")
    return rows


def best_of(samples: list[dict[str, tuple[int, str]]]) -> dict[str, tuple[int, str]]:
    """Takes the fastest observation per row.

    Minimum is the right summary here: a benchmark cannot run faster than the
    machine allows, so the low end is signal and the high end is interference.
    """
    merged: dict[str, tuple[int, str]] = {}
    for sample in samples:
        for bench, (value, checksum) in sample.items():
            if bench not in merged or value < merged[bench][0]:
                merged[bench] = (value, checksum)
    return merged


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("baseline")
    parser.add_argument("candidate")
    parser.add_argument(
        "--rounds",
        type=int,
        default=3,
        help="interleaved rounds per binary (default 3); more rounds tighten the noise estimate",
    )
    parser.add_argument("bench_args", nargs="*", help="arguments forwarded to both binaries")
    options = parser.parse_args()

    bench_args = options.bench_args or DEFAULT_ARGS

    baseline_runs: list[dict[str, tuple[int, str]]] = []
    candidate_runs: list[dict[str, tuple[int, str]]] = []
    for round_index in range(options.rounds):
        print(f"round {round_index + 1}/{options.rounds}...", file=sys.stderr)
        baseline_runs.append(run(options.baseline, bench_args))
        candidate_runs.append(run(options.candidate, bench_args))

    if options.rounds < 2:
        sys.exit("at least 2 rounds are required to estimate per-row noise")

    # Split baseline observations into two halves and compare them. Any
    # difference there is the machine talking, not the change under test.
    half = options.rounds // 2
    noise_reference = best_of(baseline_runs[:half])
    noise_control = best_of(baseline_runs[half:])

    baseline = best_of(baseline_runs)
    candidate = best_of(candidate_runs)

    rows = []
    for bench, (base_value, base_checksum) in baseline.items():
        if bench not in candidate:
            continue
        candidate_value, candidate_checksum = candidate[bench]
        delta = (candidate_value - base_value) / base_value * 100
        noise = None
        if bench in noise_reference and bench in noise_control:
            reference = noise_reference[bench][0]
            noise = abs(noise_control[bench][0] - reference) / reference * 100
        rows.append((bench, base_value, candidate_value, delta, noise,
                     base_checksum == candidate_checksum))

    rows.sort(key=lambda row: row[3])

    print(f"\n{'benchmark':<38}{'baseline':>13}{'candidate':>13}{'delta':>9}{'noise':>8}  verdict")
    mismatches = 0
    for bench, base_value, candidate_value, delta, noise, checksum_ok in rows:
        if not checksum_ok:
            mismatches += 1
            verdict = "CHECKSUM MISMATCH"
        elif noise is None:
            verdict = "no noise estimate"
        elif abs(delta) <= max(noise, 1.0):
            verdict = "within noise"
        else:
            verdict = "FASTER" if delta < 0 else "SLOWER"
        noise_text = "n/a" if noise is None else f"{noise:.1f}%"
        print(
            f"{bench:<38}{base_value:>13}{candidate_value:>13}"
            f"{delta:>8.1f}%{noise_text:>8}  {verdict}"
        )

    significant = [row for row in rows if row[4] is not None and abs(row[3]) > max(row[4], 1.0)]
    faster = [row for row in significant if row[3] < 0]
    slower = [row for row in significant if row[3] > 0]
    print(
        f"\n{len(faster)} faster, {len(slower)} slower, "
        f"{len(rows) - len(significant)} within noise"
    )
    if significant:
        print(
            "median significant delta: "
            f"{statistics.median(row[3] for row in significant):.1f}%"
        )
    if mismatches:
        print(f"\n{mismatches} row(s) changed their checksum: the candidate is not equivalent.")
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
