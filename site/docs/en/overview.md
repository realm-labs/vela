# Overview

Vela is a Hot Reload First dynamic scripting language implemented in Rust. It is built for host-owned business logic: the Rust host owns durable state, while scripts express rules that read, compute, and safely mutate that state through controlled host access.

The language is intentionally not dynamic Rust and not a Lua clone. It keeps a scripting-language workflow while preserving Rust-friendly embedding contracts.

## What It Optimizes For

- Host state remains owned by Rust.
- Scripts can read and write registered host objects through `HostRef`, `HostPath`, `PathProxy`, and `HostAccess`.
- Function and module code can be hot reloaded without invalidating active call frames.
- Reflection can query metadata and perform controlled reads, writes, and calls, but cannot mutate type structure.
- The runtime is embeddable: hosts configure capabilities, native functions, schemas, budgets, globals, and reload policy.

## Non-Goals

- No script-language generics.
- No real Rust `&mut T` exposed to scripts.
- No monkey patching or runtime type-structure mutation.
- No MVP JIT, script async/coroutines, moving GC, or full LSP.

## Current Shape

The current prototype already supports parsing, bytecode compilation, VM execution, host access, reflection, standard native functions, script-owned globals, serde snapshots, cached function handles, and standalone embedding examples.

Use the playground to try script-owned data, records, maps, sets, methods, standard helpers, and runtime diagnostics in the browser.
