---
title: "Source Files, Directories, And Changed Files"
description: "How Vela hosts compile single-file and multi-file updates."
---

Vela supports source-based reload workflows where the host compiles a new
program version from one file, a directory, or a set of changed files. The
result is still a whole candidate version that must pass compatibility checks.

## Source Identity

Compiler APIs use source labels and source identities internally so diagnostics,
spans, module ownership, and reload reports can point back to the right file.
Application-level users should not need to invent stable IDs by hand for normal
file or directory workflows.

## Single File

A single-file update is useful for examples, playgrounds, and small embedded
rules. It is compiled as a complete candidate program for that source unit, then
staged and applied at a safe point.

## Directories And Changed Files

For larger projects, hosts should compile from the project root or from a
change set that can be resolved back into the full module graph. Updating more
than one module is valid when the resulting graph is coherent.

Changed-file workflows should still validate imports, duplicate declarations,
module visibility, top-level side-effect restrictions, ABI, schema, and effects
before advancing the active version.

## Source Boundary Rejections

An update can be rejected because the changed source cannot be related safely to
the existing module graph. Examples include missing modules, ambiguous duplicate
definitions, incompatible exported declarations, or source changes that require
host approval.
