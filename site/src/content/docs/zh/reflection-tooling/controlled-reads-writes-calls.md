---
title: "受控读写调用"
description: "动态反射操作及其安全边界。"
---

反射可以动态读取字段、写入字段、调用函数或方法。这些操作是受控且受策
略约束的。

## 读取

`reflect::get(target, field)` 在目标 shape 和当前策略允许时读取字段。

```vela
let level = reflect::get(player, "level");
```

对于宿主对象，读取会走 host access 机制。对于脚本 record 和 enum，反射
会尽量使用已注册脚本 metadata，让诊断能命名脚本类型。

## 写入

`reflect::set(target, field, value)` 只在字段可写且当前反射/宿主策略允许
时写入。

```vela
reflect::set(player, "level", 12);
```

这仍然不会向脚本暴露 Rust `&mut T`。宿主修改通过 `HostRef`、`HostPath`、
`PathProxy`、`HostAccess` 和宿主 adapter 完成。

## 调用

`reflect::call(target, args...)` 只会调用被标记为 reflection-callable 的函
数或方法。effects、capabilities、budgets 和参数转换仍然生效。

## 失败模式

受控操作可能因为 unknown field、read-only field、permission denied、
stale host reference、argument mismatch、effect denial 或 budget exhaustion
失败。这些都是普通 runtime error，不代表 reflection metadata 被破坏。
