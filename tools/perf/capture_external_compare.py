#!/usr/bin/env python3
"""Capture external_compare output with reproducible metadata."""

from __future__ import annotations

import argparse
import datetime as dt
import pathlib
import shutil
import subprocess
import sys


ROOT = pathlib.Path(__file__).resolve().parents[2]


def output_text(command: list[str]) -> str:
    try:
        return subprocess.check_output(command, cwd=ROOT, text=True).strip()
    except (OSError, subprocess.CalledProcessError):
        return "unknown"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--name", default="external_compare")
    parser.add_argument("--output-dir", default="perf-results/external_compare")
    parser.add_argument(
        "--baseline",
        help="also copy the capture to perf-baselines/<name>.txt",
    )
    parser.add_argument(
        "bench_args",
        nargs=argparse.REMAINDER,
        help="arguments after -- are passed to external_compare",
    )
    args = parser.parse_args()

    bench_args = args.bench_args
    if bench_args and bench_args[0] == "--":
        bench_args = bench_args[1:]

    command = [
        "cargo",
        "bench",
        "-p",
        "vela_vm",
        "--bench",
        "external_compare",
        "--",
        *bench_args,
    ]
    captured_at = dt.datetime.now(dt.UTC).strftime("%Y%m%dT%H%M%SZ")
    output_dir = ROOT / args.output_dir
    output_dir.mkdir(parents=True, exist_ok=True)
    output_path = output_dir / f"{captured_at}-{args.name}.txt"

    metadata = [
        f"# captured_at_utc={captured_at}",
        f"# commit={output_text(['git', 'rev-parse', 'HEAD'])}",
        f"# branch={output_text(['git', 'branch', '--show-current'])}",
        f"# rustc={output_text(['rustc', '--version'])}",
        f"# cargo={output_text(['cargo', '--version'])}",
        f"# command={' '.join(command)}",
        "",
    ]

    process = subprocess.Popen(
        command,
        cwd=ROOT,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        bufsize=1,
    )
    assert process.stdout is not None
    lines = []
    for line in process.stdout:
        sys.stdout.write(line)
        lines.append(line)
    status = process.wait()

    output_path.write_text("".join(metadata + lines), encoding="utf-8")
    print(f"saved={output_path.relative_to(ROOT)}")

    if args.baseline:
        baseline_path = ROOT / "perf-baselines" / f"{args.baseline}.txt"
        baseline_path.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(output_path, baseline_path)
        print(f"baseline={baseline_path.relative_to(ROOT)}")

    return status


if __name__ == "__main__":
    raise SystemExit(main())
