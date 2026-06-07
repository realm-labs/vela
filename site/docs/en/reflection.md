# Reflection

Reflection exposes metadata and controlled value operations to scripts.

## Metadata

Scripts can inspect:

- Types, fields, variants, traits, and methods.
- Modules and functions.
- Required permissions and declared effects.
- Candidate hints for unknown names.

## Controlled Mutation

Reflection can perform controlled reads, writes, and calls when policy allows it. It cannot mutate type structure at runtime.

This means reflection is useful for diagnostics, tools, debug views, and dynamic business workflows, but it is not a monkey-patching system.

## Permissions

Reflection has separate read, write, and call permissions. Host field access still routes through `HostAccess`, so reflection does not bypass the host boundary.
