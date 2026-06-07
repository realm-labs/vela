# Native 函数

Native 函数允许脚本调用 host 注册的 Rust 代码。

## 简单 Native 函数

Rust 注册函数时需要元数据和 callable thunk。宏可以生成元数据和转换层。

```rust
#[script_function(module = "game", name = "bonus")]
fn bonus(base: i64, multiplier: i64) -> i64 {
    base * multiplier
}
```

脚本调用：

```vela
fn main() {
    return game::bonus(10, 3);
}
```

## Effect 和 Capability

Native 函数声明 host read、host write、time、random、reflection、I/O 等 effect。Engine 的 capability set 决定调用是否允许。

这让脚本保持表达力，同时让 host 可见副作用保持明确。
