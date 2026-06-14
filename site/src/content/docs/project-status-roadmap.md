---
title: "Project Status And Roadmap"
description: "Project Status And Roadmap documentation for Vela."
---

Vela is a pre-release implementation with a broad runnable prototype. The stable product direction is defined in `docs/goal.md`; the technical contract is defined in `docs/architecture.md`; current milestone status is tracked in `docs/progress.md`.

## Available Now

The current codebase includes source parsing, HIR lowering, bytecode compilation, VM execution, execution budgets, non-moving GC foundations, arrays, maps, sets, strings, Option/Result helpers, modules, runtime globals, standard natives, reflection metadata, host registration, HostAccess write-through, and hot reload workflows.

It also includes a browser Playground, a documentation site, standalone embedding examples, conformance-style tests, benchmark harnesses, and parser fuzzing infrastructure.

## Active Work

Current implementation work is centered on interpreter and inline-cache performance, host-boundary fast paths, and keeping the runtime architecture clean while preserving the product contracts around HostAccess, hot reload compatibility, diagnostics, and controlled reflection.

The performance goal is practical non-JIT interpreter performance first. JIT work is a post-MVP track, not a requirement for the current release.

## Explicit Non-Goals For The MVP

The MVP does not include script-language generics, monkey patching, arbitrary `eval`, script async/coroutines, JIT compilation, a full LSP, runtime type-structure mutation through reflection, or exposing Rust `&mut T` references to scripts.

These limits are intentional. They keep hot reload, host ownership, capability enforcement, and diagnostics tractable.

## Documentation Status

This site is being filled in by section. Pages should describe current behavior and stable design intent. When a page talks about future work, it should say so directly and avoid presenting future features as implemented.
