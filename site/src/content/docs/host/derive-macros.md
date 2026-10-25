---
title: "Value And Native Macros"
description: "The synchronous, script-owned Vela boundary."
---

The existing `Value` derive generates structural script-owned value conversion and schema metadata. `export` exposes independent synchronous Rust functions taking and returning owned values. Removed Host derives, host methods, borrowed signatures and host constructors are not supported.

The retained `native_function` example uses `#[vela_macros::export(path = "game::bonus_macro")]` on a function with scalar arguments and a scalar result. Its collection example accepts owned maps and arrays. These registrations do not expose Rust references to scripts.

No replacement object, codec or re-entry API is introduced by the removal pass. See [native functions](/host/native-functions/).
