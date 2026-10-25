---
title: "Core Concepts"
description: "The retained synchronous Vela language and removal boundary."
---

## Engine, Runtime And Values

An Engine owns compiled/linked metadata and independent native/value registrations. A Runtime owns a script heap, VM state, caches, budgets and executable-generation ownership. `CallArgs` passes values to synchronous calls; `CallOptions` bounds execution.

Script structs, enums, traits, modules, collections and iterators remain ordinary language values. Existing serde conversion makes script-owned snapshots, with no implicit mutation of the Rust input. Rust host state does not enter script GC.

## State And Reload

Ordinary VM `state` persists through compatible reload. Old calls and closures retain their exact old generation and state. Dependent `extern state` host bindings are removed; their removal does not remove ordinary state.

## Reduced Boundary

There are no caller-borrowed roots, returns, views, leases, transparent paths or Host construction/method registrations. Missing opaque/object interoperability is separately deferred; this pass adds no replacement APIs. Script reflection remains controlled, structurally read-only and unable to reconstruct removed host paths.
