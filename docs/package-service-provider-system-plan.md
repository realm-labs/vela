# Package And Service Provider System Hard-Switch Plan

> **Track:** package/module/SPI architecture continuation, adjacent to
> M14/M15/M20.5
>
> **Document status:** reviewed execution plan, updated for the linked-artifact,
> source-boundary, executable-generation, and LSP ownership hard switches
>
> **Compatibility policy:** breaking pre-release package, module, Engine,
> tooling, and hot-reload API changes are allowed. Do not preserve the global
> `script` identity namespace, the handwritten `vela.toml` parser,
> single-directory module identity, or parallel package-unaware compilation
> paths for compatibility.

---

## 0. Codex Goal

```text
/goal Execute docs/package-service-provider-system-plan.md end to end against
the current linked-artifact architecture. Treat docs/goal.md as the product
roadmap, docs/architecture.md and docs/architecture/*.md as the architecture
contract, docs/decisions.md as durable design decisions, and docs/progress.md
as the rolling milestone state.

Begin with the read-only Phase 0 inventory and baseline. Create the dedicated
dependency-light vela_package crate immediately; do not stage package types in
vela_engine. Hard switch every vela.toml consumer to one structured manifest
parser and one package/project source assembly model. Once the package-identity
cutover begins, keep Phase 2 atomic across vela_package, vela_hir, vela_def,
vela_analysis, vela_bytecode, vela_reflect, vela_hot_reload,
vela_language_service, vela_lsp_server, vela_engine, examples, and tests. Do
not add implicit-package adapters, dual ModulePath/ModuleKey indexes, fallback
imports, or old script-ID aliases to keep intermediate revisions compatible.

Make PackageId + ModulePath the sole script module identity. Feed PackageId
into every stable script definition path and eliminate the hard-coded `script`
package namespace. Keep SourceId internal and generation-local. Preserve
single-source and directory convenience APIs only as front doors that build an
explicit reserved package graph before HIR ingestion. Make ordinary package
compilation the foundation: a root package compiles with all transitive path
dependencies and may import their public declarations without using providers.

Expose one sealed PackageCompilationSnapshot, one base PackageCompileRequest,
and a ProviderCompileRequest extension. Both enter the same HIR -> compiler ->
linker pipeline, not parallel compilers. Persist a stable request fingerprint
so ordinary package and provider hot reload rebuild the same roots and
selection against a new snapshot.

Implement providers as explicit trait impl exports using
#[provider(id = "...")]. Preserve attribute arguments structurally in HIR;
infer the service from the resolved trait impl; validate one zero-field
provider construction rule; build discovery metadata without executing code;
and compile selected package graphs through the existing
HirSourceSet -> CompiledProgram -> Engine link -> Arc<LinkedArtifact> pipeline.
Attach same-generation package/provider metadata to LinkedArtifact. Create
ProgramVersion and HotUpdate only through the existing hot-reload artifact
boundary. Runtime provider lookup and calls belong to vela_engine Runtime and
must resolve stable ProviderKey/MethodId pairs to linked handles rather than
dispatch by source names. Seal only the host-selected providers into an
InstalledProviderSet; discovered but unselected providers are not runtime ABI.
Compatible reloads reapply the existing selection and logical ProviderHandle
values resolve against the new active image automatically.

Treat manifest capabilities as declared requirements, not optional flags.
Use one shared Capability/CapabilitySet definition across manifests, analysis,
Engine grants, and hot reload. Require statically observed package effects to
be a subset of declared requirements and declared requirements to be a subset
of host grants; never silently intersect away a missing grant. Keep dynamic
calls runtime-gated instead of claiming complete call-graph inference in the
first slice. Preserve execution budgets, GC roots, HostAccess, reflection
permissions, safe-point installation, retained old frames/closures, and
existing ABI/schema checks.

Close each phase only after its focused tests pass. Phase 1 may be committed as
a package-foundation checkpoint before the identity cutover. Phase 2 must land
as one coherent breaking hard-switch commit. Later ordinary package, provider,
reload, and tooling phases may use small verified Conventional Commit
checkpoints, but no checkpoint may restore a compatibility path. Finish with
full workspace, examples, manifest zero-hit, identity zero-hit, dependency,
file-size, and documentation validation. Do not mark the plan complete while
any handwritten vela.toml parser, global script package identity, package-
unaware ModuleGraph index, provider name dispatch, independently assembled
artifact metadata, or unchecked task remains.
```

---

## 1. Purpose

Vela supports static multi-module source sets, directory compilation, stable
script metadata, linked executable generations, safe-pointed hot reload, and a
native language service. It does not yet have a shared package identity model,
Cargo-like path dependencies for ordinary applications/libraries, or a
host-controlled way to discover user-written plugin implementations.

The target pipeline is:

```text
host-configured manifest roots
  -> one structured vela.toml model
  -> PackageGraph and deterministic package sources
  -> package-aware HirSourceSet and ModuleGraph
  -> sealed PackageCompilationSnapshot
  -> ordinary root packages + optional ProviderSelection
  -> one PackageCompileRequest
  -> Engine registry-aware Arc<LinkedArtifact>
  -> optional ProgramVersion / HotUpdate construction
  -> ordinary Runtime entry calls and optional provider calls
```

Discovery parses manifests and source metadata but never executes Vela code.
Runtime loading remains static and host-controlled. There is no Lua-style
`require`, runtime `eval`, script-side directory scanning, or monkey patching.

## 2. Current Baseline

The implementation starts from these current contracts:

- `vela_hir::source_ingestion` owns parsing and `HirSourceSet` construction.
- `vela_engine` is the only production source orchestrator and owns
  registry-aware linking.
- `vela_bytecode` consumes sealed HIR source sets and produces
  `CompiledProgram`; the linker produces the canonical `Arc<LinkedArtifact>`.
- `ProgramVersion` owns one same-generation linked artifact and is constructed
  only by `vela_hot_reload` from an artifact.
