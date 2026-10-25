---
title: "固有方法"
description: "Vela 固有方法文档。"
---

固有方法是直接声明在某个脚本类型上的方法。它们按 receiver 分发；当 receiver 类型已知时，会编译到稳定的方法元数据。

## 声明

使用 `impl Type { ... }` 给脚本 struct 或 enum 添加方法。第一个参数通常写作 `self`，表示被调用的值。

```vela
struct Player {
    level: i64
}

impl Player {
    fn bonus(self, amount: i64) -> i64 {
        return self.level + amount
    }
}
```

## 调用

方法调用语法是 `receiver.method(args...)`。如果编译器知道 receiver 类型，会链接到已解析方法；如果 receiver 是动态的，运行时方法分发会根据实际值解析目标。

```vela
fn main(player: Player) -> i64 {
    return player.bonus(5)
}
```

## 没有重载

同一个类型不能定义多个同名 receiver 方法。参数提示、默认值和参数数量不会形成重载集。

## 宿主边界

借用 Host 构造器和方法注册已经删除。普通脚本 impl 方法继续保留；独立同步
native 函数通过现有自有值 API 传递数据，不是 Rust receiver 方法，也没有替代
opaque 方法 API。
