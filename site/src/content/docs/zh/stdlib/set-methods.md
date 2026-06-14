---
title: "Set 方法"
description: "Vela Set 方法文档。"
---

Set 通过和 Map key 相同的 `ValueKey` 策略存储唯一脚本值。`null`、bool、
有限数字、char、string 和 bytes 等不可变叶子值按值比较；脚本堆对象和
host ref 按身份比较；`PathProxy` 等临时值会在修改前被拒绝。

## 构造和成员检查

用 `set::from_array` 从数组构造 Set。重复值会按 `ValueKey` 相等规则去重，
因此同一脚本对象的别名会合并，而字段相同但独立构造的对象仍然是不同元素。

```vela
fn main() {
    let tags = set::from_array(["daily", "quest", "daily"]);
    if tags.has("quest") && tags.len() == 2 {
        return "ok";
    }
    return "missing";
}
```

`values` 和 `iter` 产生 set value iterator。如果展示顺序重要，先 collect
再 sort。

```vela
fn main() {
    let tags = set::from_array(["raid", "daily", "quest"]);
    return tags.values().collect_array().sort().join(",");
}
```

## 修改

`add` 插入新值时返回 `true`，值已存在时返回 `false`。`remove` 返回是否真的
移除了值。

```vela
fn main() {
    let tags = set::from_array(["daily"]);
    let added = tags.add("quest");
    let removed = tags.remove("missing");
    tags.extend(set::from_array(["raid", "daily"]));
    return added && !removed && tags.len() == 3;
}
```

`clear` 移除所有值并返回 `null`。

## Set 代数

`union`、`intersection`、`difference` 和 `symmetric_difference` 返回新 Set。
关系 helper 返回 bool。

```vela
fn main() {
    let owned = set::from_array(["daily", "quest", "raid"]);
    let required = set::from_array(["daily", "quest"]);
    let event = set::from_array(["quest", "bonus"]);
    let shared = owned.intersection(event);
    return required.is_subset(owned)
        && owned.is_superset(required)
        && shared.has("quest");
}
```

Callback helper `map`、`filter`、`find`、`any`、`all` 和 `count` 与数组模型
一致。
