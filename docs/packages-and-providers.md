# Ordinary Packages

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
use inventory_api::api::normalize
```

`crate::` stays in the current package. A dependency alias crosses into one
direct dependency. Transitive dependencies are compiled but are not importable
without a direct alias. Cross-package declarations must be public. `SourceId`
remains internal to one generation; stable script IDs include `PackageId`.

## Compilation

Ordinary roots compile directly:

```rust
let snapshot = engine.load_package_workspace("app/vela.toml")?;
let app = PackageId::new("com.example.app")?;
let artifact = engine.compile_package(&snapshot, &app)?;
let mut runtime = Runtime::from_linked_artifact(engine, artifact)?;
```

The root and transitive path dependencies enter one package-aware HIR,
compiler, linker, and `LinkedArtifact` pipeline.

## Providers

Provider discovery, catalogs, selection, provider-specific compilation, handles
and ABI facts were removed in B. The old Provider examples are historical,
not an optional package mode or future work. Ordinary public records, traits,
impls, functions, imports and path dependencies remain supported without them.

## Capabilities And Reload

Capabilities obey:

```text
observed package effects ⊆ manifest requirements ⊆ Engine grants
```

Requirements are never silently intersected away. Dynamic calls remain
runtime-gated. Capability expansion during reload requires explicit restaging.

Reload rebuilds manifests/sources and reapplies the previous ordinary root
fingerprint:

```rust
let update = engine.compile_package_workspace_hot_reload_update_from_previous(
    &current_version,
    "app/vela.toml",
)?;
```

Compatible body changes are accepted. Public ABI, package/root identity and
unapproved capability expansion are checked before image advance. Rejection
leaves the active image unchanged. Old frames and closures retain their exact
artifact, nested dispatch, state and plans; new entry calls use the accepted
image. No structural state migration or host-object migration is implied.

See [package_dependency_demo](../examples/src/bin/package_dependency_demo/main.rs).
The historical filename of this document does not imply Provider support.
