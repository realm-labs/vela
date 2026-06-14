---
title: "Vela Documentation"
description: "Documentation for Vela."
sidebar:
  hidden: true
---

# Vela Documentation

Vela is a Hot Reload First dynamic scripting language for Rust applications that keep ownership of their durable state. Scripts describe business rules; the Rust host registers types, native functions, capabilities, budgets, and hot reload policy.

Start with [Overview](./overview/) for the model, [Quickstart](./quickstart/) for a runnable script, or open the [Playground](./playground/) to try language features in the browser.

## Main Paths

- Learn the language surface in [Language Basics](./language/lexical-structure-comments/).
- Understand host integration through [Embedding Overview](./host/embedding-overview/) and [HostAccess Write-Through Model](./host/hostaccess-write-through/).
- Read the hot reload contract in [Hot Reload Model](./hot-reload/model/).
- Check current implementation scope in [Project Status And Roadmap](./project-status-roadmap/).

## Current Scope

The current implementation is a runnable pre-release system. It has parsing, HIR, bytecode, VM execution, standard library helpers, HostAccess, reflection, hot reload workflows, examples, benchmarks, and a WASM playground. The documentation describes the current contract and calls out non-goals where a feature is intentionally outside the MVP.
