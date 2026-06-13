# 类型和值

Vela 在运行时是动态类型，类型元数据主要用于分析、诊断和反射。

## 标量值

常见标量值包括：

- `null`
- `bool`
- `char`
- `i8`、`i16`、`i32`、`i64`
- `u8`、`u16`、`u32`、`u64`
- `f32`、`f64`
- `string`
- `bytes`

```vela
let enabled = true;
let marker = '!';
let level = 12i64;
let ratio = 1.5f64;
let name = "knight";
let payload = b"ok";
```

## Record 和 Enum

脚本 record 和 enum 是 VM 管理的一等值。

```vela
struct Damage {
    amount: i64,
    source: string,
}

enum Check {
    Pass { score: i64 },
    Fail { reason: string },
}
```

## Host 值

Rust-owned 复杂对象不会通过 `HostValue` 深拷贝。它们用 host handle 和 path 表示。脚本 owned struct 可以在启用 serde feature 时通过 snapshot 路径和 Rust 互转。