- `RuntimeImage` owns immutable executable data; `RuntimeState` owns mutable
  globals, heap roots, caches, and profiles.
- `GlobalState -> ProjectState` is the only mutable LSP project owner.
- source traits and impl metadata already exist in HIR.
- the syntax CST already supports structured attribute arguments, but
  `HirAttribute` currently flattens arguments into one string.
- `DefPath` already contains a package string, while script identity helpers
  currently hard-code the package name `script`.
- the current language-service `vela.toml` parser recognizes only
  `[workspace].roots` and `[host].schema` using handwritten line parsing.

These are migration inputs, not alternate architectures to preserve.

## 3. Goals

- Add one dependency-light `vela_package` crate.
- Use one shared capability vocabulary and bitset across package declarations,
  compiler checks, Engine grants, and hot-reload ABI.
- Define stable package identity, package manifests, path dependencies,
  package source roots, workspace members, and requested capabilities.
- Make `PackageId + ModulePath` the only script module identity.
- Resolve `crate::` within the current package and dependency aliases through
  direct package dependencies.
- Compile an ordinary root package with all transitive path dependencies and
  allow imports of public dependency declarations without provider metadata.
- Provide ordinary package compile, Runtime, and hot-reload APIs independently
  of provider discovery or selection.
- Keep `SourceId`, `ModuleId`, and `HirDeclId` internal and generation-local.
- Export providers only through `#[provider(id = "...")]` on trait impls.
- Infer the service from the resolved trait implementation.
- Discover provider descriptors from package/HIR metadata without execution.
- Compile selected package graphs into the existing linked artifact model.
- Call providers through linked trait-implementation handles under normal VM,
  budget, capability, HostAccess, and GC rules.
- Include provider and package ABI metadata in the same executable generation.
- Preserve atomic safe-point hot reload and explain rejected package/provider
  changes.
- Share manifest, package graph, source assembly, and source maps between
  Engine and language-service tooling.

## 4. Non-Goals

This plan must not add:

- remote registries, version solving, lockfiles, publishing, or signing;
- multiple versions of one `PackageId` in one package graph;
- foreign host-language modules;
- general script-language generics or provider trait generics;
- runtime `require`, `eval`, `load_file`, or script-side package discovery;
- top-level code execution during discovery;
- provider factories or stateful provider construction in the first slice;
- per-provider capability overrides;
- dynamic package, trait, or provider monkey patching;
- a new VM provider-dispatch system parallel to normal trait impl dispatch;
- a second package/project model in the LSP;
- JIT, DAP, async/coroutine reload, or moving GC work.

The first slice also does not add a `network` capability. Unknown capability
names are diagnostics. A capability may be added only with an implemented and
tested runtime boundary.

## 5. Ownership And Dependency Direction

The final ownership split is mandatory:

```text
vela_package
  PackageId, PackageName, PackageVersion, PackageAlias
  ModulePath, ModuleKey, PackageSource, SourceTable
  structured vela.toml parsing and manifest diagnostics
  path dependency resolution and PackageGraph
  deterministic source discovery and workspace-member assembly

vela_hir
  package-aware HirSourceSet and ModuleGraph
  crate/dependency-alias import resolution
  structured HIR attributes
  provider/service declaration metadata

vela_analysis
  provider trait/type/signature/effect validation facts
  statically observed package capability requirements

vela_def
  package-aware DefPath helpers and stable script IDs

vela_bytecode
  provider compile metadata and same-generation linked provider records

vela_engine
  package workspace loading, ordinary roots, provider selection,
  one package compile request, compilation/linking
  Runtime provider lookup/call, host grants, installation helpers

vela_hot_reload
  artifact-derived package/provider ABI comparison and update reports

vela_language_service / vela_lsp_server
  shared package project loading, aliases, diagnostics, navigation, rename risk
```

Dependency rules:

- `vela_common` owns the domain-neutral `Capability` vocabulary and
  `CapabilitySet`; package, analysis, Engine, and hot reload use that one type.
- `vela_package` depends only on general utilities such as `vela_common`,
  serialization/TOML libraries, and the standard library.
- `vela_package` must not depend on Engine, HIR, bytecode, VM, hot reload, or
  LSP crates.
- `vela_hir` consumes package source records; it remains the owner of Vela
  parsing and semantic source-set construction.
- `vela_engine` orchestrates package loading, HIR, compilation, and linking; it
  does not become the owner of shared package data types.
- `vela_language_service` consumes `vela_package` directly and must not depend
  on `vela_engine`.
- `vela_hot_reload` accepts linked artifacts and immutable artifact metadata;
  it must not read manifests or compile package sources.

## 6. Unified Manifest Model

### 6.1 One `vela.toml` Schema

Use one structured parser and one manifest model for Engine and tooling. A
root manifest may describe a package, a workspace, or both:

```toml
[workspace]
members = ["plugins/sort_inventory", "packages/nvim_api"]

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
requires = ["host_read"]

[host]
schema = "target/vela/schema.json"
```

Rules:

- `[workspace].members` contains explicit paths in the first slice; workspace
  glob patterns are deferred.
- `[package]` is required for every compiled dependency and provider package.
- `[source].roots` is package-relative and must not escape the package root.
- dependency keys are direct import aliases; each path must resolve to a
  manifest containing `[package]`.
- `[host]` is root-workspace configuration and is ignored/rejected in
  dependency manifests according to a documented diagnostic rule.
- the old `[workspace].roots` form is deleted in the hard switch. CLI/editor
  launch roots remain host inputs; source roots come from package manifests.
- unknown tables, keys, capability names, duplicate keys, invalid paths, and
  malformed values produce manifest-file spans.
- use a real TOML parser with byte-range support. Do not extend the existing
  line/split parser.

### 6.2 Manifest Diagnostics

Manifest diagnostics are not Vela source spans. `vela_package` owns a
dependency-light file/span model such as:

