---
title: "Time"
description: "Time standard library documentation for Vela."
---

Time is host-provided and deterministic. Vela does not read wall-clock time
directly from the process; the embedding application installs a clock and
grants the time effect.

## Installed Clock Functions

When the host calls `with_time_clock(now, tick)`, scripts can call
`time::now`, `time::tick`, and `time::elapsed_since`.

```vela
fn main() {
    let start = 1_699_999_990;
    return time::elapsed_since(start) + time::tick();
}
```

`time::now()` returns the configured timestamp. `time::tick()` returns the
configured logical tick. `time::elapsed_since(start)` returns
`time::now() - start` and traps on invalid input or overflow.

## Capability And Replay

Time functions carry the `time` effect. Hosts can deny that effect with a
capability profile, even if the functions are registered for compilation.

```vela
fn main(ctx: Context) {
    if time::tick() > ctx.tick {
        return time::now();
    }
    return ctx.now;
}
```

For replay and tests, pass the same `now` and `tick` values to the engine. The
standard context object exposes the same deterministic values through
`Context.now` and `Context.tick` when the host registers the context schema.
