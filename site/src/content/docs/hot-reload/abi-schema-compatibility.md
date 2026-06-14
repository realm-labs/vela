---
title: "ABI And Schema Compatibility"
description: "Rules that decide whether a Vela hot reload update is safe."
---

Hot reload compatibility is conservative. An update is accepted only when new
code can coexist with old active frames, host schemas, reflection metadata, and
registered capabilities.

## Function ABI

Function body changes are the normal reload case. Local variables, private
helpers, and compatible new public functions are allowed.

Exported or host-called functions are checked more strictly. Removing
parameters, reordering parameters, changing required return behavior, or
expanding effects without host approval can be rejected.

## Schema Compatibility

Structs, enums, traits, fields, methods, variants, modules, and functions use
stable IDs. Names help diagnostics, but compatibility is not based on names
alone.

Usually safe changes include:

```text
add a field with a default
rename a field while preserving FieldId
add a method
add an enum variant
add a private helper function
```

Usually rejected changes include:

```text
reuse a FieldId or VariantId for different meaning
delete a field required by existing code
change an existing variant layout incompatibly
remove parameters from an exported function
expand host effects without approval
```

## Effects And Permissions

Capability requirements are part of the compatibility boundary. A reload that
turns a pure function into one that needs host write, random, time, file system,
or event permissions must be approved by the host policy.

## Reflection Stability

Reflection sees a versioned registry snapshot. A reload may create a new
registry, but it cannot mutate the old registry in place. This keeps active
frames, debugger views, and admin tooling consistent with the version they are
inspecting.