```rust
pub struct ManifestFileId(/* internal deterministic ID */);

pub struct ManifestSpan {
    pub file: ManifestFileId,
    pub start: u32,
    pub end: u32,
}
```

The package source table maps manifest/source IDs to canonical paths for
Engine errors and language-service diagnostics. Do not allocate fake Vela
`SourceId` values for TOML files.

## 7. Package Identity And Graph

### 7.1 Identity Types

`PackageId` is the validated canonical manifest string and the stable ABI key;
it is not derived from a path, alias, display name, or source ordering.

```rust
pub struct PackageId(Arc<str>);
pub struct PackageName(Arc<str>);
pub struct PackageAlias(Arc<str>);
pub struct PackageVersion(Arc<str>);

pub struct ModuleKey {
    pub package: PackageId,
    pub path: ModulePath,
}
```

`PackageName` and `PackageVersion` are metadata. `PackageAlias` is local to the
depending package. `PackageKey { id: PackageId }` is unnecessary.

`DefPath.package` must receive the canonical `PackageId` string. Package-aware
helpers replace the global `script_*` identity helpers for functions, globals,
types, fields, variants, traits, inherent methods, trait methods, impl method
functions, lambdas where applicable, and provider keys.

### 7.2 Package Graph

```rust
pub struct PackageGraph {
    packages: BTreeMap<PackageId, PackageDescriptor>,
    dependencies: BTreeMap<PackageId, BTreeMap<PackageAlias, PackageId>>,
    sources: SourceTable,
}
```

Rules:

- path dependencies only;
- canonicalize manifest paths before duplicate-ID comparison;
- the same `PackageId` at different canonical manifests is an error;
- dependency cycles are rejected with the full manifest edge chain;
- transitive dependencies are not importable without a direct alias;
- source file ordering and internal `SourceId` allocation are deterministic;
- source roots and dependencies cannot escape host-authorized roots after
  canonicalization;
- package graph/source assembly performs filesystem work, not Vela parsing.

### 7.3 Convenience Front Doors

`compile_source`, `compile_file`, and `compile_dir` may remain, but they must
construct an explicit reserved package graph and then enter the same package-
aware HIR pipeline. The reserved anonymous package ID must be documented and
stable for the lifetime of one embedding API contract.

Language-service scratch documents use their own explicit reserved scratch
package identity. They do not enter a package-less ModuleGraph.

There must be no package-unaware `ModuleGraph`, stable-ID helper, compiler
request, or language-service project path behind these conveniences.

## 8. Package-Aware HIR

`ModulePath` remains package-relative. `ModuleGraph` indexes modules by
`ModuleKey`, while `ModuleId` remains a dense generation-local handle.

Imports:

```vela
use crate::helpers::normalize_name
use nvim_api::CommandProvider
use nvim_api::CommandContext
```

Resolution rules:

- `crate::` resolves from the importing module's `PackageId`;
- the first segment matching a direct dependency alias enters that package;
- native and std roots remain explicit reserved roots;
- an unqualified path never crosses a package boundary;
- transitive aliases are invisible unless directly declared;
- unknown aliases, private cross-package declarations, and ambiguous reserved
  roots are HIR diagnostics with source and manifest labels.

The Phase 2 identity cutover must update HIR declarations, bindings,
qualified-name queries, script method catalogs, compiler semantic input,
reflection projection, hot-reload module changes, language-service symbols,
and every stable-ID call site together.

## 9. Structured Provider Attributes

The CST already supports structured attribute arguments. HIR must preserve
that structure rather than reconstructing a comma-separated string:

```rust
pub struct HirAttribute {
    pub path: Vec<String>,
    pub args: Vec<HirAttributeArg>,
    pub span: Span,
}

pub struct HirAttributeArg {
    pub name: Option<String>,
    pub value: HirAttributeValue,
    pub span: Span,
}
```

Provider syntax:

```vela
pub struct SortInventory {}

#[provider(id = "sort_inventory")]
impl CommandProvider for SortInventory {
    pub fn run(self, ctx: CommandContext, args: Array<String>) -> Result<CommandResult, String> {
        return CommandResult::Ok;
    }
}
```

Rules:

- `provider` is valid only on a resolved trait impl;
- exactly one named `id` string argument is required;
- positional arguments, duplicate keys, `service = ...`, and unknown keys are
  rejected with argument spans;
- the service is the resolved trait declaration, not a textual trait name;
- the target must resolve to a public zero-field script record in the first
  slice;
- provider ID uniqueness is scoped to `(PackageId, ServiceTraitId)`;
- every required trait method must be implemented or have a valid default;
- method signatures, type hints, effects, and access metadata must satisfy the
  service trait contract;
- discovery does not execute defaults, methods, globals, or native functions.

## 10. Provider Identity And Metadata

Stable identity is:

```text
ProviderKey = PackageId + ServiceTraitId + ProviderId
```

`ProviderId` is a validated manifest/source string carried by the attribute.
Renaming the provider type does not change `ProviderKey`; changing the ID is a
provider removal plus addition.

The public discovery descriptor contains stable identities and source
locations, not generation-local HIR IDs or live VM values:

```rust
pub struct ProviderDescriptor {
    pub key: ProviderKey,
    pub provider_type: TypeId,
    pub methods: Vec<ProviderMethodDescriptor>,
    pub package_declared_capabilities: CapabilitySet,
    pub package_statically_observed_capabilities: CapabilitySet,
    pub source: ProviderSourceLocation,
}
```

`ImplId` is not introduced. Internal discovery may retain a `HirDeclId` only
inside the sealed HIR/package compilation request. Compiled metadata maps a
`ProviderKey` to linked type and method-dispatch handles owned by the same
`LinkedArtifact` generation.

## 11. Package Workspace And Provider Discovery API

Package loading is general-purpose and host-controlled:

```text
root manifest
  -> PackageGraph
  -> package-aware HirSourceSet
  -> sealed PackageCompilationSnapshot
```

