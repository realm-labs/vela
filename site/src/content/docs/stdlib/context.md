---
title: "Native Call Context"
description: "The synchronous, script-owned Vela boundary."
---

The Rust `NativeCallContext` belongs to one synchronous native invocation. Existing value-oriented native operations, permissions and budget accounting remain; it is not a script object or a borrowed-host injection route.

Transparent adapter/path access, scoped host resources and borrowed arguments or returns are removed. Hosts may do their own external work and then call a script synchronously. Script async/tasks and detached continuations are not supported.

Ordinary VM-owned state is separate from Rust host state. See [embedding](/host/embedding-overview/) and the retained `native_function` and `script_state` examples.
