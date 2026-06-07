# 概览

Vela 是一个用 Rust 实现的 Hot Reload First 动态脚本语言。它面向 Rust host 持有状态的业务逻辑：Rust 负责持久状态，脚本负责规则表达，并通过受控 host access 读取、计算和修改状态。

它不是“动态 Rust”，也不是 Lua 的改写。Vela 保留脚本语言的使用体验，同时把 Rust embedding 边界设计清楚。

## 主要目标

- Rust 继续持有 host 状态。
- 脚本通过 `HostRef`、`HostPath`、`PathProxy`、`HostAccess` 访问和修改注册对象。
- 函数和模块代码可以热更新，已有调用帧继续运行旧代码。
- 反射可以查询元数据并做受控读写调用，但不能修改类型结构。
- Runtime 可嵌入：host 可以配置能力、native 函数、schema、预算、global 和热更新策略。

## 非目标

- 不支持脚本侧泛型。
- 不向脚本暴露真实 Rust `&mut T`。
- 不做 monkey patching 或运行时类型结构修改。
- MVP 不做 JIT、脚本 async/coroutine、移动 GC 或完整 LSP。

## 当前形态

当前原型已经具备解析、字节码编译、VM 执行、HostAccess、反射、标准 native、脚本 global、serde snapshot、缓存函数句柄和独立 embedding 示例。

可以在 playground 中体验 record、map、set、method、标准库 helper 和运行时诊断。