```rust
let packages = engine.load_package_workspace("app/vela.toml")?;
let artifact = engine.compile_package(&packages, &app_package_id)?;
```

The sealed snapshot is provider-independent:

```rust
pub struct PackageCompilationSnapshot {
    id: PackageCompilationSnapshotId,
    package_graph: Arc<PackageGraph>,
    sources: Arc<HirSourceSet>,
}
```

Its fields are not independently replaceable through public APIs. It may be
used to compile any workspace member as an ordinary root package. Loading and
compiling a package does not require a `ProviderCatalog` or
`ProviderSelection`.

Provider discovery is an optional read-only projection over the same snapshot:

```rust
let catalog = engine.discover_providers(&packages)?;
let providers = catalog.providers_for(service_trait_id);
```

`ProviderCatalog` is lightweight metadata and records the source
`PackageCompilationSnapshotId`. `ProviderSelection` records that same ID, so a
selection cannot be combined with a different package graph/HIR generation by
matching strings. Discovery errors preserve manifest and Vela source
diagnostics separately and discovery never executes code.

## 12. Compilation And Artifact Boundary

All package compilation uses one request:

```rust
pub struct PackageCompileRequest {
    snapshot: PackageCompilationSnapshotId,
    roots: BTreeSet<PackageId>,
}

pub struct ProviderCompileRequest {
    packages: PackageCompileRequest,
    providers: ProviderSelection,
}
```

Ordinary package compilation supplies only roots:

```rust
let request = PackageCompileRequest::for_root(&packages, app_package_id);
let artifact = engine.compile_packages(&packages, &request)?;
```

Provider-only compilation supplies selected provider keys; their owning
packages become compile roots:

```rust
let selection = ProviderSelection::from_catalog(&catalog, [provider_key]);
let request = ProviderCompileRequest::for_selection(&packages, selection);
let artifact = engine.compile_provider_selection(&packages, &request)?;
```

`compile_package` and `compile_provider_selection` are convenience APIs over
one internal sealed compilation request, not separate compiler pipelines.

The production pipeline remains:

```text
PackageCompilationSnapshot + base/provider compile request
  -> root/provider owning packages and transitive dependencies
  -> package-aware HirSourceSet selection
  -> ProgramCompilationRequest
  -> CompiledProgram
  -> Engine registry-aware linker
  -> Arc<LinkedArtifact>
```

Rules:

- every ordinary root package pulls in direct and transitive dependencies;
- ordinary source imports may use `crate::` and direct dependency aliases;
- cross-package access requires public declarations;
- provider selection adds provider-owning roots to the same compile closure;
- the first slice compiles complete selected packages, not function-level
  tree-shaken fragments;
- only explicitly selected provider keys enter the installed runtime table;
  other provider declarations in compiled packages remain discovery metadata;
- an ordinary request has an empty `InstalledProviderSet`;
- Engine remains the only production linker;
- package/provider metadata is sealed into the same executable generation as
  verified MIR, linked bytecode, debug metadata, and cache layouts;
- `LinkedArtifact` construction must require metadata from the same sealed
  compile request; callers cannot attach an independently built catalog;
- `ProgramImage`/linked records index providers by `ProviderKey` and linked
  handles, not source function names;
- ordinary execution constructs a Runtime from the artifact and calls normal
  linked entry functions;
- hot-reload initial/update APIs pass the artifact to `vela_hot_reload`, which
  alone constructs `ProgramVersion` or `HotUpdate`.

Required API distinction:

```text
load_package_workspace(...) -> PackageCompilationSnapshot
compile_packages(snapshot, request) -> Arc<LinkedArtifact>
compile_package(snapshot, root) -> Arc<LinkedArtifact>
compile_provider_selection(snapshot, provider_request) -> Arc<LinkedArtifact>
compile_package_hot_reload_initial(...) -> ProgramVersion
compile_package_hot_reload_update(...) -> HotUpdate
compile_provider_hot_reload_initial(...) -> ProgramVersion
compile_provider_hot_reload_update(...) -> HotUpdate
```

The artifact-owned installed table is distinct from the discovery catalog:

```rust
pub struct InstalledProviderSet {
    providers: BTreeMap<ProviderKey, LinkedProviderEntry>,
    selection: ProviderSelectionFingerprint,
}

pub struct LinkedProviderEntry {
    pub key: ProviderKey,
    pub provider_type: TypeHandle,
    pub receiver: ProviderReceiverPlan,
    pub methods: BTreeMap<MethodId, MethodDispatchHandle>,
    pub package_declared_capabilities: CapabilitySet,
}
```

`InstalledProviderSet` is constructed only by the linker from the sealed
selection and same-generation compile metadata. It is not attachable after
linking. `ProviderReceiverPlan` is a closed linked enum whose only first-slice
variant is `FreshZeroField`; this leaves an explicit extension point without
changing first-slice object-identity semantics.

`PackageCompileRequest`, `ProviderCompileRequest`, and `ProviderSelection`
contain the source
`PackageCompilationSnapshotId` to prevent cross-generation compilation. The
artifact stores a `PackageCompileRequestFingerprint` derived only from the
canonical sorted root `PackageId` and selected `ProviderKey` sets; it does not
contain the generation-local snapshot ID. `ProviderSelectionFingerprint` is
the provider subset of that request fingerprint. Reload reconstructs the same
root/provider request against a new package snapshot, then compares/installs
the resulting artifact.

## 13. Runtime Provider Calls

Provider lookup/call is an Engine Runtime embedding API. The VM only executes
the linked method target using existing call machinery.

```rust
runtime.call_provider(&provider_key, service_method_id, args, options)?;
```

`MethodId` is the primary API and linked-table key. A convenience method-name
API may resolve a name to the service `MethodId` once, but the execution path
does not repeatedly search source method strings.

First-slice construction rule:

