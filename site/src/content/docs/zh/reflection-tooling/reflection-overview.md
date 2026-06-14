---
title: "反射概览"
description: "Vela 反射可以检查什么，以及不能修改什么。"
---

反射给脚本和宿主工具提供受控的 Vela metadata 视图。它服务于宿主集成、
诊断、管理工具、调试器、编辑器和热更新检查。

## 反射可以看到什么

反射可以查询 type、field、method、variant、trait、module、function、
attribute、source origin、effect metadata 和 permission metadata，也可以查
询某个值的 runtime type。

```vela
fn main(player: Player) {
    let player_type = reflect::type_of(player);
    let fields = reflect::fields(player_type);
    let level = reflect::field(player, "level");
    return reflect::name(player_type);
}
```

## 受控操作

当当前策略允许时，反射可以执行受控 read、write 和 call。这些操作仍然通
过普通脚本执行相同的 runtime 和 host access 边界。

反射不是绕过 `HostAccess`、execution budget、capability、只读字段或
stale host reference 校验的后门。

## 反射不能做什么

反射不能在 runtime 修改类型结构。它不能添加字段、删除方法、替换函数、
monkey patch 类型，也不能执行生成出来的源码字符串。

## 版本化元数据

热更新会创建新的 registry 快照。反射观察相关程序版本的 registry，因此
旧调用帧和工具视图能保持稳定，而新调用可以进入新版本。
