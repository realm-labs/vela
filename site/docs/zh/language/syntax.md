# 语法

Vela 使用紧凑的 Rust-like 语法，但它不是 Rust。语言是动态的，不过 parser 和 compiler 会保留足够的元数据，用于诊断、反射和热更新兼容性检查。

## 顶层声明

```vela
struct Reward {
    item: string,
    amount: int,
}

enum RewardResult {
    Granted { item: string, amount: int },
    Denied { reason: string },
}

fn grant(reward: Reward) -> RewardResult {
    return RewardResult::Granted {
        item: reward.item,
        amount: reward.amount,
    };
}
```

顶层脚本文件可以定义 function、struct、enum、trait、impl、import、const 和 `global` 声明。

## 表达式

Block、`if`、`match`、constructor、call、method call、index、array、map、set 和 lambda 都是编译器处理的表达式形态。

```vela
let score = if reward.amount > 0 {
    reward.amount * 10
} else {
    0
};
```

## 类型提示

类型提示是诊断、反射和 ABI 检查使用的元数据。它不是脚本侧泛型。

```vela
fn preview(reward: Reward) -> int {
    return reward.amount;
}
```
