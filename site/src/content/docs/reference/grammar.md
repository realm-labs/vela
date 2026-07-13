---
title: "Grammar"
description: "A stable overview of the current Vela source grammar."
---

The grammar source of truth is `docs/grammar.ebnf`. This page summarizes the
current language surface; it is not a generated parser listing.

## Source Files

Vela source files use `.vela`. A source file may contain imports, attributes,
constants, globals, functions, structs, enums, traits, and impl blocks.

```vela
use game::reward as reward

#[event("monster.kill")]
fn on_kill(ctx: Context, player: Player) {
    reward::grant(ctx, player, 10);
}
```

## Expressions And Statements

The expression grammar covers literals, arrays, maps, typed record literals,
field access, indexing, calls, postfix call `.await`, unary and binary
operators, ranges, lambdas, `if`, `match`, and blocks. Module functions and
script methods may be declared with `async fn`.

Assignments require assignable targets: identifiers, fields, indexes, or host
path proxies. Compound assignment uses the same write boundary as ordinary
assignment.

## Patterns

Patterns are used by `match` and `for` bindings. The grammar supports wildcard,
literal, binding, path, tuple-variant, and record-variant patterns.

## Deliberate Exclusions

The grammar intentionally excludes script-language generics, script-visible
task or coroutine handles, manual resume, macro expansion, `eval`, classes,
monkey patching, and Rust-style borrow syntax. Async execution remains
sequential: `.await` suspends an `async fn` until its call completes without
exposing script-level concurrency.

Type hints are metadata contracts and analysis inputs. They do not create
generic types or monomorphized script functions. Only selected builtin
contracts accept type arguments: `Array<T>`, `Set<T>`, `Map<K, V>`,
`Iterator<T>`, `Option<T>`, and `Result<T, E>`. `Map<K, V>` keys
and `Set<T>` elements must satisfy the runtime `ValueKey` keyability policy.
User-defined script generics and non-keyable container contracts are rejected.
