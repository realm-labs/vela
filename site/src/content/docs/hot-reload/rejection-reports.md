---
title: "Rejection Reports"
description: "How Vela explains failed hot reload updates."
---

Rejected updates are expected operational events. A rejection means the runtime
kept the previous program version active and produced a report explaining why
the candidate was not safe to apply.

## Report Shape

Reports are structured for machines and renderable for humans. They should carry
the update status, source labels, spans when available, the old and candidate
version identities, and specific compatibility failures.

Rendered lines are useful for logs, but hosts should prefer structured fields
when building dashboards or deployment gates.

## Common Reasons

Typical rejection reasons include:

```text
syntax or semantic compile error
missing module or unresolved import
duplicate declaration
function ABI mismatch
schema ID reuse
field or variant incompatibility
effect or permission expansion
top-level source side effect
```

## Runtime Safety

A rejected update never partially applies. Active frames keep their old code,
new calls continue using the previous current version, and the staged candidate
is discarded or left for host-specific handling.

## Operator Guidance

Good deployment tooling should display the primary error, related locations,
and repair hints. For example, an exported event function parameter removal
should point to both the old ABI and the new declaration that caused the
mismatch.
