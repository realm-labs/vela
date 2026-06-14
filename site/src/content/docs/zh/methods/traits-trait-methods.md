---
title: "Trait 和 Trait 方法"
description: "Vela Trait 和 Trait 方法文档。"
---

Trait 是运行时协议，用来描述脚本类型或宿主类型可以实现的方法。它支持动态 protocol-style 分发，但不是把 Rust trait 直接搬进脚本语法。

## Trait 声明

Trait 方法可以是必需签名，也可以有默认实现。Trait 方法上的类型提示是运行时契约和反射元数据。

```vela
trait BonusSource {
    fn bonus(self, amount: i64) -> i64 {
        return amount
    }
}
```

## 实现

使用 `impl Trait for Type` 为脚本类型实现协议。显式方法会覆盖 trait 默认方法。

```vela
struct Player {
    level: i64
}

impl BonusSource for Player {
    fn bonus(self, amount: i64) -> i64 {
        return self.level + amount
    }
}
```

## 分发

Trait 方法调用仍然是 receiver-dispatched。已知 receiver 调用可以使用 linked method ID；动态调用通过运行时 receiver 分类和 registry-backed 元数据解析。

## 边界

Trait 不允许 monkey patching，也不允许运行时修改类型结构。宿主类型实现必须由宿主注册，并且保留 HostAccess 安全、能力检查和热更新兼容性。