- the provider target is a zero-field record;
- each top-level provider call constructs a fresh zero-field receiver in the
  active artifact generation;
- no singleton, persistent provider object, factory, or provider-owned host
  state exists;
- method invocation routes through the concrete linked trait impl target.

Handle rule:

- `ProviderHandle` contains the owning Runtime ID and stable `ProviderKey`, not
  a public generation-local linked handle;
- a handle from another Runtime is rejected;
- each call resolves the ProviderKey/MethodId pair in the current image;
- compatible hot reload automatically sends subsequent calls through the new
  installed linked entry;
- an internal resolved call target pins the active `Arc<LinkedArtifact>` for
  the duration of that call;
- old in-flight frames and closures continue on their pinned old artifact under
  the existing retained-generation semantics.

Provider calls keep normal execution-unit charging, call depth, GC rooting,
HostAccess, native capability checks, reflection policy, tracing, profiling,
and panic/error projection.

## 14. Capability Contract

The manifest declares requirements, the compiler observes statically resolved
effects in each complete compiled package, and the host grants runtime
authority. All layers use the same `vela_common::CapabilitySet`:

```text
statically observed requirements
  subset of manifest declared requirements
  subset of host grants
```

Rules:

- a statically observed capability missing from the manifest is a package
  compile diagnostic;
- the first slice conservatively scans all compiled code in each selected
  package, not only a provider reachability slice;
- statically resolved native, HostAccess, IO, time, random, event, and
  reflection effects contribute requirements;
- dynamic calls remain protected by the normal Runtime capability gate and do
  not justify claiming complete transitive effect inference;
- complete call-graph capability minimization is deferred;
- a declared capability missing from host grants rejects compilation/install
  of that selected package graph;
- no intersection silently removes a requirement;
- package capability expansion is part of hot-reload compatibility;
- the first slice uses the existing capabilities only: host read/write, event
  emit, time, random, IO read/write, and reflection read/write/call;
- unknown capability names are manifest diagnostics;
- catalog queries expose declared/statically-observed metadata but do not grant
  access or bypass reflection permissions;
- per-provider overrides and disabled-provider states are deferred.

## 15. Hot Reload Contract

Package/provider reload remains artifact- and `ProgramVersion`-based:

```text
changed source or manifest
  -> rebuild affected PackageGraph and HirSourceSet
  -> Engine compile and link Arc<LinkedArtifact>
  -> vela_hot_reload ABI/capability comparison
  -> HotUpdate
  -> Runtime safe-point staging and install
```

Compatibility checks include:

- root and dependency `PackageId` stability;
- the ordinary root package set from `PackageCompileRequestFingerprint`;
- stable definition paths affected by package/module changes;
- service trait addition/removal/method/signature/effect changes;
- every previously installed provider key still resolves in the rebuilt
  discovery snapshot;
- provider target type changes;
- provider method target/signature/effect changes;
- declared and statically observed capability expansion;
- public script schema used by provider parameters and return values.

A selected provider removal, service ABI change, provider target-type change,
or unapproved capability expansion is rejected. A provider body-only change is
accepted. An ordinary dependency body change follows the existing function and
schema ABI rules and can update an application with no providers installed.
Source/manifest reload reapplies the previous artifact's complete
`PackageCompileRequestFingerprint`; a newly discovered but unselected provider
is not installed and does not change runtime provider ABI. Changing root
packages or the installed provider selection is an explicit host restage
operation, not an incidental source reload. Rejected updates do not advance the
active image. Reports carry package, module, service, provider, manifest span,
and Vela source span context as applicable.

## 16. Language Service And LSP

Tooling uses the same `vela_package` manifest and graph model:

- root `ProjectState` loads workspace members, packages, dependencies, source
  roots, and root-only host schema configuration;
- open overlays override package disk snapshots;
- `crate::`, dependency aliases, and same-path modules in different packages
  resolve through the package-aware HIR graph;
- completion lists `crate` and direct dependency aliases;
- hover/symbols expose package and provider identity;
- definition navigates provider impl -> service trait and dependency imports ->
  manifests/source declarations;
- references find service provider impls across packages;
- diagnostics cover manifests, dependency cycles, duplicate IDs, unknown
  aliases, provider attributes, service mismatches, and capabilities;
- rename reports package/provider/service hot-reload ABI risk;
- editor tooling never executes scripts or loads the Rust host application.

Manifest changes and package source changes advance one language-service
generation at an explicit project refresh commit. Engine and tooling may have
different IO front doors, but they must assemble the same package graph and HIR
inputs from the same records.

## 17. Execution Policy

This plan combines independent verified checkpoints with one mandatory atomic
cutover:

1. Phase 0 is read-only inventory and baseline.
2. Phase 1 introduces the shared crate and unified manifest/project model. It
   may be committed when all existing consumers use it.
3. Phase 2 is one atomic breaking package-identity hard switch. Intermediate
   compilation failures are allowed; no compatibility adapter may be added.
4. Phases 3-7 add complete vertical behavior on top of the single identity
   model and may use small coherent commits.
5. Each resume inspects and continues the dirty worktree; do not reset partial
   identity work or recreate old APIs.
6. No phase is complete based only on type definitions or zero-hit searches;
   focused behavior tests must pass.

## 18. Phase 0: Inventory And Baseline

- [x] Record current workspace format, Clippy, tests, examples, and package-
      relevant benchmark compilation status.
- [x] Inventory every handwritten `vela.toml` parser and config model.
- [x] Inventory every `ModulePath`-only module index and module lookup.
- [x] Inventory every hard-coded `script` DefPath/stable-ID call site.
- [x] Inventory every capability enum/set definition and effect-to-capability
      mapping.
- [x] Inventory every Engine source/dir/hot-reload front door.
- [x] Inventory current HIR trait/impl/attribute metadata and validation gaps.
- [x] Inventory ProgramImage/LinkedArtifact metadata construction and Runtime
      handle/version rules.
