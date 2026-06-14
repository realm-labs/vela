---
title: "Host Object Lifetime"
description: "Host object lifetime documentation for Vela."
---

This page will document host object identity, lifetime, and stale-reference behavior.

## Planned Coverage

- HostRef identity and generation checks.
- Call-scoped HostAccess.
- Stale host reference diagnostics.
- Why scripts never hold Rust `&mut T`.
- Runtime-owned script values versus Rust-owned host objects.
