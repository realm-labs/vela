# Types And Values

Vela is dynamically typed at runtime, with type metadata used by analysis, diagnostics, and reflection.

## Scalar Values

Common scalar values are:

- `Null`
- `Bool`
- `Int`
- `Float`
- `String`

```vela
let enabled = true;
let level = 12;
let ratio = 1.5;
let name = "knight";
```

## Records And Enums

Script records and enums are first-class values managed by the VM.

```vela
struct Damage {
    amount: Int,
    source: String,
}

enum Check {
    Pass { score: Int },
    Fail { reason: String },
}
```

## Host Values

Rust-owned complex objects are not copied through `HostValue`. They are represented by host handles and paths. Script-owned structs can be serialized into or out of Rust through the serde snapshot path when that feature is enabled.
