---
title: "安装和 CLI"
description: "Vela 安装和 CLI文档。"
---

Vela 目前主要从源码仓库中使用。公开安装包、稳定 bytecode artifact 分发还不是当前稳定接口。

## 环境要求

需要较新的 Rust toolchain 和 Cargo。文档站还会用到 Node 和 npm，但只有在开发网站时才需要。

克隆仓库后可以先验证 workspace：

```bash
cargo fmt --all -- --check
cargo test --workspace
```

## CLI 用法

CLI 可以通过本地 workspace binary 运行 `.vela` 源码文件：

```bash
cargo run -p vela_cli -- path/to/script.vela
```

CLI 适合直接执行脚本和做 smoke check。更接近生产的用法通常是通过 `vela_engine` 嵌入，让宿主注册类型、函数、capability、预算、global 和热更新策略。

## 示例和文档站

可运行嵌入示例在 `examples/src/bin`：

```bash
cargo run -p vela_examples --bin modules
cargo run -p vela_examples --bin host_type_methods
cargo run -p vela_examples --bin script_global
```

开发文档站：

```bash
cd site
npm ci
npm run dev
```

发布流程会生成 playground WASM 资源。只改文档时，安装依赖后通常可以直接用 `npm run build` 验证站点。
