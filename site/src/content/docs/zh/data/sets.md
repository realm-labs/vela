---
title: "Set"
description: "Vela Set文档。"
---

Set 存储唯一的动态值，适合成员检查和脚本拥有数据上的集合运算。`Set<T>`
是内建类型提示契约，用于检查边界和类型化修改路径。Set 元素使用和 Map key
相同的 `ValueKey` 策略：不可变叶子值按值作为 key，脚本堆对象和 host ref
按身份作为 key，`PathProxy` 等临时值会在修改前被拒绝。`Function` 在 callable
身份语义明确前不会被接受为可 key 化的类型提示契约。

## 构造和成员检查

Set 通常通过标准库 helper 或宿主提供的快照值创建。判断唯一性时应使用成员 API，而不是依赖数组扫描。

```vela
fn has_tag(tags, tag: String) -> bool {
    return tags.has(tag)
}
```

## 修改

Set 方法覆盖插入、移除和清空等操作。修改脚本 set 会改变脚本堆值；修改宿主拥有的 set-like 字段要经过 HostAccess。

```vela
fn mark_seen(seen, id: i64) {
    if !seen.has(id) {
        seen.add(id)
    }
    return seen
}
```

```vela
fn add_tag(tags: Set<String>, tag) {
    tags.add("checked") // 静态兼容
    tags.add(tag)       // 动态值，写入前检查
    return tags
}
```

## 集合运算

标准方法会提供 intersection、union、difference、subset check 等受支持操作。
擦除的 `Set` 仍然有效；只有边界需要检查 keyable 元素契约时才需要 `Set<T>`。

## 迭代

Set 是按 value 迭代的可重复 sequence。除非 API 明确承诺顺序，否则不要把 set 遍历顺序作为持久业务语义。
