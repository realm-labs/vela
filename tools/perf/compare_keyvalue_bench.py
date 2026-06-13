#!/usr/bin/env python3
"""Compare key=value benchmark captures and fail on regressions."""

from __future__ import annotations

import argparse
import shlex
import sys


def parse_value(value: str) -> str:
    try:
        parts = shlex.split(value)
    except ValueError:
        return value
    return parts[0] if len(parts) == 1 else value


def parse_rows(path: str) -> list[dict[str, str]]:
    rows = []
    with open(path, encoding="utf-8") as handle:
        for line in handle:
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            values = {}
            for part in shlex.split(line):
                if "=" not in part:
                    continue
                key, value = part.split("=", 1)
                values[key] = parse_value(value)
            if "bench" in values and "runtime" in values:
                rows.append(values)
    return rows


def row_key(row: dict[str, str]) -> tuple[str, str, str]:
    return (row["runtime"], row.get("mode", ""), row["bench"])


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--baseline", required=True)
    parser.add_argument("--candidate", required=True)
    parser.add_argument("--runtime")
    parser.add_argument("--bench")
    parser.add_argument("--mode")
    parser.add_argument("--metric", default="per_iter_mean_ns")
    parser.add_argument("--max-regression-percent", type=float, default=5.0)
    parser.add_argument("--allow-checksum-mismatch", action="store_true")
    args = parser.parse_args()

    baseline = {row_key(row): row for row in parse_rows(args.baseline)}
    candidate = {row_key(row): row for row in parse_rows(args.candidate)}
    keys = sorted(set(baseline) & set(candidate))
    if args.runtime:
        keys = [key for key in keys if key[0] == args.runtime]
    if args.mode:
        keys = [key for key in keys if key[1] == args.mode]
    if args.bench:
        keys = [key for key in keys if args.bench in key[2]]

    if not keys:
        print("no comparable rows found", file=sys.stderr)
        return 2

    failed = False
    limit = 1.0 + args.max_regression_percent / 100.0
    print(
        "runtime mode bench baseline candidate ratio status",
    )
    for key in keys:
        base = baseline[key]
        cand = candidate[key]
        base_value = int(base[args.metric])
        cand_value = int(cand[args.metric])
        ratio = cand_value / base_value if base_value else float("inf")
        status = "ok"
        if not args.allow_checksum_mismatch and base.get("checksum") != cand.get("checksum"):
            status = "checksum_mismatch"
            failed = True
        elif ratio > limit:
            status = "regressed"
            failed = True
        print(
            f"{key[0]} {key[1] or '-'} {key[2]} "
            f"{base_value} {cand_value} {ratio:.4f} {status}"
        )

    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
