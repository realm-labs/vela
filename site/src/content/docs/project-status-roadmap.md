---
title: "Project Status And Roadmap"
description: "The retained synchronous Vela language and removal boundary."
---

Vela is a pre-release synchronous interpreted language. The authoritative product, architecture and removal contracts live in `docs/goal.md`, `docs/architecture.md` and `docs/runtime-simplification-plan.md`; repository-wide validation status belongs to `docs/progress.md`.

The removal pass deletes Service/Provider, script async/tasks, borrowed roots/arguments/returns/views, scoped resources, transparent paths, dependent extern bindings and Host construction/method surfaces. Tooling uses strict schema 4; incompatible old executable artifacts are rejected before activation. No opaque-object, conversion or re-entry replacement is implemented.

Retained work includes ordinary script language/container behavior, exact-generation script reload, independent native/value calls, reflection, GC/budgets, interpreter optimization, CLI/browser execution, native LSP/editor scheduling and planned debugger support. JIT and borrowed-host write-through are not product tracks.

Changed fixtures require current non-native audits. Historical editor captures do not prove current behavior; this removal does not claim fresh native captures or full repository acceptance.
