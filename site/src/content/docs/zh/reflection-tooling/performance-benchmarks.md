---
title: "性能和 Benchmark"
description: "Vela benchmark 结果如何采集和解释。"
---

Vela 性能工作必须先测量。正确性、budgets、GC roots、HostAccess routing、
reflection policy、hot reload ownership 和 diagnostics 都优先于原始速度。

## Benchmark Harnesses

常用 benchmark 命令包括：

```bash
cargo bench -p vela_vm --bench baseline
cargo bench -p vela_vm --bench baseline -- --quick
cargo bench -p vela_vm --bench external_compare -- --quick
cargo bench -p vela_engine --bench hot_reload -- --quick
```

过滤参数放在 `--` 后，例如：

```bash
cargo bench -p vela_vm --bench external_compare -- --quick string
```

## 外部对比模式

混合对比 harness 会明确报告 runtime mode：

```text
runtime=vela    mode=internal_hot_loop
runtime=lua54   mode=embedded_hot_loop
runtime=rhai    mode=embedded_hot_loop
runtime=node    mode=process_hot_loop
runtime=python3 mode=process_hot_loop
```

不同 mode 的行只能作为方向性参考，不应该混成一个绝对公平排行榜。

## 优化循环

被接受的性能工作应遵循：

```text
capture baseline -> profile hotspot -> 做一个聚焦改动 ->
capture candidate -> 和 baseline 对比 -> 有证据才保留
```

`tools/perf/` 下的辅助脚本会保留 key=value 原始输出，并用 checksum 校验对
比 candidate。

## 持久结果

日常本地 capture 放在 `perf-results/`。小而明确的 guardrail 可以放在
`perf-baselines/`。当前 benchmark 规则和持久 baseline 摘要维护在
`docs/performance.md`。
