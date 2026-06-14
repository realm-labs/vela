---
title: "Enum 和 Match"
description: "Vela Enum 和 Match文档。"
---

Enum 用于建模带标签的值。Variant 可以为空、tuple-like 或 record-like，`match` 则提供受控的分支方式来处理这些形状。

## 声明 Enum

Enum variant 可以携带字段和类型提示。Vela 不使用泛型 enum 语法，所以 `Option` 和 `Result` 是普通动态 enum 家族，而不是 `Option<T>` 或 `Result<T, E>`。

```vela
enum QuestState {
    NotStarted
    Active { step: i64 }
    Complete(reward: string)
}
```

## Match

Pattern 包括通配符、绑定、字面量、路径、tuple variant 和 record variant。`if` guard 可以进一步限制某个 arm。

```vela
fn next_step(state: QuestState) -> i64 {
    match state {
        QuestState::Active { step } if step < 10 => step + 1,
        QuestState::Active { step } => step,
        _ => 0,
    }
}
```

## Variant 数据

Record variant 保留命名字段。Tuple variant 保留位置字段。Pattern 绑定会在 arm body 中引入局部值；更新 enum 通常意味着构造一个新的 enum 值。

## 热更新和反射

Variant 名称、字段形状、类型提示和稳定 ID 参与反射和热更新检查。删除或改变公开 variant 形状可能被拒绝，因为现有调用方或活动帧仍然可能依赖它。
