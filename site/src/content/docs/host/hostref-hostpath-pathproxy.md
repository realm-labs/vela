---
title: "Removed Transparent Host Paths"
description: "The synchronous, script-owned Vela boundary."
---

Transparent HostRef/HostPath/PathProxy access and dependent extern-state host bindings are removed, including reflected access, collection adapters and aliases. They are not available through replacement constructors or methods.

Ordinary script records, fields, collections, iterators and VM-owned `state` remain. Rust explicitly receives results from existing value APIs; script mutation never writes through to caller-borrowed Rust data. See [call arguments and results](/host/call-arguments-return-values/).
