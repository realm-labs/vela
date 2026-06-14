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
field access, indexing, calls, unary and binary operators, ranges, lambdas,
`if`, `match`, and blocks.

Assignments require assignable targets: identifiers, fields, indexes, or host
path proxies. Compound assignment uses the same write boundary as ordinary
assignment.

## Patterns

Patterns are used by `match` and `for` bindings. The grammar supports wildcard,
literal, binding, path, tuple-variant, and record-variant patterns.

## Deliberate Exclusions

The grammar intentionally excludes script-language generics, async/coroutines,
macro expansion, `eval`, classes, monkey patching, and Rust-style borrow
syntax.

Type hints are metadata contracts and analysis inputs. They do not create
generic types or monomorphized script functions. Only selected builtin
contracts accept type arguments: `Array<T>`, `Set<T>`, `Map<String, V>`,
`Iterator<T>`, `Option<T>`, and `Result<T, E>`. `Set<T>` is limited to the
runtime's set-keyable element contracts: `null`, `bool`, `i64`, `f64`, and
`String`.
