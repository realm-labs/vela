---
title: "ABI And Schema Compatibility"
description: "ABI And Schema Compatibility documentation for Vela."
---

This chapter belongs to **Hot Reload**.

## Goals

TODO: document the semantics, examples, host boundary behavior, and common errors for ABI And Schema Compatibility.

## Design Boundaries

- No script-language generics.
- No real Rust `&mut T` is exposed to scripts.
- Host state mutation must go through the HostAccess boundary.

## Example

TODO: add runnable Vela or Rust embedding examples.
