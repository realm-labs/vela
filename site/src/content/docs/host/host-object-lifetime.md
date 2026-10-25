---
title: "Removed Host Object Surface"
description: "The synchronous, script-owned Vela boundary."
---

Caller-borrowed Rust objects, borrowed arguments and returns, retained views, leases, and explicit borrow release are removed. A wrapper around a caller-stack borrow is not supported.

No independent old opaque construction/method surface survives this removal. Object storage, identity, invalidation, re-entry and opaque reflection/schema design are separate follow-up questions, not APIs implemented by this pass.

Use existing synchronous native value calls and script-owned values. Rust state remains outside script GC; serialization does not create implicit write-back. See [snapshot values](/host/serde-snapshot-values/).
