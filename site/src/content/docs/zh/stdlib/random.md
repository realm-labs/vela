---
title: "Random"
description: "Vela Random 标准库文档。"
---

Random 是 opt-in 能力。当前标准 surface 是由宿主用确定性 seed 安装的
`math::random(min, max)`。它返回闭区间 `[min, max]` 内的 `i64`。

## Controlled Random

宿主安装 controlled random 后，脚本通过 `math` 模块使用该函数。

```vela
fn main() {
    let first = math::random(1, 6);
    let second = math::random(10, 12);
    if first >= 1 && first <= 6 && second >= 10 && second <= 12 {
        return first * 100 + second;
    }
    return 0;
}
```

非整数边界和 `min > max` 都会产生 VM diagnostic。

## Capability 和确定性

`math::random` 带有 `random` effect。宿主可以注册函数让脚本能编译，但通过
capability 拒绝执行。

```vela
fn main() {
    let roll = math::random(1, 20);
    return roll >= 10;
}
```

测试和 replay 时使用相同 seed 和调用顺序。脚本不应依赖具体算法或序列，只
应依赖给定 engine 配置下的宿主确定性策略。
