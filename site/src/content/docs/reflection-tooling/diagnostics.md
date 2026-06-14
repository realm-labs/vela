---
title: "Diagnostics"
description: "How Vela reports parser, semantic, runtime, reflection, and reload errors."
---

Diagnostics are part of the runtime contract. Vela errors should carry enough
structure for CLI output, editor labels, hot reload reports, and host logs.

## Diagnostic Data

A useful diagnostic includes:

```text
error kind
source span
message
related locations
candidate names
repair hint
call stack when runtime execution was involved
```

Source spans are available for script declarations and many runtime errors.
Host-generated schemas may omit source spans.

## Candidate Hints

Reflection and host schema errors should include candidates when possible. A
misspelled field can point to nearby fields from the registered type.

```text
FieldNotFound
type: Player
field: levle
candidates: ["level"]
```

## Runtime Context

Runtime diagnostics should preserve call-stack and source information across
script calls, native calls, host access, and reflection. Hot reload diagnostics
should include version/update context when a candidate cannot be applied.

## CLI Rendering

`vela_cli` renders source errors and VM errors for script execution. It is a
consumer of structured diagnostics, not the only representation.
