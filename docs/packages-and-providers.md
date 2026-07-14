# Packages And Service Providers

Vela package loading is static, manifest-driven, and host-controlled. Scripts
cannot scan directories, load source at runtime, or mutate package/type
structure.

## Manifest

Every compiled package has a stable ID and source roots:

```toml
[package]
id = "com.example.inventory-plugin"
name = "inventory_plugin"
version = "0.1.0"

[source]
roots = ["src"]

[dependencies]
inventory_api = { path = "api" }

[capabilities]
requires = ["host_read", "host_write"]
```

Dependency keys are direct import aliases. Paths and source roots must remain
inside host-authorized roots. A root manifest may also declare explicit
`[workspace].members`. `[host].schema` is root configuration and is rejected in
dependency manifests. Manifest tables and package fields are strictly typed and
required where documented. Duplicate IDs, cycles, unknown keys/capabilities,
unauthorized paths, and overlapping source roots are rejected before source
assembly; each source file has one package/module owner.

## Imports And Identity

Script module identity is always `PackageId + ModulePath`:

```vela
use crate::helpers::normalize
use inventory_api::api::InventoryProvider
```

`crate::` stays in the current package. A dependency alias crosses into one
direct dependency. Transitive dependencies are compiled but are not importable
without a direct alias. Cross-package declarations must be public. `SourceId`
remains internal to one generation; stable script IDs include `PackageId`.

## Compilation

Ordinary roots do not require provider discovery:

```rust
let snapshot = engine.load_package_workspace("app/vela.toml")?;
let app = PackageId::new("com.example.app")?;
let artifact = engine.compile_package(&snapshot, &app)?;
let mut runtime = Runtime::from_linked_artifact(engine, artifact);
```

The root and transitive path dependencies enter one package-aware HIR,
compiler, linker, and `LinkedArtifact` pipeline.

## Providers

Providers are public zero-field records exported by resolved trait impls:

```vela
pub struct SortInventory {}

#[provider(id = "sort_inventory")]
impl InventoryProvider for SortInventory {
    pub fn run(self, value: i64) -> i64 { return value + 1; }
}
```

The service is inferred from the trait. Identity is `PackageId +
ServiceTraitId + ProviderId`. Discovery reads a sealed snapshot without
executing script, native, reflection, or HostAccess code:

```rust
let catalog = engine.discover_providers(&snapshot)?;
let key = catalog.providers()[0].key().clone();
let selection = catalog.select([key.clone()])?;
let request = ProviderCompileRequest::for_selection(&snapshot, selection);
let artifact = engine.compile_provider_selection(&snapshot, &request)?;
```

Only selected providers enter runtime ABI. The linker seals `ProviderKey ->
TypeHandle -> MethodId -> MethodDispatchHandle` in the same artifact generation
as bytecode and verified MIR. Each call creates a fresh zero-field receiver and
uses normal budgets, GC roots, HostAccess, capabilities, tracing, and profiling:

```rust
let handle = runtime.provider_handle(&key)?;
let value = runtime.call(
    handle.method(service_method_id),
    CallArgs::new(),
    CallOptions::unbounded(),
)?;
```

Handles are bound to one Runtime and contain stable keys, never linked handles.
Provider methods use the same sealed call target and `Runtime::call`/
`Runtime::call_async` surface as functions and bound methods. Outer calls and
same-session reentry share one pure resolver over the pinned linked artifact;
only receiver allocation and root admission differ afterward.

## Capabilities And Reload

Capabilities obey:

```text
observed package effects ⊆ manifest requirements ⊆ Engine grants
```

Requirements are never silently intersected away. Dynamic calls remain
runtime-gated. Capability expansion during reload requires explicit restaging.

Reload rebuilds manifests/sources and reapplies the previous root/provider
fingerprint:

```rust
let update = engine.compile_package_workspace_hot_reload_update_from_previous(
    &current_version,
    "app/vela.toml",
)?;
```

Body changes are accepted. Selected provider removal, target/service/method ABI
changes, root/selection changes, and unapproved capability expansion are
rejected without image advance. Unselected additions do not change ABI. Old
frames and provider-created closures retain their artifact; new calls through
existing logical handles resolve against the accepted image.

See `package_dependency_demo` and `plugin_provider_demo` under `examples`.
