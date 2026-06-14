---
title: "Standard Library"
description: "Overview of Vela's current domain-neutral standard library surface."
---

Vela's standard library is domain-neutral. Game, billing, workflow, or product
concepts should come from host registration, native functions, schemas, and
examples rather than builtin language features.

## Core Value Helpers

The current standard surface includes helpers and methods for strings, bytes,
arrays, maps, sets, ranges, iterators, Option-style values, Result-style values,
math, time, random, context helpers, and controlled I/O.

Examples in the repository exercise modules such as `math`, `time`, `option`,
`result`, `set`, `bytes`, and `io`.

## Capabilities

Effectful APIs require explicit host capabilities. Time, random, event emit,
host read/write/call, standard I/O, and file I/O should be configured by the
embedding host.

Sandboxed file APIs stay relative to the configured filesystem root and require
I/O capabilities.

## Iteration And Collections

Arrays, maps, sets, ranges, strings, bytes, and host-provided iterables can
participate in `for` loops and selected iterator-style helpers. Lazy iterator
adapters and terminal helpers are benchmarked separately from collection
materialization.

## Reference Status

This is an overview, not a complete generated standard library index. The
stable long-term goal is to expose names, params, return hints, docs, effects,
and reflection access through metadata so this page can eventually be generated.
