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

循环变量按每次迭代独立作用域处理，所以闭包不会全部捕获最后一个循环值。
