---
title: "Error And Diagnostic Codes"
description: "Common Vela diagnostic families and how to interpret them."
---

Vela diagnostics are structured by subsystem. The exact code list is still
stabilizing, so this page groups the durable families rather than pretending a
complete generated catalog exists.

## Parse And Semantic Errors

Parser and semantic errors cover invalid syntax, unresolved names, duplicate
declarations, invalid assignment targets, rejected generic type syntax,
top-level side effects, and invalid module imports.

These errors should include source spans and related locations when available.

## Runtime Errors

Runtime errors cover type guard failures, bad calls, arithmetic failures,
budget exhaustion, stack depth limits, missing entries, and value conversion
failures.

## Host And Reflection Errors

Host and reflection errors cover field not found, field not writable,
permission denied, required capability missing, stale host ref generation,
unknown reflected item, and reflect-call denial.

## Hot Reload Errors

Hot reload diagnostics cover compile failures, ABI mismatches, schema
incompatibilities, effect/access expansion, source graph problems, and rejected
top-level side effects.

Reports should state that the previous active version remains current when an
update is rejected.
