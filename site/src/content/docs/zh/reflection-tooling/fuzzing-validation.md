---
title: "Fuzzing 和验证"
description: "用于保持 Vela parsing、bytecode 和 runtime 行为稳定的验证层。"
---

Vela 使用分层验证。parser tests、compiler tests、VM tests、examples、
benchmarks、fuzz targets 和 CI checks 覆盖不同失败模式。

## 标准验证

默认完整验证目标是：

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

文档站语法高亮还提供：

```bash
cd site && npm run test:syntax
```

## Fuzzing

仓库在 `fuzz/` 下包含 parser fuzz target。本地安装 `cargo-fuzz` 后可以运
行：

```bash
cargo fuzz run parser
```

fuzzing 主要用于发现 parser crash、恢复问题和意外 panic。它不能替代语义
测试或 VM conformance tests。

## 示例覆盖

`examples/src/bin` 下的可运行示例覆盖热更新、反射、宿主权限、stale host
refs、schema rejection、I/O capabilities 和 domain-neutral standard helpers。

## 验证纪律

不要删除测试来让失败通过。新的 runtime 行为应先在拥有该 contract 的层写
聚焦测试；当行为跨越子系统时，再补充更宽的 example 或 conformance
fixture。
