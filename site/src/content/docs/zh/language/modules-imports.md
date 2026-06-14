---
title: "模块和导入"
description: "Vela 模块和导入文档。"
---

Vela 源文件不会在文件内声明自己的模块名。模块身份来自宿主选择的编译模式：单文件编译是轻量入口脚本，目录编译会把文件路径映射成模块路径。

## 模块身份

在目录模式下，`scripts/game/reward.vela` 会映射为 `game::reward`。目录编译后的入口需要使用完整函数名调用，例如 `game::main::main`。

```text
scripts/game/main.vela   -> game::main
scripts/game/reward.vela -> game::reward
scripts/config.vela      -> config
```

## 导入

`use` 从其他模块导入公开声明。静态路径使用 `::`；运行时字段访问使用 `.`，两者不是同一类操作。

```vela
use game::reward::grant
use config::BASE_REWARD as DEFAULT_REWARD

pub fn main(player) {
    grant(player, DEFAULT_REWARD)
}
```

## 可见性

`pub` 表示声明可以从所属模块导入，或在导出时通过嵌入 API 调用。私有声明是模块内部实现细节。

```vela
pub const BASE_REWARD: i64 = 10

fn internal_bonus(level: i64) -> i64 {
    return level * 2
}
```

## 热更新

导入关系参与依赖影响分析。一次热更新可以同时处理一个或多个变更文件，但 staged 版本能否激活仍然取决于 ABI 和 schema 兼容性。
