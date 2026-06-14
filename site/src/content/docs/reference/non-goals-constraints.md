---
title: "Non-Goals And Constraints"
description: "Design constraints that keep Vela embeddable and hot-reload safe."
---

Vela is a dynamic scripting language for host-owned business logic. It is not
dynamic Rust and it is not a Lua clone. Some features are intentionally outside
the first release.

## Language Non-Goals

The MVP does not include script-language generics, function overloading by type,
a Rust-style borrow checker, arbitrary `eval`, macros, monkey patching, classes,
script threads, async/coroutine hot reload, or JIT compilation.

## Host Boundary Constraints

Scripts never receive real Rust `&mut T`. Host mutation must flow through
`HostRef`, `HostPath`, `PathProxy`, `HostAccess`, and the host adapter. Host
state is not placed under the script GC.

## Reflection Constraints

Reflection may query metadata and perform controlled reads, writes, and calls.
It may not mutate type structure, replace methods, add fields, or create a
monkey-patching system.

## Runtime Constraints

Execution must be budgeted. Optimizations must preserve source diagnostics, GC
roots, hot reload versioning, reflection permissions, and host access checks.
JIT is a post-MVP backend goal, not a requirement for the first interpreter.
