---
title: "Time"
description: "Vela Time 标准库文档。"
---

Time 由宿主提供，并且是确定性的。Vela 不直接读取进程 wall-clock；embedding
应用安装 clock 并授予 time effect 后，脚本才可以使用时间函数。

## 已安装的 Clock 函数

宿主调用 `with_time_clock(now, tick)` 后，脚本可以调用 `time::now`、
`time::tick` 和 `time::elapsed_since`。

```vela
fn main() {
    let start = 1_699_999_990;
    return time::elapsed_since(start) + time::tick();
}
```

`time::now()` 返回配置的 timestamp。`time::tick()` 返回配置的逻辑 tick。
`time::elapsed_since(start)` 返回 `time::now() - start`，输入非法或 overflow
时 trap。

## Capability 和 Replay

Time 函数带有 `time` effect。即使函数已经注册用于编译，宿主也可以通过
capability profile 拒绝执行。

```vela
fn main(ctx: Context) {
    if time::tick() > ctx.tick {
        return time::now();
    }
    return ctx.now;
}
```

测试和 replay 时，给 engine 传入相同的 `now` 和 `tick`。宿主注册 context
schema 后，标准 context 对象也会通过 `Context.now` 和 `Context.tick` 暴露同
一组确定性值。
