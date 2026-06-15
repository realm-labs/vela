# Package And Service Provider System Implementation Plan

> **Track:** package/module/SPI architecture continuation, M14/M15/M20.5
> adjacent
> **Document status:** Codex execution plan
> **Compatibility policy:** breaking pre-release package, module graph,
> engine, tooling, and hot-reload APIs are allowed. Do not preserve
> single-directory-only compilation APIs or old import assumptions only for
> compatibility. Preserve product contracts: no script-language generics beyond
> restricted builtin type hints, no Rust `&mut` exposure, host mutation only
> through `HostRef`/`HostPath`/`PathProxy`/`HostAccess`, no runtime `eval`,
> no monkey patching, source-spanned diagnostics, execution budgets, GC roots,
> reflection permissioning, and hot-reload ABI/schema checks.

---

## 0. Codex Goal

```text
/goal Implement Vela's package and trait-backed service provider system from
docs/package-service-provider-system-plan.md. Treat docs/goal.md as the product
roadmap, docs/architecture.md and docs/architecture/*.md as the architecture
contract, and docs/progress.md as the current milestone state. Build a package
system that lets hosts discover user-written Vela plugin logic without runtime
source execution: package manifests define source roots, path dependencies, and
capability requirements; package-aware module graphs resolve `crate::` and
dependency-alias imports; trait-backed providers are exported through
`#[provider(id = "...")] impl ServiceTrait for ProviderType`; discovery builds
a provider catalog without executing scripts; selected package graphs compile
into normal ProgramVersion values; hot reload remains atomic, safe-pointed, and
ABI/capability checked. Defer foreign host-language modules, remote registries,
version solving, runtime `require`, and dynamic script-side package loading.
Validate each checkpoint with focused parser/HIR/engine/hot-reload/tooling
tests and commit small Conventional Commit checkpoints.
```

---

## 1. Purpose

Vela currently supports directory compilation and static module imports. That is
enough for application scripts, but it does not give hosts a clean way to
discover user-defined plugin logic such as Neovim-style commands, filters,
providers, or hooks.

The target design is a package and service provider layer above the existing
module graph:

```text
package roots
  -> package manifests
  -> dependency graph
  -> package-aware module graph
  -> provider catalog
  -> selected package graph compilation
  -> ProgramVersion install and hot reload
```

This keeps discovery host-controlled and static enough for tooling, diagnostics,
capability review, and hot reload. It intentionally avoids Lua-style runtime
`require`, arbitrary source execution, and directory-wide compilation as the
only discovery mechanism.

---

## 2. Goals

- Add a first-class Vela package model with package identity, source roots, path
  dependencies, and capability declarations.
- Support third-party Vela packages that live outside the application source
  directory.
- Make module identity package-aware: a module is identified by
  `PackageId + ModulePath`, not just by a path relative to one root.
- Add package import roots:
  `crate::` for the current package and dependency aliases such as
  `nvim_api::CommandProvider` for dependencies.
- Keep `SourceId` internal. Users and embedders should not choose source IDs.
- Add trait-backed service providers as the SPI mechanism.
- Use `#[provider(id = "...")]` on trait implementations as the explicit export
  boundary. The service is inferred from `impl ServiceTrait for ProviderType`;
  do not repeat `service = ...` in the attribute.
- Build a provider catalog from manifests and parsed/HIR metadata without
  executing scripts.
- Let hosts query providers before compiling or installing a runtime program.
- Compile selected packages and their dependencies into a normal linked
  `ProgramVersion`.
- Keep hot reload package-graph scoped, atomic, safe-pointed, and checked
  against service/provider ABI, schema, effect, and capability compatibility.
- Share package/project source assembly between engine and language-service
  tooling rather than creating separate module models.

---

## 3. Non-Goals

This pass must not:

- Add foreign host-language modules.
- Add remote package registries, version solving, lockfiles, publishing, or
  package signing in the first slice.
- Add script-language generics or generic provider traits beyond existing
  restricted builtin type-hint syntax.
