---
title: "语法参考"
description: "当前 Vela 源码语法的稳定概要。"
---

语法的事实来源是 `docs/grammar.ebnf`。本页总结当前语言表面，不是自动生
成的 parser 清单。

## 源文件

Vela 源文件使用 `.vela`。一个源文件可以包含 imports、attributes、consts、
globals、functions、structs、enums、traits 和 impl blocks。

```vela
use game::reward as reward

#[event("monster.kill")]
fn on_kill(ctx: Context, player: Player) {
    reward::grant(ctx, player, 10);
}
```

## 表达式和语句

表达式语法覆盖 literals、arrays、maps、typed record literals、field
access、indexing、calls、unary/binary operators、ranges、lambdas、`if`、
`match` 和 blocks。

赋值目标必须是可赋值目标：identifier、field、index 或 host path proxy。
复合赋值和普通赋值使用相同写入边界。

## Patterns

`match` 和 `for` binding 使用 patterns。语法支持 wildcard、literal、
binding、path、tuple-variant 和 record-variant patterns。

## 有意排除

语法有意排除脚本侧泛型、async/coroutines、macro expansion、`eval`、
classes、monkey patching 和 Rust-style borrow syntax。

Type hints 是 metadata contract 和分析输入。它们不会创建泛型类型，也不
会生成 monomorphized script functions。
