# Syntax

Vela uses a compact Rust-like syntax without being Rust. The language is dynamic, but the parser and compiler preserve enough metadata for diagnostics, reflection, and hot reload compatibility.

## Items

```vela
struct Reward {
    item: string,
    amount: int,
}

enum RewardResult {
    Granted { item: string, amount: int },
    Denied { reason: string },
}

fn grant(reward) {
    return RewardResult::Granted {
        item: reward.item,
        amount: reward.amount,
    };
}
```

Top-level script files can define functions, structs, enums, traits, impls, imports, constants, and `global` declarations.

## Expressions

Blocks, `if`, `match`, constructors, calls, method calls, indexing, arrays, maps, sets, and lambdas are expressions in the language surface used by the compiler.

```vela
let score = if reward.amount > 0 {
    reward.amount * 10
} else {
    0
};
```

## Type Hints

Type hints are metadata for diagnostics, reflection, and ABI checks. They are not script-language generics.

```vela
fn preview(reward: Reward) -> int {
    return reward.amount;
}
```
