---
title: "Sets"
description: "Sets documentation for Vela."
---

Sets store unique dynamic values. They are useful for membership checks and set
algebra on script-owned data. `Set<T>` is a builtin type-hint contract for
checked boundaries and typed mutation paths. Elements use the same `ValueKey`
policy as map keys: immutable leaf values compare by value, script heap objects
and host refs compare by identity, and transient values such as `PathProxy` are
rejected before mutation. `Function` is not accepted as a keyable type-hint
contract until callable identity is explicit.

## Construction And Membership

A set is usually created through standard library helpers or host-provided snapshot values. Membership APIs should be used instead of relying on array scans for uniqueness.

```vela
fn has_tag(tags, tag: String) -> bool {
    return tags.has(tag)
}
```

## Mutation

Set methods cover insertion, removal, and clearing. Mutating a script set changes the script heap value; mutating a host-owned set-like field goes through HostAccess.

```vela
fn mark_seen(seen, id: i64) {
    if !seen.has(id) {
        seen.add(id)
    }
    return seen
}
```

```vela
fn add_tag(tags: Set<String>, tag) {
    tags.add("checked") // statically compatible
    tags.add(tag)       // dynamic value, guarded before mutation
    return tags
}
```

## Set Operations

Standard methods provide operations such as intersection, union, difference,
and subset checks where supported by the runtime. Erased sets remain valid;
`Set<T>` is only needed when a boundary wants an element contract for a
keyable element type.

## Iteration

Sets are repeatable sequences over their values. Iteration order is runtime-defined and should not be used as persistent business semantics unless the API explicitly documents an ordering.
