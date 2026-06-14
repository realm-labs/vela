---
title: "Record 和 Struct"
description: "Vela Record 和 Struct文档。"
---

`struct` 声明定义脚本拥有的 record 形状。Record 是带命名字段的动态值，可以有字段默认值和字段契约。它们和宿主对象是不同的数据类别，即使两者建模的是同一个业务概念。

## 声明和构造

字段可以有类型提示和默认值。Record 字面量使用类型路径加命名字段。

```vela
struct Reward {
    code: string
    amount: i64 = 0
}

fn default_reward() -> Reward {
    return Reward { code: "xp", amount: 10 }
}
```

## 字段访问和修改

脚本 record 字段使用点号访问，并且在写入值满足字段契约时可以赋值。字段名不存在或写入类型不兼容都会产生诊断。

```vela
fn boost(reward: Reward) -> Reward {
    reward.amount += 5
    return reward
}
```

## Struct 和宿主类型

脚本 struct 是 GC 管理的脚本数据。注册宿主类型是 Rust 拥有的状态，通过 `HostRef`、`HostPath`、`PathProxy` 和 HostAccess 访问。源码层都可能出现点号语法，但运行时边界不同。

## 反射和热更新

字段名、类型提示、默认值和属性都是可反射元数据，也属于 schema 兼容性的一部分。热更新可以接受兼容新增，但会拒绝破坏已有调用帧或宿主契约的结构变化。
