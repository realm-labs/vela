#!/usr/bin/env python3
"""Capture external_compare output with reproducible metadata."""

from __future__ import annotations

import argparse
import datetime as dt
import os
import pathlib
import platform
import shlex
import shutil
import subprocess
import sys


ROOT = pathlib.Path(__file__).resolve().parents[2]


def output_text(command: list[str]) -> str:
    try:
        return subprocess.check_output(command, cwd=ROOT, text=True).strip()
    except (OSError, subprocess.CalledProcessError):
        return "unknown"


def load_average_text() -> str:
    try:
        return ",".join(f"{value:.2f}" for value in os.getloadavg())
    except (AttributeError, OSError):
        return "unavailable"


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

    worktree = output_text(["git", "status", "--porcelain"])
    metadata = [
        f"# captured_at_utc={captured_at}\n",
        f"# commit={output_text(['git', 'rev-parse', 'HEAD'])}\n",
        f"# branch={output_text(['git', 'branch', '--show-current'])}\n",
        f"# worktree={'clean' if not worktree else 'dirty'}\n",
        f"# rustc={output_text(['rustc', '--version'])}\n",
        f"# cargo={output_text(['cargo', '--version'])}\n",
        f"# platform={platform.platform()}\n",
        f"# machine={platform.machine()}\n",
        f"# cpu={output_text(['sysctl', '-n', 'machdep.cpu.brand_string'])}\n",
        f"# logical_cpus={os.cpu_count()}\n",
        f"# load_average_before={load_average_text()}\n",
        f"# command={shlex.join(command)}\n",
        "\n",
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

    if args.baseline and status == 0:
        baseline_path = ROOT / "perf-baselines" / f"{args.baseline}.txt"
        baseline_path.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(output_path, baseline_path)
        print(f"baseline={baseline_path.relative_to(ROOT)}")

    return status


if __name__ == "__main__":
    raise SystemExit(main())
