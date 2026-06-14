---
title: "标准库方法"
description: "Vela 标准库方法文档。"
---

标准库方法提供基础值和集合值的内建行为。它们和脚本方法、宿主方法一样按 receiver 分发，但目标由运行时注册。

## Value Family

当前标准方法覆盖 string、bytes、array、map、set、range、iterator、`Option`、`Result`，以及部分数字转换 helper。

```vela
fn summarize(name: string, values) -> string {
    let total = values.iter().count()
    return f"{name}:{total}"
}
```

## 集合和 Iterator

集合方法暴露显式 view 和惰性适配器。`collect_array()` 是把 iterator 输出物化为数组的标准终端方法。

```vela
fn doubled(values) {
    return values.iter()
        .filter(|value| value > 0)
        .map(|value| value * 2)
        .collect_array()
}
```

## Option 和 Result Helper

`Option` 和 `Result` helper 让预期缺失和可恢复失败保持可见，而不是把它们变成 VM trap。

```vela
fn safe_amount(text: string) -> i64 {
    return text.parse_i64().unwrap_or(0)
}
```

## 分发和兼容性

标准方法会尽量使用稳定 ID，未知 receiver 仍可走动态分发。新增或修改标准方法时，必须保持与其他 callable target 相同的宿主边界、预算和热更新兼容性规则。