- Add runtime `require`, `eval`, `load_file`, or script-side package discovery.
- Let scripts scan plugin directories or decide which source files are loaded.
- Execute top-level Vela code during discovery.
- Add dynamic monkey patching of packages, modules, traits, or providers.
- Weaken HostAccess, execution budgets, reflection permissions, GC roots, or
  hot-reload checks.
- Treat every trait implementation as a provider automatically.
- Derive provider identity from type names or file paths.
- Preserve single-directory-only APIs as the long-term architecture if they
  conflict with package graph semantics.

---

## 4. Architecture Summary

The ownership split should be:

```text
vela_package
  manifest parsing, package ids, dependency graph, package source assembly

vela_hir
  package-aware module graph, import resolution, declaration ownership,
  provider metadata extraction

vela_analysis
  service trait/provider diagnostics, TypeFacts, tooling facts

vela_bytecode
  package-aware function/type/trait stable IDs and provider ABI metadata

vela_engine
  package discovery, provider catalog API, selected package compilation,
  runtime install helpers

vela_hot_reload
  package graph update reports, provider ABI/capability compatibility checks

vela_language_service / vela_lsp_server
  package-aware workspace roots, dependency aliases, provider navigation,
  diagnostics, and reload-risk metadata
```

If adding a dedicated `vela_package` crate creates too much churn for the first
slice, start with an internal `vela_engine::package` module, but keep data
types dependency-light so they can move into a shared crate before LSP and
engine models drift.

---

## 5. Package Model

### 5.1 Manifest Shape

Use `vela.toml` as the package manifest. First-slice syntax:

```toml
[package]
id = "com.example.inventory-tools"
name = "inventory_tools"
version = "0.1.0"

[source]
roots = ["src"]

[dependencies]
nvim_api = { path = "../nvim_api" }
text_utils = { path = "../text_utils" }

[capabilities]
host_read = true
host_write = false
io_read = false
io_write = false
network = false
```

Rules:

- `package.id` is the stable package identity used for ABI, provider identity,
  diagnostics, and future package registries.
- `package.name` is a display name and should not be used as a stable key.
- `package.version` is recorded in metadata, but first-slice path dependencies
  do not perform semantic version solving.
- `source.roots` lists package-relative directories containing `.vela` files.
- dependency table keys are import aliases.
- dependency aliases must be unique within a package.
- dependency paths are resolved relative to the manifest directory.
- capability declarations are package maximum requirements. The host still
  grants an allowed capability profile at runtime.

### 5.2 Package Identity

Add focused identity types:

```rust
pub struct PackageId(/* stable id from package.id */);
pub struct PackageName(String);
pub struct PackageAlias(String);
pub struct PackageVersion(String);

pub struct PackageKey {
    pub id: PackageId,
}
```

Do not use filesystem paths, package display names, or dependency aliases as
stable ABI identity.

### 5.3 Package Graph

The resolver produces:

```rust
pub struct PackageGraph {
    pub packages: BTreeMap<PackageId, PackageDescriptor>,
    pub dependencies: BTreeMap<PackageId, BTreeMap<PackageAlias, PackageId>>,
}
```

First-slice dependency behavior:

- path dependencies only.
- duplicate package IDs in the resolved graph are rejected unless they refer to
  the same canonical manifest path.
- cyclic dependencies are rejected with source-spanned or manifest-spanned
  diagnostics.
- transitive dependencies are visible only through direct dependency aliases
  unless a package re-exports declarations explicitly in source.

Remote registries, lockfiles, version constraints, and multiple versions of the
same package are deferred.

---

## 6. Package-Aware Module Identity

### 6.1 Module Keys

Keep `ModulePath` package-relative:

```text
src/commands/sort.vela -> commands::sort
```

Add a package-aware key:

```rust
pub struct ModuleKey {
    pub package: PackageId,
    pub path: ModulePath,
}
```

The module graph should index modules by `ModuleKey`, not by `ModulePath`
alone. Existing single-package code can use an implicit root package during the
transition, but the final architecture should not assume one directory equals
one global module namespace.

### 6.2 Source IDs

`SourceId` remains internal:

```text
PackageGraph + source ordering -> deterministic SourceId allocation
```

Users and embedders should pass package roots, manifests, package sources, or
source records, not raw `SourceId`. Diagnostics map `SourceId` back to package
and source path through a source table.

### 6.3 Import Roots

Add package import roots:

```vela
use crate::helpers::normalize_name
use nvim_api::CommandProvider
use nvim_api::CommandContext
```

Rules:

- `crate::` resolves within the current package.
- a dependency alias resolves to that dependency package.
- no unqualified cross-package imports.
- no implicit transitive dependency imports.
- native/std roots remain explicit reserved roots, such as `std::` or existing
  native module roots.

The resolver must report ambiguous or unknown package roots as HIR diagnostics,
not VM errors.

---

## 7. Service Trait Model

Service interfaces are ordinary Vela traits that are intended for host
discovery:

```vela
pub trait CommandProvider {
    fn run(self, ctx: CommandContext, args: Array<String>) -> Result<CommandResult, String>
}
```

First-slice rules:

- service traits must be public if they are used across package boundaries.
- service traits participate in stable trait IDs and hot-reload ABI checks.
- service trait method names, parameter counts, type hints, return hints, and
  effect/capability metadata are part of the service ABI.
- service trait implementations use the normal script trait implementation
  machinery; provider dispatch should not invent a separate call system.

The provider system should not make every trait special. A trait becomes a
service only when the host queries it as a service or when a package exports a
provider implementation for it.

---

## 8. Provider Export Model

### 8.1 Attribute Syntax

Use an attribute on trait implementations:

```vela
pub struct SortInventory {}

#[provider(id = "sort_inventory")]
impl CommandProvider for SortInventory {
    pub fn run(self, ctx: CommandContext, args: Array<String>) -> Result<CommandResult, String> {
        // user plugin logic
    }
}
```

The service is inferred from the impl:

```text
impl CommandProvider for SortInventory
     ^ service trait        ^ provider implementation type
```

Do not use:

```vela
#[provider(service = CommandProvider, id = "sort_inventory")]
```

because `service = CommandProvider` duplicates information already present in
the impl and creates an unnecessary source of mismatch.

### 8.2 Provider Identity

Provider identity is:

```text
ProviderKey = PackageId + ServiceTraitId + ProviderId
```

`ProviderId` comes from `#[provider(id = "...")]` and is stable public ABI.

Rules:

- `id` is required.
- `id` must be unique for a service within one package.
- renaming the provider type does not change provider identity.
- changing the `id` is a provider removal plus provider addition for hot reload.
- display names, descriptions, and ordering metadata may be added later, but
  must not replace stable IDs.

### 8.3 Provider Construction

First slice should support stateless providers:

```vela
pub struct SortInventory {}
```

The runtime can construct a zero-field provider value for calls or use a
runtime-owned singleton. This avoids adding provider factories before the SPI
contract is proven.

Future stateful providers may add explicit factories:

```vela
#[provider_factory(id = "sort_inventory")]
pub fn create_sort_inventory(ctx) -> SortInventory {
    return SortInventory { /* config */ }
}
```

Do not add factory syntax in the first slice unless a test case proves it is
needed. Module globals and host context are enough for the initial plugin use
cases.

### 8.4 Provider Descriptor

Discovery should produce descriptors shaped like:

```rust
pub struct ProviderDescriptor {
    pub package: PackageId,
    pub service_trait: TraitId,
    pub provider_id: ProviderId,
    pub provider_type: TypeId,
    pub impl_id: ImplId,
    pub methods: Vec<ProviderMethodDescriptor>,
    pub required_capabilities: CapabilitySet,
    pub span: Span,
}
```

Provider descriptors are metadata. They do not contain live VM values and do
not execute script code.

---

## 9. Discovery Pipeline

Discovery is host-controlled:

```text
host package roots
  -> find vela.toml files
  -> parse manifests
  -> resolve path dependencies
  -> collect package source records
  -> parse/HIR declarations
  -> collect #[provider] impl metadata
  -> validate service impl contracts
  -> return ProviderCatalog
```

