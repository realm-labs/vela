---
title: "Set"
description: "Vela Set文档。"
---

Set 存储唯一的动态值，适合成员检查和脚本拥有数据上的集合运算。

## 构造和成员检查

Set 通常通过标准库 helper 或宿主提供的快照值创建。判断唯一性时应使用成员 API，而不是依赖数组扫描。

```vela
fn has_tag(tags, tag: String) -> bool {
    return tags.contains(tag)
}
```

## 修改

Set 方法覆盖插入、移除和清空等操作。修改脚本 set 会改变脚本堆值；修改宿主拥有的 set-like 字段要经过 HostAccess。

```vela
fn mark_seen(seen, id: i64) {
    if !seen.contains(id) {
        seen.insert(id)
    }
    return seen
}
```

## 集合运算

标准方法会提供 intersection、union、difference、subset check 等受支持操作。这些操作仍然是动态的，不需要 `Set<T>` 语法。

## 迭代

Set 是按 value 迭代的可重复 sequence。除非 API 明确承诺顺序，否则不要把 set 遍历顺序作为持久业务语义。
