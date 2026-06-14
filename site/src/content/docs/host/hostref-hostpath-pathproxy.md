---
title: "HostRef, HostPath, PathProxy"
description: "HostRef, HostPath, PathProxy documentation for Vela."
---

This chapter belongs to **Host Integration**.

## Goals

TODO: document the semantics, examples, host boundary behavior, and common errors for HostRef, HostPath, PathProxy.

## Design Boundaries

- No script-language generics.
- No real Rust `&mut T` is exposed to scripts.
- Host state mutation must go through the HostAccess boundary.

## Example

TODO: add runnable Vela or Rust embedding examples.