Discovery may parse and lower enough HIR to validate declarations and trait
implementations. It must not run bytecode, execute top-level expressions, call
native functions, read host state, or perform HostAccess operations.

The API should make this explicit:

```rust
let catalog = engine.discover_packages(["plugins", "vendor"])?;
let providers = catalog.providers_for(command_provider_trait_id);
```

`ProviderCatalog` is safe to show in UIs, config screens, CLI listings, and
LSP tooling.

---

## 10. Compilation Pipeline

Compilation should accept a resolved package graph or selected providers:

```rust
let selection = ProviderSelection::new()
    .enable(command_provider_trait_id, ProviderId::new("sort_inventory"));

let program = engine.compile_provider_selection(&catalog, selection)?;
runtime.install(program)?;
```

Compilation rules:

- selected providers pull in their owning packages.
- selected packages pull in direct and transitive dependencies.
- dependencies are compiled as normal package modules.
- all selected code links into one `ProgramVersion` for the runtime image.
- provider catalog metadata is embedded into the linked program image.
- stable IDs use package-aware definition paths.

`compile_dir(root)` may remain as a convenience for single-package applications,
but it should become sugar over package source assembly, not a separate module
model.

---

## 11. Runtime Provider Calls

The host should call providers by stable identity, not by raw function name:

```rust
let provider = runtime.provider(command_provider_trait_id, ProviderId::new("sort_inventory"))?;
let result = provider.call("run", args)?;
```

Or with a direct helper:

```rust
runtime.call_provider(
    command_provider_trait_id,
    ProviderId::new("sort_inventory"),
    "run",
    args,
)?;
```

Runtime behavior:

- provider lookup resolves through the active `ProgramVersion`.
- provider method calls route to the concrete script trait impl method.
- execution budgets, call-depth budgets, GC roots, HostAccess, and capability
  checks are unchanged.
- missing provider, missing method, or disabled provider errors are
  source/runtime diagnostics with provider identity context.
- high-frequency callers may cache a provider handle, but the handle must be
  invalidated or version-checked on hot reload just like cached function
  handles.

---

## 12. Hot Reload Model

Package/provider hot reload remains ProgramVersion based:

```text
changed package file or manifest
  -> rediscover affected package graph
  -> recompile affected package graph
  -> validate provider ABI and capabilities
  -> stage ProgramVersion
  -> install at safe point
```

Compatibility checks must include:

- package ID stability.
- dependency alias and package ID changes that affect imports.
- service trait method additions/removals/signature changes.
- provider ID removals and additions.
- provider target type changes.
- provider method signature and effect changes.
- package capability expansion.
- script schema changes for public provider argument/return types.

Rejected updates must not advance the active runtime image. Reports should name
the package, module, provider, service trait, and source spans involved.

---

## 13. Language Service And Tooling

The language service should use the same package model:

- parse `vela.toml` package manifests.
- index package roots and dependency package roots.
- resolve `crate::` and dependency aliases.
- provide completions for dependency aliases and imported service traits.
- show provider descriptors in document/workspace symbols.
- support go-to-definition from provider impl to service trait.
- support references from service trait to provider implementations.
- report missing dependency, duplicate package ID, duplicate provider ID, and
  service impl mismatch diagnostics.
- report hot-reload ABI risk when renaming provider IDs, service traits, or
  public provider method signatures.

Editor tooling must not execute Vela programs or run the Rust host application
to discover providers.

---

## 14. Security And Capability Rules

Package manifests declare requested capabilities. Hosts grant capabilities.
The effective runtime capability set is the intersection of requested and
granted capabilities.

Rules:

- discovery may read manifests and source files from host-configured package
  roots only.
- scripts cannot add package roots at runtime.
- scripts cannot load packages at runtime.
- capability expansion during hot reload is rejected unless the host explicitly
  approves and restages the update.
- provider catalog queries are metadata reads, not reflection permission
  bypasses.
- provider calls follow normal call and HostAccess permissions.

---

## Phase Status

Use this checklist as the durable execution tracker. Mark a task only after its
focused tests and validation command pass.

