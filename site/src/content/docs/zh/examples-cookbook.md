---
title: "示例和 Cookbook"
description: "Vela 任务导向示例和 Cookbook。"
---

仓库在 `examples/src/bin` 下提供了一组独立示例。它们是理解完整宿主设置的最好入口，因为这些示例会编译真实 Rust、注册 host 类型，并实际触发 Runtime capability。

## 从 Rust 运行脚本

`level_up` 示例展示了最小的 host 写穿形态：

```vela
fn main(player: Player) {
    player.level += 1;
    return player.level;
}
```

运行方式：

```bash
cargo run -p vela_examples --bin level_up
```

宿主会注册 `Player`，授予 host read/write capability，把 `player` 作为可变 host binding 传入，并读取脚本返回值。

## 注册 Native 函数

`native_function` 示例覆盖手写注册和宏生成的 native 函数：

```vela
fn main(player: Player) {
    let manual = game::bonus_manual(3, 4);
    let generated = game::bonus_macro(10, 7);
    let collection = game::collection_bonus(
        { "quest": 5, "raid": 8 },
        ["vip", "daily"],
    );
    let level = game::grant_level(player, collection);

    return manual + generated + level;
}
```

如果需要让脚本调用 Rust 函数，或者需要通过 `NativeCallContext` 路由写入，可以从这个示例开始。

## 使用模块和 Global

`modules` 示例会编译一个目录，并调用完整限定名入口：

```vela
use game::reward::grant

fn main() {
    return grant(4) + game::config::BASE_REWARD;
}
```

`script_global` 示例展示了在 Vela 中声明 runtime-managed global，并从 Rust 初始化或更新它。

## 验证边界

一些示例会故意失败，用来展示诊断和策略约束：

- `host_permission_denied`
- `host_read_only_denied`
- `host_write_permission_denied`
- `host_call_permission_denied`
- `generic_type_hint_denied`
- `reflect_schema_mutation_denied`

需要检查 capability profile、只读 schema、不支持的 type hint 或反射限制时，可以参考这些示例。
