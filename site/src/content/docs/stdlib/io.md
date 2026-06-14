---
title: "I/O"
description: "I/O standard library documentation for Vela."
---

I/O is not installed by default. Hosts opt into stdout and sandboxed filesystem
helpers, then grant `io_read` and/or `io_write` capabilities. This keeps script
execution deterministic by default and makes process effects explicit.

## Stdout

`io::print(value)` writes a formatted value to stdout. `io::println(value)`
writes the value plus a newline. Both return `Result`.

```vela
fn main() {
    let printed = io::println("hello from Vela");
    return printed.is_ok();
}
```

Ordinary output failures become `Result::Err(IoError)`. Type errors, missing
functions, and denied capabilities remain VM diagnostics.

## Sandboxed Filesystem

`fs::read_to_string(path)` reads a UTF-8 file inside the configured sandbox.
`fs::write_string(path, text)` writes a UTF-8 file inside the sandbox. Both
return `Result`.

```vela
fn main() {
    let input = result::unwrap_or(fs::read_to_string("input.txt"), "missing");
    fs::write_string("output.txt", "done");
    return input.len();
}
```

Paths must be relative. Empty paths, absolute paths, drive prefixes, and
parent-directory escapes are rejected with `Result::Err(IoError)`.

## Host Installation

The Rust host installs these helpers explicitly.

```rust
let engine = Engine::builder()
    .with_standard_natives()
    .capability(Capability::IoRead)
    .capability(Capability::IoWrite)
    .with_stdio()
    .with_fs_io(root)
    .build()?;
```

Use I/O helpers for tools, demos, and controlled scripts. Server gameplay
scripts should usually communicate through host APIs, events, or explicit
state adapters instead of direct filesystem access.
