---
title: "Lexical Structure And Comments"
description: "Lexical Structure And Comments documentation for Vela."
---

Vela source files are UTF-8 text files with the `.vela` extension. The syntax is line-oriented where simple declarations and statements can end at a newline or semicolon, while block forms such as `fn`, `struct`, `enum`, `trait`, `impl`, `if`, `match`, and `for` are self-terminating.

## Source Shape

Identifiers begin with `_` or an ASCII letter and continue with ASCII letters, digits, or `_`. Keywords such as `fn`, `struct`, `match`, `self`, `true`, `false`, and `null` are reserved and cannot be used as ordinary names.

```vela
#!/usr/bin/env vela

// File-level declarations are ordinary items.
pub const BASE_XP: i64 = 100

fn award(level: i64) -> i64 {
    return BASE_XP + level * 10
}
```

## Comments

Line comments start with `//` and run to the end of the line. Block comments use `/* ... */` and may be nested, which makes it practical to disable a block that already contains comments.

```vela
fn classify(value: i64) -> String {
    /* Nested comments are valid:
       /* disabled note */
    */
    if value > 0 {
        return "positive"
    }
    return "zero-or-negative"
}
```

## Boundaries

Vela does not include preprocessor directives, macro expansion, `eval`, or runtime parsing of generated source strings. Attributes are parsed as metadata, but their meaning is defined by later semantic phases, host registration, or tooling.
