# Script Reflection

Reflection queries script/stdlib/native metadata and performs permitted
operations on script-owned values. It supports diagnostics, hot-reload ABI
comparison, editors and planned debugger integration. It is not a Rust object
bridge. No opaque-object runtime or opaque reflection schema remains after D.

## TypeRegistry

The registry describes types, fields, variants, methods, traits, modules and
functions using stable semantic identities, normalized hints, signatures,
attributes, access/effect metadata, docs and optional declaration origins.
Hot reload publishes a new immutable registry rather than mutating an old
schema. Field order/display names are not substitutes for stable identity.

Runtime type structure is read-only. Adding/removing fields, replacing methods,
monkey patching and eval-generated code are not supported. Copied descriptor
records are ordinary script data, not capabilities to edit registry structure.

## Type Hints And Analysis Facts

Primitive tags remain exact; annotations are contracts, not implicit numeric
conversions. Builtin `Array<T>`, `Set<T>`, `Map<K, V>`, `Iterator<T>`, `Option<T>`
and `Result<T, E>` are restricted hints, not script-language generics. Key/element
contracts must satisfy `ValueKey` policy.

Internal analysis facts can represent unknown/Any, local unions, narrowed enum
variants, callable parameter/result facts and container element facts. They
support completion, hover and diagnostics without imposing static typing.
Unknown dynamic results degrade conservatively rather than inventing members.
See [primitive contracts](primitives-type-hints-and-guards.md) and
[LSP architecture](lsp.md).

## Script Reflection API

The retained families include:

- Type lookup: `reflect::type_of`, `types`, `type_info`, `has_type`, `name`, `kind`.
  `type_of` returns an Option of a copied descriptor when metadata exists.
- Descriptor inspection: `owner`, `origin`, `access`, `fields`, `field`, `methods`,
  `method`, `params`, `returns`, attributes and trait metadata.
- Values: `has_field`, `get`, `set`, `has_method`, `call` on permitted script values.
- Enums/traits: `variants`, `variant`, `variant_info`, `variant_is`, `has_variant`,
  `implements`, `traits`, `trait_info`, `has_trait`.
- Source organization: `modules`, `module`, `has_module`, `functions`, `function`,
  `has_function`, `exports`.
- Policy: `permissions`, `has_permission`.

For script records and enum payloads, `reflect::set` returns an updated copy.
It does not update the caller's local binding unless assigned, and unknown
fields are rejected rather than added to the schema. Reflection map values
preserve keys rather than stringifying them; metadata records remain
string-field records. Value reads/calls and returned aggregates use normal
script ownership, roots, guards and budgets.

There are no host-path construction/inspection operations, transparent Rust
field reads/writes, borrowed-child recovery or callable asyncness descriptors.
Dynamic reflection cannot reconstruct a removed registration family.

## Reflection Permissions

Engine reflection is disabled until an explicit policy/permission set enables
it. `EngineBuilder::reflection_lookup_budget` only limits work; it does not
enable reflection, and its explicit limit takes precedence over the policy
limit independently of setter order.

Retain distinct checks for metadata lookup, value field reads/writes, method
calls, private access and host-read/host-write/event effects of native calls.
Removing Rust-object paths does **not** remove host-read/write native effects.
Reflected native calls must honor both reflection policy and ordinary callable
capabilities; neither grants the other implicitly.

Field-required reflection permission names filter descriptors and script
field access when registry metadata is available. They are reflection policy,
not arbitrary callable ACL strings or native binding fingerprints. Rejected
operations fail before their associated effect, with structured diagnostics.

## Tooling And Artifacts

Static registry facts may be exported for editor analysis without executing
scripts/native code or reading application state. Tooling schema is **4**;
Rust-binding schema is **7** and portable program format is **8**. Incompatible
schemas/formats reject before use. Removed Service/Provider, async, borrow and
path metadata must not be synthesized by a compatibility reader.

See [native registration](host-and-registration.md),
[stdlib/embedding](stdlib-and-embedding.md) and [validation](../validation.md).
