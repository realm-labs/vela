---
title: "能力和执行预算"
description: "从嵌入层限制 host effects，并约束 Vela 调用能执行多少工作。"
---

Capabilities 决定程序可以使用哪些宿主 effects。Budgets 决定一次调用可以做
多少工作。

## Capabilities

现有值导向 native API 可以声明宿主 effects，但不会暴露 Rust receiver 或借用值。
下面的闭包拥有各自捕获的 Rust 状态，只有 `i64` 和 unit 值跨越脚本边界：

```rust
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

let counter = Arc::new(AtomicI64::new(0));
let read_counter = Arc::clone(&counter);
let write_counter = Arc::clone(&counter);
let engine = Engine::builder()
    .capability(Capability::HostRead)
    .capability(Capability::HostWrite)
    .register_typed_native_fn::<(), _>(
        NativeFunctionDesc::new("app::read_counter", NativeFunctionId::new(10_001))
            .returns(TypeHint::i64())
            .effects(EffectSet::host_read()),
        move || read_counter.load(Ordering::Relaxed),
    )
    .register_typed_native_fn::<(i64,), _>(
        NativeFunctionDesc::new("app::write_counter", NativeFunctionId::new(10_002))
            .param("value", TypeHint::i64())
            .returns(TypeHint::unit())
            .effects(EffectSet::host_write()),
        move |value| write_counter.store(value, Ordering::Relaxed),
    )
    .build()?;
```

当前 capability gates 包括 host read/write、event emit、time、random、IO
read/write，以及 reflection read/write/call。注册的独立同步 native 函数声明
effects；runtime 会把这些 effects 与 active capability profile 对比。宿主读写
权限不会恢复透明宿主路径、构造器或方法。

```rust
let sandboxed = Engine::builder()
    .execution_profile(ExecutionProfile::sandboxed())
    .build()?;
```

## 执行预算

`CallOptions` 控制 instruction count、memory bytes 和 call depth。

```rust
let options = CallOptions::new(
    10_000,       // instruction limit
    1024 * 1024,  // memory limit
    64,           // max call depth
);
runtime.call("main", CallArgs::new(), options)?;
```

`CallOptions::unbounded()` 适合可信 examples 和 tests，但生产宿主应该使用明确
限制。

## Denials

权限检查针对注册 native 调用声明的 effects。没有 `HostRead` 时，
`app::read_counter()` 会被拒绝；没有 `HostWrite` 时，`app::write_counter(value)`
会被拒绝。自有值参数和返回值不代表 native 操作是纯函数。脚本必须显式调用：

```vela
fn main() {
    let current = app::read_counter();
    app::write_counter(current + 1);
    return current;
}
```

## 脚本自有字段

脚本自有字段不需要 Rust 宿主授权：`HostRead` 或 `HostWrite`。读取、赋值或
对这些字段使用 `+=` 都只操作脚本数据，包括现有 Rust 值转换产生的数据。
类型契约和执行预算仍然生效；这些修改不会隐式写回 Rust。

```vela
struct Counter { value: i64 }

fn increment(counter: Counter) -> i64 {
    counter.value += 1;
    return counter.value;
}
```
