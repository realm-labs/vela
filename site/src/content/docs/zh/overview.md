---
title: "概览"
description: "Vela 概览文档。"
---

Vela 是一个给 Rust 宿主使用的脚本语言，适合把业务规则放进可更新的脚本里，同时让持久状态、权限和运行时副作用仍然由 Rust 控制。

游戏服务器脚本是主要验证场景，但语言核心保持领域无关。玩家、任务、奖励、订单、账户、工作流这类对象来自宿主注册和示例，不是语言内建特殊对象。

## Vela 重点解决什么

Vela 首先是 Hot Reload First。源码会编译成带版本的字节码，更新时在安全边界替换函数或模块 CodeObject。已经在运行的调用帧继续执行旧代码，新调用在更新接受后进入新代码。

Vela 也把 HostAccess 当成核心边界。脚本可以写成普通字段访问：

```vela
fn level_up(player: Player) {
    player.level += 1;
    return player.level;
}
```

这不会把 Rust 的 `&mut Player` 暴露给脚本。VM 会把读改写操作路由到 `HostRef`、`HostPath`、`PathProxy` 和 `HostAccess`，由宿主 adapter 校验权限并执行写入。

## Vela 不是什么

Vela 不是动态 Rust，也不是 Lua table/metatable 模型的复刻，更不是无限制插件沙箱。MVP 明确不包含脚本泛型、JIT、脚本 async/coroutine、monkey patching、运行时修改类型结构，也不会把真实 Rust 引用暴露给脚本。

语言是动态的，但嵌入边界是显式的。宿主决定哪些类型、字段、方法、native 函数、global、能力和预算对某个 Runtime 可用。

## 系统形态

常规执行链路是：

```text
source -> parser -> HIR -> bytecode -> VM -> HostAccess -> Rust host state
```

宿主创建 `Engine`，编译单个源码或目录为 program，再创建 `Runtime`，通过 `CallArgs` 和 `CallOptions` 调用脚本入口。反射和诊断可以查询元数据、报告错误和执行受控读写调用，但不能在运行时修改注册 schema。
