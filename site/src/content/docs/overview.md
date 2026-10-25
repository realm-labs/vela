---
title: "Overview"
description: "The retained synchronous Vela language and removal boundary."
---

Vela is a synchronous, interpreted dynamic scripting language for domain-neutral host business logic. Hosts compile source or ordinary packages into linked bytecode and call scripts under finite budgets.

## Hot Reload First

Compatible reload preserves VM-owned script state. Active calls and retained closures keep their exact executable generation, including nested calls and state; new entries use the accepted new generation. Incompatible updates leave the active owner unchanged.

## Script-Owned Data

Ordinary structs, traits, modules, collections, iterators and controlled script reflection remain. Existing independent synchronous native functions and value conversion remain. Serialized inputs are script-owned; Rust explicitly receives results, without implicit write-back.

Caller-borrowed objects, views, leases, transparent host mutation, dependent extern state, Host constructors/methods, Service/Provider and script async/tasks are removed. No replacement opaque-object system, conversion API or re-entry model is supplied. Native LSP workers and protocol cancellation remain independent tooling.

See [quickstart](/quickstart/) and [embedding](/host/embedding-overview/).