- [x] Freeze behavior tests for existing single-source, directory, hot-reload,
      workspace overlay, schema, and stable-ID behavior.
- [x] Record current active-file sizes and dependency edges for affected crates.

Exit gate:

- [x] Every legacy surface has a named final owner and deletion phase.
- [x] Baseline failures, if any, are recorded before implementation edits.

## 19. Phase 1: `vela_package` And Unified Manifest

- [x] Add `vela_package` to the workspace with the dependency rules above.
- [x] Move the capability vocabulary and bitset to the single
      `vela_common::Capability` / `CapabilitySet` owner; update Engine and
      other consumers without retaining a duplicate Engine definition.
- [x] Add validated package/module identity types, manifest file spans, source
      table, manifest model, and package graph diagnostics.
- [x] Move the existing `ModulePath` type mechanically into `vela_package` and
      update imports to that one owner before adding package-aware indexes; do
      not create a duplicate package `ModulePath`.
- [x] Parse the unified root/package manifest with a structured TOML parser.
- [x] Resolve explicit workspace members and path dependencies.
- [x] Canonicalize and authorize manifest/source/dependency paths.
- [x] Detect duplicate package IDs, alias collisions, missing manifests, and
      dependency cycles.
- [x] Discover deterministic package sources across `[source].roots`.
- [x] Replace the language-service handwritten parser and source assembly with
      `vela_package` results.
- [x] Route Engine package/project IO through the same graph builder.
- [x] Hard switch tests/docs from `[workspace].roots` to the unified schema.
- [x] Keep Vela parsing in `vela_hir`; do not parse source in `vela_package`.

Focused tests:

- [x] `manifest_parses_workspace_package_sources_dependencies_and_capabilities`
- [x] `manifest_reports_unknown_keys_with_spans`
- [x] `manifest_and_engine_use_the_same_capability_ids`
- [x] `path_dependency_resolves_relative_to_manifest`
- [x] `source_root_cannot_escape_authorized_package_root`
- [x] `duplicate_package_id_at_different_manifests_is_rejected`
- [x] `dependency_cycle_reports_manifest_edge_chain`
- [x] `package_sources_are_deterministic`
- [x] `engine_and_language_service_assemble_the_same_package_graph`

Validation:

```bash
cargo test -p vela_package
cargo test -p vela_language_service project
cargo test -p vela_engine source
cargo clippy -p vela_package -p vela_language_service -p vela_engine --all-targets -- -D warnings
```

## 20. Phase 2: Package Identity Hard Switch

- [x] Use the single `vela_package::ModulePath` owner established in Phase 1;
      do not add an HIR alias or second module-path type.
- [x] Make `ModuleSource`/package source inputs carry `PackageId`.
- [x] Index HIR modules and children by `ModuleKey`.
- [x] Resolve `crate::` and direct dependency aliases.
- [x] Reject implicit transitive imports and unknown aliases.
- [x] Make declaration qualification and visibility package-aware.
- [x] Replace hard-coded `script` identity helpers with PackageId-aware DefPath
      construction for every script definition kind.
- [x] Update HIR method catalogs, analysis facts, MIR/bytecode semantic input,
      linker indexes, ProgramImage indexes, reflection, hot reload, Engine,
      language service, LSP, examples, and tests in the same cutover.
- [x] Make convenience source/file/dir APIs build an explicit reserved package.
- [x] Delete package-unaware source-set, module lookup, and stable-ID paths.
- [x] Add architecture guards against the hard-coded script package and
      ModulePath-only global indexes returning.

Focused tests:

- [x] `crate_import_resolves_within_current_package`
- [x] `dependency_alias_resolves_to_direct_package`
- [x] `transitive_dependency_requires_direct_alias`
- [x] `same_module_path_in_two_packages_does_not_collide`
- [x] `same_symbol_path_in_two_packages_has_distinct_stable_ids`
- [x] `single_source_uses_reserved_explicit_package`
- [x] `language_service_scratch_document_uses_reserved_explicit_package`
- [x] `package_aware_identity_survives_compile_link_runtime_and_reload`
- [x] `language_service_symbols_keep_package_ownership`

Mandatory zero-hit gates:

```bash
rg -n 'const SCRIPT_PACKAGE|DefPath::[a-z_]+\("script"' crates --glob '*.rs'
rg -n 'BTreeMap<ModulePath, ModuleId>|module_by_path' crates/vela_hir --glob '*.rs'
rg -n 'implicit package|legacy package|package_unaware|fallback.*package' crates --glob '*.rs'
```

Every hit must be eliminated or be an explicit assertion inside the guard that
prevents regression.

Validation:

```bash
cargo test -p vela_hir module_graph
cargo test -p vela_def
cargo test -p vela_analysis
cargo test -p vela_bytecode
cargo test -p vela_reflect
cargo test -p vela_hot_reload
cargo test -p vela_engine
cargo test -p vela_language_service
```

## 21. Phase 3: Ordinary Package Compilation Vertical Slice

- [ ] Add `Engine::load_package_workspace` returning one sealed
      `PackageCompilationSnapshot` without requiring provider discovery.
- [ ] Add `PackageCompileRequest` bound to the snapshot ID with ordinary root
      package IDs and no dependency on provider metadata or selection types.
- [ ] Resolve every root to its complete transitive dependency closure.
- [ ] Compile ordinary root and dependency packages through one HIR,
      `ProgramCompilationRequest`, `CompiledProgram`, and Engine linker path.
- [ ] Add `compile_packages` plus `compile_package` convenience API.
- [ ] Add ordinary package hot-reload initial/update front doors using the same
      linked-artifact boundary.
- [ ] Seal a stable request fingerprint containing root PackageIds even when no
      providers are installed.
- [ ] Keep ordinary artifacts' `InstalledProviderSet` empty.
- [ ] Validate public/private cross-package imports and direct-alias rules.
- [ ] Validate statically observed package capabilities against manifests and
      host grants without requiring provider metadata.
