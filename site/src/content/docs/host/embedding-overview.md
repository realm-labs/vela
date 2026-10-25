---
title: "Embedding Overview"
description: "The synchronous, script-owned Vela boundary."
---

Vela executes synchronously through `Engine` and `Runtime`. Compile source or an ordinary package, call a script entry under finite `CallOptions`, and explicitly read its script-owned result.

Existing independent synchronous native functions and value conversion remain. `serde_value` demonstrates snapshot conversion with no implicit write-back. `script_state` demonstrates VM-owned state; it is not an extern Rust binding. `modules`, `package_dependency_demo` and the hot-reload examples remain ordinary source workflows.

Caller-borrowed objects, transparent host mutation, Host construction/method registrations, dependent extern state, script async/tasks and Service/Provider execution are removed. No replacement opaque-object API is provided.

See [call arguments and results](/host/call-arguments-return-values/) and [native functions](/host/native-functions/).
