---
title: "Non-Goals And Constraints"
description: "The retained synchronous Vela language and removal boundary."
---

Vela has no general script-language generics, overload sets, arbitrary `eval`, monkey patching, runtime type-structure mutation, moving GC, structural state migration, JIT, script async/await or detached tasks.

Caller-borrowed Rust roots, arguments, returns, iterators, retained views/leases and transparent host paths are removed, including wrappers around call-scoped borrows. Dependent extern state, Host constructors and Host methods are removed rather than replaced. Rust host state remains outside script GC.

Existing independent synchronous native calls and script-owned value conversion remain. Serialized values never imply Rust write-back. Ordinary VM-owned state, collections, traits, reflection and exact-generation reload remain.

Object storage, identity, invalidation, re-entry and opaque schemas are separate deferred design work, not features introduced by this pass. LSP workers and protocol cancellation are tooling, not script asynchronous execution.
