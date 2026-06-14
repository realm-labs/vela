---
title: "Safe Points"
description: "Where Vela is allowed to make a staged update visible."
---

Safe points are the only places where a staged hot reload update can become the
current program version. Vela does not interrupt arbitrary bytecode instructions
to replace function bodies.

## Purpose

Safe points give the host a predictable boundary for code replacement. They
prevent partially executed functions from observing a different instruction
stream or registry halfway through a call.

Common host safe points include:

```text
end of event
tick boundary
between queued jobs
explicit runtime.check_reload()
```

## Old And New Calls

When a safe point accepts an update, only future calls use the new version.
Active calls keep their old `CodeObject` and old metadata snapshot until they
return.

This means a long-running event handler can finish with the code it started
with, while the next event handler uses the updated code.

## Host Responsibilities

The host should place safe points at boundaries where repeated work naturally
returns control to Rust. It should not expose unbudgeted infinite script loops;
execution budgets still apply while old frames are running.

## Debugging And Reporting

Safe-point reports identify whether a staged update was applied, rejected, or
not present. Hosts should log these reports with source labels so operators can
connect a rejection to the file or deployment that produced it.
