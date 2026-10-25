---
title: "Grammar"
description: "A stable overview of the current Vela source grammar."
---

The grammar source of truth is `docs/grammar.ebnf`. This page summarizes the
current language surface; it is not a generated parser listing.

## Source Files

Vela source files use `.vela`. A source file may contain imports, attributes,
constants, VM-owned state declarations, functions, structs, enums, traits, and impl blocks.

```vela
use game::reward as reward

#[event("monster.kill")]
fn on_kill(ctx: Context, player: Player) {
    reward::grant(ctx, player, 10);
}
```

## Expressions And Statements

The expression grammar covers literals, arrays, maps, typed record literals,
field access, indexing, synchronous calls, unary and binary operators, ranges,
lambdas, `if`, `match`, and blocks.

Assignments require assignable targets: identifiers, script fields, or script
collection indexes. Compound assignment uses the same script-owned value
boundary as ordinary assignment; neither implicitly writes back to Rust.

## Patterns

Patterns are used by `match` and `for` bindings. The grammar supports wildcard,
literal, binding, path, tuple-variant, and record-variant patterns.

## Deliberate Exclusions

The grammar intentionally excludes script-language generics, script-visible
task or coroutine handles, manual resume, macro expansion, `eval`, classes,
monkey patching, Rust-style borrow syntax, `async fn`, `.await`, and detached
task operations. Scripts execute synchronously under finite budgets.

Type hints are metadata contracts and analysis inputs. They do not create
generic types or monomorphized script functions. Only selected builtin
contracts accept type arguments: `Array<T>`, `Set<T>`, `Map<K, V>`,
`Iterator<T>`, `Option<T>`, and `Result<T, E>`. `Map<K, V>` keys
and `Set<T>` elements must satisfy the runtime `ValueKey` keyability policy.
User-defined script generics and non-keyable container contracts are rejected.
