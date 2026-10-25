---
title: "I/O"
description: "Vela I/O 标准库文档。"
---

I/O 默认不安装。宿主需要显式开启 stdout 和沙箱文件系统 helper，并授予
`io_read` 和/或 `io_write` capability。这样脚本默认保持确定性，进程 effect
也清晰可见。

## Stdout

`io::print(value)` 向 stdout 写入格式化值。`io::println(value)` 写入值并换行。
两者都返回 `Result`。

```vela
fn main() {
    let printed = io::println("hello from Vela");
    return printed.is_ok();
}
```

普通输出失败会变成 `Result::Err(IoError)`。类型错误、函数未注册和 capability
拒绝仍然是 VM diagnostic。

## 沙箱文件系统

`fs::read_to_string(path)` 读取配置沙箱内的 UTF-8 文件。
`fs::write_string(path, text)` 写入沙箱内的 UTF-8 文件。两者都返回 `Result`。

```vela
fn main() {
    let input = result::unwrap_or(fs::read_to_string("input.txt"), "missing");
    fs::write_string("output.txt", "done");
    return input.len();
}
```

路径必须是相对路径。空路径、绝对路径、盘符前缀和父目录逃逸会以
`Result::Err(IoError)` 拒绝。

## 宿主安装

Rust 宿主需要显式安装这些 helper。

```rust
let engine = Engine::builder()
    .with_standard_natives()
    .capability(Capability::IoRead)
    .capability(Capability::IoWrite)
    .with_stdio()
    .with_fs_io(root)
    .build()?;
```

I/O helper 适合工具、demo 和受控脚本。宿主业务逻辑也可以通过现有独立同步
native 调用和显式返回结果交换脚本自有数据。这些调用保留声明的 effect 与
capability 检查，不提供透明 Rust 状态适配器。
