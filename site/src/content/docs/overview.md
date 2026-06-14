---
title: "Overview"
description: "Overview documentation for Vela."
---

Vela is a Hot Reload First dynamic scripting language for Rust host-owned business logic. Scripts express rules while the Rust host owns durable state and exposes controlled read/write access through HostAccess.

## Design Focus

- Rust hosts keep ownership of durable state.
- Scripts mutate host state through HostRef, HostPath, PathProxy, and HostAccess.
- Hot reload is versioned at function and module boundaries; active frames keep running old CodeObjects.
- Reflection can query metadata and perform controlled reads, writes, and calls without mutating type structure.

## Documentation Status

This site now establishes the full documentation outline. Detailed chapters will be filled in as the language, standard library, and embedding APIs stabilize.
