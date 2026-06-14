---
title: "Strings And Bytes"
description: "Strings And Bytes documentation for Vela."
---

Strings are UTF-8 text values. Bytes are immutable binary buffers. Vela keeps the two categories separate so text APIs and binary APIs can have clear host and standard-library contracts.

## String Forms

Plain strings use `"..."`, multiline strings use `"""..."""`, and interpolated strings must be explicitly prefixed with `f`. Plain strings never interpolate.

```vela
fn greeting(name: string) -> string {
    return f"hello {name}"
}

fn template() -> string {
    return """
line one
line two
"""
}
```

## Text Methods

String methods cover predicates, transforms, search, split, parse helpers, and explicit traversal. `len()`, `find()`, and `slice(start, end)` are byte-indexed; `chars()` is UTF-8 character traversal.

```vela
fn parse_count(text: string) {
    return text.trim().parse_i64()
}
```

## Bytes

Byte strings use `b"..."`. Indexing a bytes value yields `u8`. Byte APIs should use explicit endian helpers rather than host-endian reads.

```vela
fn first_byte(packet: bytes) -> u8 {
    return packet[0]
}
```

## Boundaries

Strings and bytes are heap-backed runtime values. Host APIs should declare whether they expect text or binary data; Vela will not silently convert between them.
