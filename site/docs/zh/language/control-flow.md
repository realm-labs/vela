# 控制流

Vela 支持业务逻辑常用控制流：`if`、`match`、循环、提前返回，以及 Option/Result 风格 helper。

## If

```vela
fn reward(enabled, amount) {
    if enabled {
        return amount;
    }
    return 0;
}
```

## Match

```vela
fn score(result) {
    match result {
        Check::Pass { score } => return score,
        Check::Fail { reason } => return 0,
    }
}
```

## For In

`for in` 支持普通 item 迭代和带 index 的迭代。

```vela
fn total(values) {
    let sum = 0;
    for index, value in values {
        sum += value + index;
    }
    return sum;
}
```

`for value in source` 会先求值一次 `source`，创建 iterator，然后不断前进直到耗尽。Array、set、map、string 和 range 是可重复 source。已有 iterator value 是一次性 cursor，所以对它执行循环会消耗它。

String 的 `for in` 产出 UTF-8 `char`，等价于 `text.chars()`。需要 byte 遍历时使用 `text.bytes()`。`for index, value in source` 是语法层循环 lowering；它不会分配 eager 的 `enumerate()` 集合。

循环变量按每次迭代独立作用域处理，所以闭包不会全部捕获最后一个循环值。
