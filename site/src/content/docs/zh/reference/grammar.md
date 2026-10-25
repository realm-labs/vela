---
title: "语法参考"
description: "当前 Vela 源码语法的稳定概要。"
---

语法的事实来源是 `docs/grammar.ebnf`。本页总结当前语言表面，不是自动生
成的 parser 清单。

## 源文件

Vela 源文件使用 `.vela`。一个源文件可以包含 imports、attributes、consts、
VM-owned state declarations、functions、structs、enums、traits 和 impl blocks。

```vela
use game::reward as reward

#[event("monster.kill")]
fn on_kill(ctx: Context, player: Player) {
    reward::grant(ctx, player, 10);
}
```

## 表达式和语句

表达式语法覆盖 literals、arrays、maps、typed record literals、field
access、indexing、同步 calls、unary/binary operators、ranges、lambdas、
`if`、`match` 和 blocks。

赋值目标必须是可赋值目标：identifier、脚本 field 或脚本集合 index。
复合赋值和普通赋值使用相同的脚本自有值边界，都不会隐式写回 Rust。

## Patterns

`match` 和 `for` binding 使用 patterns。语法支持 wildcard、literal、
binding、path、tuple-variant 和 record-variant patterns。

## 有意排除

语法有意排除脚本侧泛型、脚本可见的 task/coroutine handle、手动 resume、
macro expansion、`eval`、classes、monkey patching 和 Rust-style borrow
syntax、`async fn`、`.await` 和分离任务操作。脚本在有限预算内同步执行。

Type hints 是 metadata contract 和分析输入。它们不会创建泛型类型，也不
会生成 monomorphized script functions。
