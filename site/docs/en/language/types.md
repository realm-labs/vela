# Types And Values

Vela is dynamically typed at runtime, with type metadata used by analysis, diagnostics, and reflection.

## Scalar Values

Common scalar values are:

- `null`
- `bool`
- `char`
- `i8`, `i16`, `i32`, `i64`
- `u8`, `u16`, `u32`, `u64`
- `f32`, `f64`
- `string`
- `bytes`

```vela
let enabled = true;
let marker = '!';
let level = 12i64;
let ratio = 1.5f64;
let name = "knight";
let payload = b"ok";
```

## Records And Enums

Script records and enums are first-class values managed by the VM.

```vela
struct Damage {
    amount: i64,
    source: string,
}

enum Check {
    Pass { score: i64 },
    Fail { reason: string },
}
```

## Host Values

Rust-owned complex objects are not copied through `HostValue`. They are represented by host handles and paths. Script-owned structs can be serialized into or out of Rust through the serde snapshot path when that feature is enabled.
