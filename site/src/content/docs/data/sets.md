---
title: "Sets"
description: "Sets documentation for Vela."
---

Sets store unique dynamic values. They are useful for membership checks and set algebra on script-owned data.

## Construction And Membership

A set is usually created through standard library helpers or host-provided snapshot values. Membership APIs should be used instead of relying on array scans for uniqueness.

```vela
fn has_tag(tags, tag: string) -> bool {
    return tags.contains(tag)
}
```

## Mutation

Set methods cover insertion, removal, and clearing. Mutating a script set changes the script heap value; mutating a host-owned set-like field goes through HostAccess.

```vela
fn mark_seen(seen, id: i64) {
    if !seen.contains(id) {
        seen.insert(id)
    }
    return seen
}
```

## Set Operations

Standard methods provide operations such as intersection, union, difference, and subset checks where supported by the runtime. These operations remain dynamic and do not require `Set<T>` syntax.

## Iteration

Sets are repeatable sequences over their values. Iteration order is runtime-defined and should not be used as persistent business semantics unless the API explicitly documents an ordering.