```text
[ ] not started
[~] in progress
[x] complete
```

---

## 15. Phase 1: Manifest And Package Graph

Purpose: introduce the package model without changing script semantics yet.

- [ ] Add package manifest data types.
- [ ] Parse `vela.toml` with `[package]`, `[source]`, `[dependencies]`, and
  `[capabilities]`.
- [ ] Resolve path dependencies relative to the manifest directory.
- [ ] Detect duplicate package IDs and dependency cycles.
- [ ] Assemble package source records from all source roots.
- [ ] Allocate deterministic internal `SourceId` values for package sources.
- [ ] Keep `SourceId` out of user-facing engine APIs.

Tests:

- [ ] `manifest_parses_package_source_dependencies_and_capabilities`
- [ ] `path_dependency_resolves_relative_to_manifest`
- [ ] `duplicate_package_id_is_rejected`
- [ ] `dependency_cycle_is_rejected`
- [ ] `package_sources_get_deterministic_source_ids`

Validation:

```bash
cargo test -p vela_engine package
cargo test -p vela_language_service project
```

---

## 16. Phase 2: Package-Aware Module Graph

Purpose: make imports and declarations package-aware.

- [ ] Add `ModuleKey { package, path }`.
- [ ] Extend `ModuleSource` or add package-aware source input records.
- [ ] Index modules by `ModuleKey`.
- [ ] Resolve `crate::` imports to the current package.
- [ ] Resolve dependency alias imports to direct dependency packages.
- [ ] Reject unknown dependency aliases and implicit transitive imports.
- [ ] Preserve existing single-package compile behavior as sugar over an
  implicit package graph.
- [ ] Update stable definition paths to use package IDs consistently.

Tests:

- [ ] `crate_import_resolves_within_current_package`
- [ ] `dependency_alias_import_resolves_to_dependency_package`
- [ ] `transitive_dependency_import_requires_direct_alias`
- [ ] `same_module_path_in_two_packages_does_not_collide`
- [ ] `single_package_compile_uses_implicit_package`

Validation:

```bash
cargo test -p vela_hir module_graph
cargo test -p vela_bytecode
```

---

## 17. Phase 3: Provider Metadata In HIR

Purpose: collect trait-backed provider declarations without runtime execution.

- [ ] Parse and preserve `#[provider(id = "...")]` attributes on impl items.
- [ ] Reject `#[provider]` on non-trait impls.
- [ ] Infer service trait from `impl ServiceTrait for ProviderType`.
- [ ] Require a stable string `id`.
- [ ] Reject duplicate provider IDs for the same package and service trait.
- [ ] Validate provider method coverage against the service trait.
- [ ] Validate provider method signatures, type hints, return hints, and
  effects against the trait contract.
- [ ] Emit source-spanned diagnostics for malformed provider attributes and
  service mismatches.

Tests:

- [ ] `provider_impl_exports_trait_service`
- [ ] `provider_service_is_inferred_from_impl_trait`
- [ ] `provider_rejects_redundant_or_unknown_attribute_keys`
- [ ] `provider_rejects_missing_id`
- [ ] `provider_rejects_duplicate_id_for_same_service`
- [ ] `provider_rejects_method_signature_mismatch`

Validation:

```bash
cargo test -p vela_syntax provider
cargo test -p vela_hir provider
```

---

## 18. Phase 4: Provider Catalog Discovery API

Purpose: let hosts list available script plugin logic before compilation.

- [ ] Add `ProviderDescriptor`, `ServiceDescriptor`, and `ProviderCatalog`.
- [ ] Add engine discovery API for package roots.
- [ ] Build provider descriptors from package graph and HIR metadata.
- [ ] Include package, service trait, provider ID, provider type, method,
  capability, and source-span metadata.
- [ ] Return diagnostics without executing scripts.
- [ ] Add catalog query helpers by service trait and provider ID.

Tests:

- [ ] `discover_packages_returns_provider_catalog`
- [ ] `discovery_does_not_execute_top_level_code`
- [ ] `catalog_filters_providers_by_service_trait`
- [ ] `catalog_reports_provider_source_spans`
- [ ] `catalog_includes_required_capabilities`