- [ ] Construct a Runtime from an ordinary package artifact and call its linked
      entry function.

Focused tests:

- [ ] `ordinary_package_imports_public_dependency_function`
- [ ] `ordinary_package_imports_dependency_type_and_method`
- [ ] `ordinary_package_rejects_private_dependency_declaration`
- [ ] `ordinary_package_includes_transitive_dependencies_but_not_their_aliases`
- [ ] `ordinary_package_compiles_and_runs_without_provider_catalog`
- [ ] `ordinary_package_artifact_has_empty_installed_provider_set`
- [ ] `ordinary_package_request_rejects_another_snapshot`
- [ ] `ordinary_dependency_body_reload_updates_root_package_calls`
- [ ] `ordinary_dependency_abi_change_is_rejected_without_image_advance`
- [ ] `ordinary_package_capability_use_must_be_declared_and_granted`

Validation:

```bash
cargo test -p vela_package
cargo test -p vela_hir package
cargo test -p vela_bytecode package
cargo test -p vela_engine package
cargo test -p vela_hot_reload package
```

## 22. Phase 4: Structured Provider HIR And Catalog

- [ ] Replace flattened HIR attribute values with structured arguments without
      regressing existing `doc`, `derive`, `id`, event, or policy attributes.
- [ ] Parse and validate `#[provider(id = "...")]` on trait impls.
- [ ] Resolve service trait and provider target declarations package-aware.
- [ ] Require a public zero-field provider target.
- [ ] Validate method coverage, defaults, signatures, return hints, effects,
      access, and declared/statically-observed capabilities.
- [ ] Reject duplicate provider keys and malformed/redundant arguments.
- [ ] Add stable `ProviderKey`, public descriptors, source locations, and a
      lightweight catalog bound to one `PackageCompilationSnapshotId`.
- [ ] Add `Engine::discover_providers(&PackageCompilationSnapshot)` without
      compilation or execution.
- [ ] Prove discovery does not execute top-level code or native/HostAccess work.

Focused tests:

- [ ] `provider_service_is_inferred_from_resolved_impl_trait`
- [ ] `provider_rejects_non_trait_impl_and_nonzero_field_target`
- [ ] `provider_rejects_redundant_unknown_duplicate_or_missing_id`
- [ ] `provider_rejects_method_signature_and_effect_mismatch`
- [ ] `duplicate_provider_key_is_rejected`
- [ ] `catalog_reports_stable_ids_and_source_spans`
- [ ] `discovery_does_not_execute_script_or_host_code`
- [ ] `catalog_cannot_mix_selection_from_another_generation`
- [ ] `statically_observed_effect_must_be_declared_by_package`

Validation:

```bash
cargo test -p vela_syntax attribute
cargo test -p vela_hir provider
cargo test -p vela_analysis provider
cargo test -p vela_engine provider_catalog
```

## 23. Phase 5: Linked Provider Runtime Vertical Slice

- [ ] Add provider selection by full `ProviderKey` bound to one
      `PackageCompilationSnapshotId`.
- [ ] Resolve owning packages and transitive dependencies.
- [ ] Compile complete selected packages through sealed HIR requests.
- [ ] Seal only selected providers into an `InstalledProviderSet` carried from
      `CompiledProgram` into `LinkedArtifact` in the same generation.
- [ ] Keep discovered but unselected providers out of runtime lookup and ABI.
- [ ] Link ProviderKey -> provider type handle -> MethodId -> method dispatch
      handle.
- [ ] Reject missing native definitions or cross-generation metadata at link.
- [ ] Add Runtime current-image lookup, fresh zero-field receiver construction,
      and provider method calls.
- [ ] Add Runtime-bound logical `ProviderHandle` values that re-resolve stable
      keys against the current compatible image.
- [ ] Keep resolved generation-local provider targets internal and pin their
      artifact only for the active call/frame.
- [ ] Preserve normal budgets, GC roots, HostAccess, capabilities, tracing,
      profiling, and errors.
- [ ] Keep provider lookup out of the core VM public API.

Focused tests:

- [ ] `compile_provider_selection_includes_transitive_dependencies`
- [ ] `linked_artifact_owns_same_generation_provider_metadata`
- [ ] `linked_artifact_installs_only_selected_providers`
- [ ] `runtime_calls_provider_trait_impl_method`
- [ ] `runtime_primary_provider_call_uses_method_id_without_name_dispatch`
- [ ] `runtime_rejects_missing_provider_or_method`
- [ ] `provider_call_constructs_fresh_zero_field_receiver`
- [ ] `provider_call_uses_normal_budget_host_access_and_capability_checks`
- [ ] `provider_handle_rebinds_after_compatible_reload`
- [ ] `provider_handle_rejects_another_runtime`

Validation:

```bash
cargo test -p vela_bytecode provider
cargo test -p vela_engine provider
cargo test -p vela_vm linked_execution
```

## 24. Phase 6: Package And Provider Hot Reload

- [ ] Add artifact-derived package/provider ABI records.
- [ ] Persist the installed selection fingerprint in the artifact/version and
      reapply it during ordinary source/manifest reload.
- [ ] Derive the fingerprint only from canonical selected ProviderKey values,
      not the generation-local PackageCompilationSnapshotId.
- [ ] Persist and reapply ordinary root PackageIds through the complete
      PackageCompileRequestFingerprint even when provider selection is empty.
- [ ] Stage changed manifests and sources through Engine package graph rebuild.
- [ ] Compute changed packages and affected dependents for reports.
- [ ] Compare service trait, provider key/target/method, package identity,
      public schema, and capability ABI.
- [ ] Accept provider body-only changes.
- [ ] Reject selected provider removal, target changes, service ABI changes,
      and unapproved capability expansion.
- [ ] Keep newly discovered but unselected providers out of the update ABI;
      require explicit host restaging to change the installed selection.
- [ ] Keep old active frames/closures pinned and move new calls to the accepted
      generation at the existing safe point.
