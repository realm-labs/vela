---
title: "C API"
description: "Current status of Vela's external C ABI surface."
---

The `vela_c_api` crate is the external binary interface for non-Rust hosts. It
is intentionally separate from hot reload ABI: C ABI describes native embedding
symbols, while hot reload ABI describes script compatibility.

## Current Surface

The first slice exposes:

```text
opaque engine handles
opaque runtime handles
API version query
source compilation
no-argument entry calls
scalar result values
ABI-owned string/value cleanup
```

The exported status codes distinguish null pointer, invalid UTF-8, engine,
compile, runtime, unsupported value, and panic failures.

## Value Ownership

Strings and byte buffers returned through the C ABI are owned by the ABI caller
until freed with the matching Vela cleanup function. Opaque engine/runtime
handles must also be released through their corresponding free functions.

## Boundaries

The C API does not expose Rust references. Future host object vtables and
aggregate value handles should preserve the same safety rule: host mutation
must cross an explicit adapter boundary.

## Status

The C API is early and intentionally small. Treat this page as a capability
summary until the ABI is versioned and release-hardened.
