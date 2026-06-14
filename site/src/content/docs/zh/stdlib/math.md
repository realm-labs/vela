---
title: "Math"
description: "Vela Math 标准库文档。"
---

Math 模块提供确定性的数值 helper。已实现的 helper 接收有限数值，并根据操作
返回整数或有限浮点数。非法 domain、非有限值、overflow 或参数数量错误都是
VM diagnostic。

## 标量 Helper

普通标量处理使用 `max`、`min`、`clamp`、`sign`、`floor`、`ceil`、
`round` 和 `abs`。

```vela
fn main() {
    let raw = -12;
    let normalized = math::clamp(math::abs(raw), 0, 10);
    return normalized + math::sign(-3);
}
```

当所有参数都是整数且操作可以保持整数语义时，helper 返回整数。浮点输入通常
返回 `f64`，而 `floor`、`ceil` 和 `round` 返回 `i64`。

## 移动、距离和幂运算

`lerp`、`move_towards`、`distance2d`、`distance3d`、`pow` 和 `sqrt`
覆盖常见玩法和模拟公式，但不把语言标准库变成特定游戏领域 API。

```vela
fn main() {
    let step = math::move_towards(0, 10, 3);
    let distance = math::distance2d(0, 0, 3, 4);
    let root = math::sqrt(81);
    return step + math::round(distance) + math::round(root);
}
```

`move_towards` 拒绝负 delta。`sqrt` 拒绝负输入。`pow` 对非负整数指数使用
checked integer power，否则使用有限浮点 power。

## 数值转换 Helper

显式数值转换 helper 位于按 primitive 命名的标准模块中。扩大转换不会失败；
缩小转换返回 `Result`。

```vela
fn main() {
    let wide = i64::from_i32(12);
    let narrow = u8::try_from_u64(255).unwrap_or(0);
    return wide + narrow;
}
```

Wrapping 和 bit helper 是显式函数，例如 `u8::wrapping_add`、
`u8::bit_and`、`u8::shift_left` 和 `u8::rotate_right`。算术操作符不隐含
wrapping 行为。

`math::random` 单独在 Random 页面说明，因为它只有在宿主开启 controlled
random 后才会安装。