- [ ] Include manifest and source labels in rejection reports.

Focused tests:

- [ ] `provider_body_change_is_accepted`
- [ ] `ordinary_package_reload_reapplies_previous_root_set`
- [ ] `unselected_provider_addition_does_not_change_runtime_abi`
- [ ] `ordinary_reload_reapplies_previous_provider_selection`
- [ ] `provider_removal_is_rejected_without_advancing_active_image`
- [ ] `service_trait_method_change_is_rejected`
- [ ] `provider_target_or_signature_change_is_rejected`
- [ ] `capability_expansion_requires_host_approval`
- [ ] `dependency_change_reports_impacted_packages`
- [ ] `old_frame_keeps_old_provider_generation_and_new_call_uses_new_generation`

Validation:

```bash
cargo test -p vela_hot_reload provider
cargo test -p vela_engine provider
cargo test -p vela_engine reload
```

## 25. Phase 7: Tooling, Examples, Docs, And Close-Out

- [ ] Load workspace/package manifests through `ProjectState` using
      `vela_package`.
- [ ] Preserve overlay precedence and one-generation refresh commits.
- [ ] Add completion for `crate` and direct dependency aliases.
- [ ] Add package/provider symbols, hover, definition, references, and rename
      risk metadata.
- [ ] Publish manifest/package/provider diagnostics without running host code.
- [ ] Add one standalone API-package + plugin-package example.
- [ ] Add one standalone ordinary app-package + library-package example with no
      provider declarations.
- [ ] Document manifest schema, imports, discovery, selection, calls,
      capabilities, and reload.
- [ ] Update `docs/architecture.md`, relevant subsystem architecture docs,
      `docs/decisions.md`, and `docs/progress.md` to final implemented truth.
- [ ] Add dependency-direction, parser ownership, package identity, artifact
      ownership, provider dispatch, and file-size architecture guards.
- [ ] Remove stale plan wording and mark tasks complete only after validation.

Focused tests:

- [ ] `lsp_completion_lists_crate_and_dependency_aliases`
- [ ] `definition_follows_provider_to_service_trait_across_package`
- [ ] `references_find_service_provider_impls_across_packages`
- [ ] `rename_provider_id_reports_hot_reload_risk`
- [ ] `manifest_change_refreshes_one_project_generation`
- [ ] `example_ordinary_package_dependency_compiles_runs_and_reloads`
- [ ] `example_plugin_provider_discovers_compiles_runs_and_reloads`

Validation:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo clippy --manifest-path examples/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path examples/Cargo.toml
cargo run --manifest-path examples/Cargo.toml --bin package_dependency_demo
cargo run --manifest-path examples/Cargo.toml --bin plugin_provider_demo
```

Final zero-hit and architecture gates must prove:

- one structured `vela.toml` parser exists;
- `vela_package` owns package/project identity and graph assembly;
- no hard-coded global script package identity remains;
- no package-unaware ModuleGraph index or import resolver remains;
- Engine is the sole production compiler/linker orchestrator;
- hot reload accepts artifacts, not manifests, HIR, or CompiledProgram;
- ordinary package compilation does not require provider discovery or catalog;
- provider runtime dispatch uses linked handles, not names;
- provider metadata cannot be attached across executable generations;
- only selected providers enter `InstalledProviderSet` and runtime ABI;
- provider handles re-resolve stable ProviderKey/MethodId pairs after a
  compatible reload;
- the capability vocabulary has one shared definition;
- language service and Engine consume the same package graph model;
- all active files satisfy the current file-size policy.

## 26. Deferred Questions

These do not block the first slice:

- stateful providers and explicit factories;
- singleton provider lifetime;
- multiple versions of one package in one runtime image;
- remote registries, lockfiles, signatures, and publishing;
- workspace member globs;
- deployment bundles containing bytecode plus package ABI metadata;
- per-provider host-approved capability subsets;
- provider enable/disable state independent of package selection;
- foreign host-language package modules.

## 27. Completion Criteria

The plan is complete only when:

- [ ] `vela_package` is the single shared package/manifest owner.
- [ ] Engine and language service use the same package graph/source assembly.
- [ ] `PackageId + ModulePath` is the only script module identity.
- [ ] all stable script definitions include PackageId.
- [ ] SourceId remains internal and deterministic.
- [ ] an ordinary root package imports, compiles, runs, and hot reloads its path
      dependencies without provider metadata.
- [ ] provider attributes are structured and source-spanned in HIR.
- [ ] discovery returns a sealed read-only catalog without execution.
- [ ] selected packages compile and link through the existing artifact pipeline.
- [ ] only the explicit selection is sealed into `InstalledProviderSet`.
- [ ] linked provider metadata is same-generation by construction.
- [ ] Runtime calls a zero-field provider by MethodId through linked trait impl
      dispatch.
- [ ] logical ProviderHandle values rebind across compatible image updates.
- [ ] capability declarations, statically observed effects, and host grants use
      one CapabilitySet and subset checks rather than intersection.
- [ ] provider hot reload preserves safe points, retained generations, and ABI
      rejection semantics.
- [ ] package/provider tooling works without executing scripts or host code.
- [ ] all focused, workspace, examples, architecture, dependency, zero-hit, and
      file-size gates pass.

The proving vertical slice is:

```text
ordinary app package -> path library dependency -> public import
Engine compiles and Runtime runs it without a ProviderCatalog
dependency body reload is accepted and dependency ABI break is rejected
one API package
one plugin package with a direct path dependency
one public service trait
one zero-field provider exported with #[provider(id = "...")]
host discovery lists the provider without execution
Engine compiles and links the selected package graph
Runtime calls the provider MethodId through linked trait dispatch
provider body hot reload is accepted at a safe point
the logical provider handle enters the compatible new generation
provider signature or capability expansion is rejected without image advance
language tooling resolves the dependency and provider/service navigation
```
