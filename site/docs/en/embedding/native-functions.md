# Native Functions

Native functions let scripts call Rust code registered by the host.

## Simple Native Function

Rust registers a function with metadata and a callable thunk. Macro-generated bindings can produce the metadata and conversion layer.

```rust
#[script_function(module = "game", name = "bonus")]
fn bonus(base: i64, multiplier: i64) -> i64 {
    base * multiplier
}
```

Script usage:

```vela
fn main() {
    return game::bonus(10, 3);
}
```

## Effects And Capabilities

Native functions declare effects such as host read, host write, time, random, reflection, or I/O. The engine capability set decides whether the call is allowed.

This keeps scripts expressive while making host-visible side effects explicit.