Validation:

```bash
cargo test -p vela_engine provider_catalog
```

---

## 19. Phase 5: Compile Selected Package Graph

Purpose: compile provider selections into normal runtime programs.

- [ ] Add provider selection data types.
- [ ] Resolve selected providers to owning packages.
- [ ] Include transitive dependencies in the compile set.
- [ ] Embed provider metadata into linked program images.
- [ ] Add runtime provider lookup by service trait and provider ID.
- [ ] Add provider method call helper that routes to concrete trait impl
  methods.
- [ ] Keep cached provider handles version-checked across reload.

Tests:

- [ ] `compile_provider_selection_includes_dependencies`
- [ ] `runtime_calls_provider_method`
- [ ] `runtime_rejects_missing_provider`
- [ ] `provider_call_uses_normal_budget_and_host_access_checks`
- [ ] `cached_provider_handle_rejects_or_refreshes_after_reload`

Validation:

```bash
cargo test -p vela_engine provider
cargo test -p vela_vm provider
```

---

## 20. Phase 6: Package And Provider Hot Reload

Purpose: make provider updates safe and explainable.

- [ ] Stage package graph updates from changed source files and manifest files.
- [ ] Compute changed packages and affected dependent packages.
- [ ] Check provider ABI compatibility.
- [ ] Check service trait ABI compatibility.
- [ ] Check package capability expansion.
- [ ] Include provider and package details in hot-reload reports.
- [ ] Reject updates without advancing active `ProgramVersion`.

Tests:

- [ ] `provider_body_change_is_accepted`
- [ ] `provider_id_removal_is_rejected_when_active`
- [ ] `service_trait_method_removal_is_rejected`
- [ ] `provider_signature_change_is_rejected`
- [ ] `capability_expansion_is_rejected_without_host_approval`
- [ ] `dependency_package_change_invalidates_dependents`

Validation:

```bash
cargo test -p vela_hot_reload provider
cargo test -p vela_engine reload
```

---

## 21. Phase 7: Tooling, Examples, And Docs

Purpose: make the package/SPI model usable.

- [ ] Update language-service project loading to use package manifests and
  dependency aliases.
- [ ] Add completions for `crate::` and dependency aliases.
- [ ] Add hover/definition/references support for service traits and provider
  impls.
- [ ] Add diagnostics for missing packages, duplicate package IDs, duplicate
  provider IDs, and service mismatches.
- [ ] Add a standalone example with an API package and a plugin package.
- [ ] Document package manifests, imports, service traits, provider IDs,
  discovery, compilation, and hot reload.

Tests:

- [ ] `lsp_completion_lists_dependency_aliases`
- [ ] `definition_follows_provider_to_service_trait`
- [ ] `references_find_service_provider_impls`
- [ ] `rename_provider_id_reports_hot_reload_risk`
- [ ] `example_plugin_provider_runs`

Validation:

```bash
cargo test -p vela_language_service package provider
cargo test -p vela_lsp_server package provider
cargo run -p vela_examples --bin plugin_provider_demo
```

---

## 22. Open Design Questions

Keep these out of the first slice unless they block implementation:

- Whether provider factories are needed before stateless providers are proven.
- Whether multiple versions of the same package may coexist in one runtime
  image.
- Whether package manifests need lockfiles before public release.
- Whether package registry metadata should share the future deployment package
  artifact format.
- Whether public service traits should require an explicit service attribute or
  whether host queries are enough.
- Whether providers should support host-approved per-provider capability
  overrides in addition to package-level capabilities.

---

## 23. First Vertical Slice

The smallest useful slice is:

```text
one API package
one plugin package with path dependency
one service trait
one stateless provider impl with #[provider(id = "...")]
host discovery lists the provider
host compiles selected provider package graph
runtime calls provider.run(...)
provider body hot reload is accepted
provider signature change is rejected
```

This proves the model without adding registries, factories, remote packages, or
foreign-language module artifacts.
