---
title: "Standard Library Overview"
description: "Standard library overview for Vela."
---

The Vela standard library is split between always-available value helpers and
host-installed capabilities. Pure helpers work on script-owned values such as
strings, arrays, maps, sets, bytes, `Option`, `Result`, and numbers. Host
helpers such as time, random, context events, and I/O are installed by the
embedding application so scripts cannot accidentally depend on process-global
state.

## Value Helpers

Most day-to-day APIs are methods on values. They use the same dispatch path as
script and host methods, but their implementation is native and registered with
stable method IDs.

```vela
fn main() {
    let tags = ["daily", "quest", "daily"].distinct().sort();
    let label = tags.join(":").to_upper();
    return label;
}
```

Collections provide both eager helpers and iterator views. Prefer iterator
pipelines when several transformations can be composed before materialization.

```vela
fn main() {
    let total = [1, 2, 3, 4]
        .iter()
        .filter(|value| value > 2)
        .map(|value| value * 10)
        .collect_array()
        .sum();
    return total;
}
```

## Modules And Constructors

Module functions create standard enum values, convert collections, and expose
numeric utilities. Examples include `option::some`, `result::ok`,
`set::from_array`, `bytes::from_hex`, and `math::*` helpers.

```vela
fn main() {
    let decoded = bytes::from_hex("ff00");
    let fallback = result::unwrap_or(decoded, b"");
    return fallback.len();
}
```

`Option` is used for ordinary absence, such as a missing lookup or failed parse.
`Result` is used when the script should inspect a recoverable failure payload.
VM traps, permission denials, and budget exhaustion are diagnostics, not
`Result::Err`.

## Capability Boundaries

The default standard natives do not grant nondeterministic or process-affecting
effects. Hosts opt into capabilities through the engine builder.

```vela
fn main(ctx: Context) {
    let now = time::now();
    let roll = math::random(1, 6);
    ctx.emit("roll.finished", roll);
    return roll;
}
```

In this example, `time::now`, `math::random`, and `Context.emit` only work when
the host registers the corresponding time, random, context schema, and event
capabilities. Sandboxed filesystem and stdout helpers are similarly opt-in.
