# Bytecode Source Boundary Hard-Switch Plan

> **Track:** compiler layering and source-front-door ownership
> **Document status:** Codex goal-mode execution plan
> **Execution status:** Complete. Checkpoints A-I landed and passed final
> validation on 2026-07-12.
> **Compatibility policy:** This is a breaking pre-release internal API hard
> switch. Preserve Vela language behavior, source identity, diagnostics,
> generated MIR/bytecode, linking, execution budgets, hot reload, and public
> Engine behavior. Do not preserve obsolete `vela_bytecode` source-text APIs.

This plan removes source parsing and module-graph construction from
`vela_bytecode`. The completed architecture has one dependency direction:

```text
source text
  -> vela_syntax parsing
  -> vela_hir source-set ingestion and ModuleGraph construction
  -> vela_bytecode validated-scope compilation
  -> vela_engine registry-aware linking
  -> vela_hot_reload LinkedArtifact generation operations
  -> linked execution
```

The switch is deletion-first and atomic at its production boundary. Temporary
compile failures are allowed while callers migrate, but a completed checkpoint
must not retain both source-text and graph-based bytecode compiler APIs.

---

## 0. Codex Goal

Use this prompt to execute the refactor:

```text
/goal Execute docs/bytecode-source-boundary-hard-switch-plan.md as a breaking,
deletion-first hard switch. Treat docs/goal.md as the product roadmap,
docs/architecture.md and docs/architecture/*.md as the architecture contract,
docs/decisions.md as durable design decisions, and docs/progress.md as the
rolling status source.

Move source-set parsing, syntax-diagnostic collection, ModuleGraph population,
and import resolution behind a focused vela_hir source-ingestion API. Preserve
the distinction and ordering between syntax and semantic diagnostics. Do not
make vela_hir depend on bytecode compiler error types.

Replace vela_bytecode compile_*_source entrypoints with validated source-set or
scope compiler entrypoints whose cohesive request type explicitly identifies
single-source versus module-graph compilation and function versus whole-program
roots without exposing independently combinable graph/module/declaration facts. Keep
script method identity, symbol naming, evaluated constants, schema defaults,
compiler options, registry input, verified MIR ownership, and bytecode output
behavior unchanged. A bare HirBody is not sufficient for whole-program input.

Migrate Engine source/file/directory compilation, text and directory hot
reload, bytecode compiler tests, VM tests, integration tests, examples, and
benchmarks in the same hard-switch track. Test code must use the production HIR
source-ingestion boundary or a thin test helper built on that boundary; it must
not restore parsing inside vela_bytecode or introduce a production fallback.

Delete parse_semantic_source, parse_semantic_modules, all public
compile_*_source functions in vela_bytecode, obsolete SemanticSource and
SemanticModules duplication, and the vela_syntax dependency from
crates/vela_bytecode/Cargo.toml. Do not add aliases, deprecated wrappers,
feature-gated legacy APIs, a second compiler path, or a new long-lived compiler
facade crate unless a documented dependency cycle proves the existing
vela_hir + vela_engine ownership impossible.

Validate focused HIR, bytecode, VM, Engine, hot-reload, diagnostic, example,
and benchmark behavior before running the complete workspace gates. Finish
with zero-hit dependency and source-API audits, update durable docs, and commit
coherent verified checkpoints using Conventional Commits. Do not mark the goal
complete while vela_bytecode directly or transitively exposes parsing as one of
its compiler responsibilities, while any old source-text compiler entrypoint
remains, or while diagnostic-stage equivalence is unproven.

After Checkpoints A-E, continue through the review close-out in Checkpoints
F-I. Do not treat the first graph API as the final boundary: make invalid
graph/scope/root combinations unrepresentable or reject them before semantic
input construction. Then make vela_engine the only production source
orchestrator. Delete source-text/module-source compile entrypoints from
vela_hot_reload, remove its production source-ingestion and standalone-linking
path, and feed hot reload generation operations with Engine-linked
LinkedArtifact values. Move source-to-reload behavior coverage to Engine; any
remaining source compilation inside vela_hot_reload must be cfg(test)-only
artifact fixture setup, not a public or production path.

Do not mark the goal complete until all Checkpoints F-I pass, the plan and
progress documents no longer report the review close-out open, invalid request
regressions are covered, vela_hot_reload has no production source compiler API,
and the final zero-hit and workspace validation gates are green.
```

---

## 1. Problem And Baseline

`vela_bytecode` currently has nine internal production dependencies. Most are
connected to its compiler, verifier, linker, and bytecode data model. The
specific confirmed layering violation is `vela_syntax`:

```text
vela_bytecode::compiler public API accepts SourceId + &str / ModuleSource
  -> compiler::semantic calls vela_syntax::parse_source_with_id
  -> compiler::semantic constructs ModuleGraph and resolves imports
  -> compiler prepares semantic input, verified MIR, and bytecode
```

This is normally the first parse, not a duplicate parse. The defect is
ownership: a backend crate also acts as the source compiler facade, reaches
backward into syntax, and owns front-end diagnostic staging.

The existing implementation also duplicates single-source and multi-module
semantic containers. Those containers differ mainly in selected module IDs,
symbol qualification, method-catalog mode, and schema/constant traversal. The
hard switch should represent those differences explicitly instead of retaining
two source-owning compiler paths.

Existing useful boundaries:

- `vela_hir` already owns the production `vela_syntax` dependency and
  `ModuleGraph::add_source`/`add_parsed_source`.
- `vela_engine` already owns user-facing source, file, directory, and hot-reload
  orchestration.
- `vela_bytecode` already consumes `ModuleGraph` internally when constructing
  semantic input, MIR, and bytecode.
- `vela_vm` already has a development dependency on `vela_hir`, so its source
  fixtures can construct graph inputs without creating an Engine/VM cycle.

---

## 2. Target Ownership And Dependency Contract

### 2.1 Layer Ownership

```text
vela_syntax
  owns lexer/parser/CST/AST and parse diagnostics

vela_hir
  owns source-set ingestion, parsed-source lowering, ModuleGraph construction,
  selected ModuleId values, import resolution, HIR diagnostics, and source hash

vela_bytecode
  owns graph-to-semantic-input preparation, const/schema compile-time
  evaluation until separately moved, verified MIR generation, physical
  bytecode emission, verification, and CompiledProgram

vela_engine
  owns embedding-facing source/file/directory compilation, compiler options,
  registry snapshots, linking, and hot-reload orchestration

vela_vm
  executes linked artifacts only; source compilation remains test/benchmark
  setup and uses the same HIR + bytecode boundaries as production
```

Mandatory dependency direction:

```text
vela_syntax -> vela_hir -> vela_analysis -> vela_mir -> vela_bytecode
                                                ^              |
                                                |              v
                                  stable target crates       vela_vm

vela_engine orchestrates vela_hir + vela_bytecode + linker/runtime
```

The arrow means "feeds/consumed by", not Cargo dependency notation.
`vela_bytecode` must have no direct Cargo dependency or Rust path reference to
`vela_syntax` in production, tests, examples, or benchmarks.

### 2.2 Source Ingestion Result

Add or refine one focused HIR-owned source-set ingestion API. Exact names may
follow local conventions, but the semantic shape must be cohesive:

```rust
pub struct HirSourceSet {
    pub graph: ModuleGraph,
    pub modules: Box<[ModuleId]>,
}

pub enum HirSourceBuildErrorKind {
    Syntax,
    Semantic,
}

pub struct HirSourceBuildError {
    pub kind: HirSourceBuildErrorKind,
    pub diagnostics: Vec<Diagnostic>,
}
```

Required behavior:

- accept one or more `ModuleSource` values in deterministic input order;
- parse every source and aggregate syntax diagnostics in deterministic source
  and diagnostic order;
- do not report syntax diagnostics as semantic diagnostics;
- do not proceed into bytecode compilation when syntax diagnostics exist;
- populate one `ModuleGraph`, retain selected `ModuleId` values, then resolve
  imports exactly once;
- return HIR/import diagnostics as the semantic stage;
- reject duplicate paths and invalid imports with current spans/codes/messages;
- keep HIR independent of `CompileError`, `CompilerOptions`, registry views,
  MIR, bytecode, Engine, and VM types.

If the existing `ModuleGraph` diagnostic storage cannot preserve stage
identity, introduce a build-result/error type rather than inferring stages from
diagnostic strings or codes. Do not clone parsing logic into Engine or tests.

### 2.3 Bytecode Compiler Input

Checkpoints A-E replaced the source-text API family with the following
graph-based request. This is the historical first shape and must be deleted by
Checkpoint G, not preserved as the final API:

```rust
pub enum ProgramCompilationMode {
    SingleSource { root: ModuleId },
    ModuleGraph { modules: Box<[ModuleId]> },
}

pub struct ProgramCompilationRequest<'a> {
    pub graph: &'a ModuleGraph,
    pub mode: &'a ProgramCompilationMode,
    pub options: &'a CompilerOptions,
    pub registry: Option<RegistryCompileView<'a>>,
}

pub struct FunctionCompilationRequest<'a> {
    pub graph: &'a ModuleGraph,
    pub module: ModuleId,
    pub function: HirDeclId,
    pub options: &'a CompilerOptions,
    pub registry: Option<RegistryCompileView<'a>>,
}
```

These are architectural shapes, not mandatory field-for-field names. Prefer a
request type over another family of `_with_options_and_registry` functions.

Required invariants:

- whole-program compilation receives a `ModuleGraph`, not a bare `HirBody`;
- function roots use stable HIR identity after the front end resolves names;
- single-source mode preserves the existing empty root module path and the
  narrow `main` method-identity namespace exception;
- module mode preserves qualified function/type/global symbols and deterministic
  module traversal;
- method catalogs, evaluated constants, schema defaults, semantic input,
  verified MIR, executable identities, budget layouts, and bytecode remain
  generated once per request;
- graph diagnostics are rejected before semantic-input/MIR construction;
- bytecode compiler errors no longer contain or manufacture parser diagnostics.

The first implementation of this shape exposed raw `ModuleId`/`HirDeclId`
combinations and a caller-owned module list. Post-completion review proved that
this leaves invalid states representable: an unknown root, an empty/partial or
duplicate module list, or a function/module mismatch can be silently filtered
or can mix full-graph method/metadata state with partial executable roots.

The final boundary must instead use one validated source-set/scope value. A
preferred shape is:

```rust
pub enum ProgramCompilationKind {
    SingleSource,
    ModuleGraph,
}

pub struct ProgramCompilationRequest<'a> {
    pub sources: &'a HirSourceSet,
    pub kind: ProgramCompilationKind,
    pub options: &'a CompilerOptions,
    pub registry: Option<RegistryCompileView<'a>>,
}

pub struct FunctionCompilationRequest<'a> {
    pub sources: &'a HirSourceSet,
    pub function: HirDeclId,
    pub options: &'a CompilerOptions,
    pub registry: Option<RegistryCompileView<'a>>,
}
```

Exact names may follow local conventions. Equivalent private-field request
constructors or a `ValidatedCompilationScope` are acceptable. The invariant is
mandatory: callers cannot independently combine a graph, arbitrary module IDs,
and a declaration. Single-source construction requires exactly one selected
module; module-graph construction consumes the complete ordered module set;
function construction derives and validates the owning module and executable
function kind/body from the selected source set.

### 2.4 Public Product API

Keep the embedding surface at `vela_engine`:

```text
Engine::compile_source
Engine::compile_file
Engine::compile_dir
Engine text/file/directory hot-reload entrypoints
```

Those APIs preserve current behavior while internally doing:

```text
load source(s)
  -> vela_hir source-set ingestion
  -> vela_bytecode graph compilation request
  -> link / version / runtime operations
```

Do not expose `SourceId` through ordinary single-source embedding APIs. Preserve
existing internal deterministic IDs used by diagnostics and tests.

The orchestration layer must use a structured error that keeps front-end and
backend ownership distinct. Exact names may follow existing Engine/hot-reload
conventions, but the shape must be equivalent to:

```rust
pub enum SourceCompilationErrorKind {
    Frontend(HirSourceBuildError),
    Backend(vela_bytecode::compiler::error::CompileError),
}
```

`EngineSourceError`, Engine hot-reload errors, CLI diagnostics, and tests
should project through this boundary without flattening diagnostics into strings.
Do not keep `CompileErrorKind::SyntaxDiagnostics` merely to avoid changing
Engine or hot-reload error plumbing.

### 2.5 Final Hot-Reload Boundary

Post-completion review fixes source orchestration ownership as follows:

```text
vela_engine
  source/file/directory
  -> HirSourceSet
  -> validated bytecode compilation request
  -> registry-aware link
  -> LinkedArtifact
  -> vela_hot_reload generation operation

vela_hot_reload
  previous ProgramVersion + LinkedArtifact + ABI + policy
  -> initial ProgramVersion / HotUpdate
```

`vela_hot_reload` must not expose production functions that accept `SourceId +
&str`, `ModuleSource`, `HirSourceSet`, `ModuleGraph`, or `CompiledProgram` and
then parse, compile, or link. Keep artifact/version operations such as
`initial_version_from_linked_artifact` and `update_from_linked_artifact`.
Registry-aware linking belongs to Engine because Engine owns registered native
implementations, compiler options, and embedding policy; the standalone linker
must not be a second source-to-generation route inside hot reload.

Engine owns the structured composition error for source ingestion, bytecode
compilation, linking, and hot-reload policy/ABI rejection. Do not retain a
`HotReloadErrorKind::Frontend` variant after hot reload no longer consumes
source. CLI, playground, examples, and public Engine results must still project
front-end diagnostics with their existing codes, spans, labels, and ordering.

---

## 3. Scope And Non-Goals

This plan includes:

- the HIR source-set build boundary and staged diagnostics;
- graph-based bytecode program and selected-function requests;
- removal of single-source/multi-module semantic container duplication where
  the new explicit mode makes it obsolete;
- migration of Engine compilation and all hot-reload source paths;
- migration of bytecode, VM, Engine, integration, example, and benchmark
  callers;
- deletion of every bytecode source-text compiler API and its syntax dependency;
- focused architecture, behavior, diagnostic, and dependency tests.

This plan does not include:

- changes to Vela syntax, HIR semantics, MIR shape, bytecode instructions, the
  linker, VM dispatch, GC, budgets, HostAccess, reflection, or hot-reload ABI;
- a general incremental compiler database or query engine;
- a new package manager or module syntax;
- parser recovery redesign or LSP workspace-state changes;
- moving const/schema evaluation merely to reduce the bytecode dependency count;
- splitting `vela_bytecode` into artifact and compiler crates without a separate
  dependency/ownership review;
- preserving old internal compile APIs for downstream compatibility;
- a new permanent `vela_compiler` facade crate by default.
- preserving `vela_hot_reload` source compile convenience APIs for internal
  compatibility.

The count of internal dependencies is an audit signal, not proof that the other
eight dependencies are invalid. Finish this confirmed boundary correction
before proposing a broader crate split.

---

## 4. Execution Checkpoints

Use this checklist as the durable tracker:

```text
[ ] not started
[~] in progress
[x] complete
```

### Checkpoint A: Freeze Behavior And Inventory Callers

- [x] Inventory every `compile_function_source*`, `compile_program_source*`,
      `compile_module_sources*`, `parse_semantic_source`, and
      `parse_semantic_modules` definition/import/call.
- [x] Classify callers as production Engine/hot reload, bytecode tests, VM unit
      tests, integration tests, examples, or benchmarks.
- [x] Add or identify fixtures for single-source success, multi-module success,
      syntax failure, duplicate module, unresolved import, function-not-found,
      method identity, constants, schema defaults, and registry-aware calls.
- [x] Record current diagnostic kind, order, code, span, and message for syntax
      versus semantic failures.
- [x] Record deterministic symbol names and compiled executable identities for
      representative single-source and module inputs.

Validation:

```bash
rg -n "compile_(function|program|module)_source|parse_semantic_(source|modules)" crates examples tests --glob '*.rs'
cargo test -p vela_bytecode
cargo test -p vela_vm
cargo test -p vela_engine
```

Checkpoint A may add characterization tests but must not add a second production
compiler path.

### Checkpoint B: Establish The HIR Source-Set Boundary

- [x] Add the HIR-owned source-set build result and staged error contract.
- [x] Make single-source and multi-module ingestion use the same implementation.
- [x] Preserve parse-all-before-lowering behavior for multi-module syntax
      diagnostics.
- [x] Preserve deterministic source/module/diagnostic ordering.
- [x] Resolve imports once after all syntax-clean modules are present.
- [x] Add HIR tests for syntax-stage and semantic-stage separation.
- [x] Add tests for empty root path, relative-path module identity, duplicate
      module paths, unresolved imports, and multiple-source diagnostics.
- [x] Keep existing lower-level `add_parsed_source` only if syntax/LSP callers
      genuinely require it; do not use it to bypass the production build result.

Validation:

```bash
cargo test -p vela_hir
cargo clippy -p vela_hir --all-targets -- -D warnings
```

### Checkpoint C: Build The Graph-Based Bytecode Boundary

- [x] Introduce cohesive program and selected-function compilation request
      types.
- [x] Collapse `SemanticSource` and `SemanticModules` into one graph-borrowing
      semantic compilation context plus explicit mode, or document why a smaller
      equivalent representation is clearer.
- [x] Resolve function names to `HirDeclId` outside the physical backend; prefer
      stable identity in the selected-function request.
- [x] Preserve single-source and module symbol qualification exactly.
- [x] Preserve method catalog modes and the single-source `main` identity
      exception.
- [x] Preserve constant fixed-point evaluation and schema-default traversal.
- [x] Preserve `CompiledProgram` verified-MIR ownership, executable identity,
      budget layout, bytecode verification, and linker handoff.
- [x] Add graph-based compiler tests for every frozen fixture.
- [x] Do not yet commit a completed checkpoint with both public API families.

Focused validation:

```bash
cargo test -p vela_bytecode
cargo clippy -p vela_bytecode --all-targets -- -D warnings
```

### Checkpoint D: Atomic Caller Hard Switch And Deletion

Perform these changes as one hard-switch checkpoint:

- [x] Migrate `Engine::compile_source`, `compile_file`, and `compile_dir`.
- [x] Migrate initial/update/staged hot reload for source, file, directory, and
      changed-file paths.
- [x] Introduce or update the orchestration-layer source compilation error so
      HIR front-end failures and bytecode backend failures remain structured.
- [x] Migrate bytecode compiler tests to the HIR source-set boundary.
- [x] Migrate VM tests and test-local helpers using its existing HIR dev
      dependency.
- [x] Migrate integration tests, conformance tests, examples, and all benchmarks.
- [x] Replace repeated test setup with a thin helper only when it removes real
      duplication; the helper must call production HIR and bytecode APIs.
- [x] Delete every `compile_*_source*` function from `vela_bytecode`.
- [x] Delete `parse_semantic_source` and `parse_semantic_modules`.
- [x] Delete obsolete source-owning semantic containers and imports.
- [x] Remove `vela_syntax` entirely from `vela_bytecode/Cargo.toml`; do not move
      it to dev-dependencies.
- [x] Remove syntax-diagnostic variants from bytecode `CompileError` if they no
      longer represent graph-to-bytecode failures; the orchestration-layer
      error must still expose the same user-visible diagnostics.
- [x] Do not leave deprecated wrappers, aliases, feature gates, fallback paths,
      or commented migration code.

Checkpoint D is incomplete if the tree is green only because both API families
remain available.

Focused validation:

```bash
cargo test -p vela_hir
cargo test -p vela_bytecode
cargo test -p vela_vm
cargo test -p vela_engine
cargo test -p vela_hot_reload
cargo check -p vela_vm --benches
cargo check -p vela_engine --benches
```

### Checkpoint E: Architecture Closure And Full Validation

- [x] Add a dependency/zero-hit test or CI audit that prevents
      `vela_bytecode -> vela_syntax` from returning.
- [x] Prove no source-text compilation API remains in `vela_bytecode`.
- [x] Prove bytecode tests do not own a hidden parser facade.
- [x] Compare frozen diagnostic stage/order/code/span/message fixtures.
- [x] Compare representative generated MIR, bytecode verification, executable
      identities, and runtime results.
- [x] Run all examples required by `docs/validation.md`.
- [x] Run the active-file size audit and split migration-dense files when
      ownership becomes unclear.
- [x] Update `docs/progress.md` with the completed boundary and validation.
- [x] Update this document's execution status and checklist.
- [x] Commit completed checkpoints with Conventional Commits and finish with a
      clean worktree.

Zero-hit audits:

```bash
rg -n "vela_syntax" crates/vela_bytecode Cargo.toml crates/vela_bytecode/Cargo.toml
rg -n "compile_(function|program|module)_source" crates/vela_bytecode crates/vela_vm crates/vela_engine examples tests --glob '*.rs'
rg -n "parse_semantic_(source|modules)|SemanticSource|SemanticModules" crates/vela_bytecode/src --glob '*.rs'
```

The first command must have no `vela_bytecode` dependency or source hit. The
second may only match an explicitly renamed test helper if that name remains
useful; it must not match a bytecode public API or implementation. Prefer names
such as `compile_test_program` that make fixture ownership clear.

Full validation:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Run the complete example and benchmark-build subset from `docs/validation.md`
as it exists at execution time.

### Checkpoint F: Reopen And Freeze Review Findings

- [x] Record the current public compilation-request shape and every production
      constructor/caller.
- [x] Record the negative acceptance matrix for an unknown single-source root,
      empty/partial/duplicate module scope, a module ID from another graph, a
      non-function declaration root, and a function/module mismatch; do not add
      failing or ignored tests to the green characterization checkpoint.
- [x] Define the invariant fixture and expected outcome for a graph containing
      an excluded function and script method; the passing regression lands with
      the sealed request API in Checkpoint G.
- [x] Inventory every public and private `vela_hot_reload::compile_*` function
      that accepts source text or `ModuleSource`, plus all external callers.
- [x] Classify source-to-reload tests: move embedding/end-to-end behavior to
      Engine, and retain only ABI/policy/generation tests in hot reload.
- [x] Pin Engine source, file, directory, initial reload, update reload, staged
      reload, and changed-file diagnostic behavior before API deletion.
- [x] Replace the bytecode linker metadata fixture's direct
      `ModuleGraph::add_source` call with the production HIR ingestion boundary.

Validation:

```bash
cargo test -p vela_hir
cargo test -p vela_bytecode
cargo test -p vela_hot_reload
cargo test -p vela_engine
```

### Checkpoint G: Seal Compilation Request Invariants

Perform this as a breaking request-API replacement, not a parallel wrapper:

- [x] Replace public raw graph + mode IDs with `HirSourceSet`, a validated
      compilation scope, or equivalent private-field constructors.
- [x] Add passing negative tests proving every invalid case recorded in
      Checkpoint F is rejected before semantic-input construction or cannot be
      represented by the public API.
- [x] Add the passing scope-consistency regression recorded in Checkpoint F and
      prove bytecode roots, method catalog, and retained metadata cannot observe
      different module sets.
- [x] Derive the single-source root from a source set that contains exactly one
      selected module.
- [x] Make module-graph compilation consume exactly the complete ordered module
      set represented by the source-set/scope value; do not silently accept
      arbitrary subsets or duplicate module IDs.
- [x] Derive a selected function's module from its declaration and reject
      missing, wrong-kind, bodyless, or out-of-scope declarations before
      semantic-input construction.
- [x] Ensure method catalog, symbol maps, constant evaluation, schema defaults,
      semantic roots, retained script metadata, and executable roots all consume
      the same validated scope.
- [x] Add a structured request-invariant error instead of returning an unrelated
      registry/MIR inconsistency for malformed public input.
- [x] Delete the old public `ProgramCompilationMode` shape and every constructor
      that can assemble independent graph/module/declaration facts.
- [x] Migrate bytecode, Engine, VM, integration, benchmark, and test-support
      callers atomically; do not keep aliases or deprecated constructors.

Focused validation:

```bash
cargo test -p vela_hir -p vela_bytecode -p vela_vm -p vela_engine
cargo clippy -p vela_bytecode -p vela_engine --all-targets -- -D warnings
```

### Checkpoint H: Make Engine The Only Production Source Orchestrator

Perform the source-to-hot-reload API deletion atomically:

- [x] Route every Engine source/file/directory compile and hot-reload entry
      through one Engine-owned source-set -> compile -> registry-aware-link
      helper or cohesive service.
- [x] Ensure Engine passes only `LinkedArtifact` plus ABI/policy/version inputs
      into `vela_hot_reload` generation operations.
- [x] Delete public `vela_hot_reload::compile_initial*`, `compile_update*`, and
      module-source variants that parse, compile, or link.
- [x] Delete `compile_single_program`, `compile_module_program`, the production
      `build_source_set` import, source/model imports used only by those paths,
      and the standalone source-linking helper from `vela_hot_reload`.
- [x] Delete `HotReloadErrorKind::Frontend` and project front-end failures from
      an Engine-owned structured hot-reload compilation error instead.
- [x] Move source-to-reload and embedding behavior tests to `vela_engine`.
- [x] Convert remaining hot-reload unit fixtures to prebuilt `LinkedArtifact`
      values. A single `#[cfg(test)]` source-to-artifact helper is acceptable
      only when it calls the production HIR/bytecode/link boundaries and is not
      exported or compiled in production.
- [x] Update CLI, playground, examples, C/public embedding surfaces, and docs
      for the Engine-owned error/API shape without flattening diagnostics.
- [x] Do not add a permanent `vela_compiler` crate or move source orchestration
      back into HIR, bytecode, or hot reload.

Focused validation:

```bash
cargo test -p vela_hot_reload
cargo test -p vela_engine
cargo test -p vela_cli
cargo test -p vela_playground_wasm
cargo check -p vela_engine --benches
```

### Checkpoint I: Review Close-Out And Full Validation

- [x] Add CI zero-hit checks preventing production source ingestion/compilation
      APIs from returning to `vela_hot_reload`.
- [x] Add CI or focused architecture tests for the sealed bytecode request
      boundary and forbidden raw-mode constructors.
- [x] Prove `vela_bytecode` still has no syntax dependency or source-text API.
- [x] Prove only Engine production code calls `build_source_set` as part of
      source-to-executable orchestration; HIR/LSP consumers and cfg(test)
      fixtures must remain clearly scoped exceptions.
- [x] Prove `vela_hot_reload` production entrypoints accept linked artifacts
      rather than source, graphs, source sets, or compiled programs. Reading
      immutable script metadata carried inside an artifact for ABI comparison
      remains valid.
- [x] Run the active-file size audit and review any newly oversized Engine or
      hot-reload files.
- [x] Run all required examples, benchmark builds, fuzz build, formatting,
      clippy, and workspace tests.
- [x] Update `docs/architecture.md`, `docs/decisions.md`, `docs/progress.md`, and
      this plan to completed current truth.
- [x] Commit coherent hard-switch checkpoints and finish with a clean worktree.

Additional zero-hit audits:

```bash
rg -n "build_source_set|ModuleSource|HirSourceSet|CompiledProgram" crates/vela_hot_reload/src --glob '*.rs'
rg -n "pub fn compile_(initial|update)" crates/vela_hot_reload/src --glob '*.rs'
rg -n "ProgramCompilationMode|ModuleGraph \{ modules|FunctionCompilationRequest.*module" crates --glob '*.rs'
rg -n "\.add_source\(" crates/vela_bytecode --glob '*.rs'
```

Expected production results are zero. `#[cfg(test)]` artifact-fixture hits must
be reviewed manually and cannot expose public APIs or duplicate production
orchestration.

---

## 5. Required Test Matrix

| Area | Required proof |
|---|---|
| Single source | empty root module identity, `main` lookup, methods, globals |
| Module graph | deterministic paths, imports, qualified symbols, all roots |
| Syntax errors | syntax stage, complete aggregation, stable order/code/span |
| HIR errors | semantic stage, imports/duplicates, stable order/code/span |
| Function compile | stable `HirDeclId` selection and not-found projection |
| Constants/defaults | cross-module constants and schema defaults unchanged |
| Registry | native/host/stdlib target resolution unchanged |
| MIR/bytecode | verified bundle, executable IDs, budgets, verifier unchanged |
| Engine | source/file/directory behavior and error projection unchanged |
| Hot reload | initial/update/staging/changed-file behavior unchanged |
| VM/tests | source fixtures use production HIR + bytecode boundaries |
| Performance setup | benchmark compilation succeeds without legacy helpers |

Tests should assert structured diagnostics where available. Snapshotting only a
formatted debug string is insufficient proof of stage, code, and span
preservation.

---

## 6. Commit Strategy

Recommended checkpoints:

```text
test(compiler): freeze source boundary behavior
feat(hir): add staged source-set graph construction
refactor(bytecode): hard switch compiler to HIR graph input
docs: close bytecode source boundary hard switch
test(compiler): freeze source boundary review regressions
refactor(bytecode)!: seal graph compilation scope
refactor(reload)!: make Engine the source orchestrator
docs: close bytecode source boundary review
```

The original A-E hard-switch commit and the follow-up G/H commits may be large
because caller migration and deletion are atomic architecture checkpoints. Do
not split them into commits that leave a merged-compatible dual production API.
Recovery commits are allowed while the goal is active, but a checkpoint is not
complete until its declared tests are green.

---

## 7. Completion Gate

Do not mark this goal complete while any of the following is true:

- any Checkpoint A-E item is unchecked;
- any Checkpoint F-I item is unchecked;
- `vela_bytecode` depends on or references `vela_syntax` anywhere;
- bytecode exposes an API accepting source text or performs parsing/module-graph
  construction;
- old and new compiler entrypoints coexist for compatibility;
- Engine or hot reload bypasses the HIR source-set boundary;
- raw graph/module/declaration combinations can construct an inconsistent
  bytecode compilation request;
- executable roots, script methods, constants/defaults, and retained metadata
  can observe different module scopes;
- `vela_hot_reload` production code accepts source text, `ModuleSource`, HIR
  graphs/source sets, or `CompiledProgram`, or performs standalone linking;
- Engine is not the sole production owner of source/file/directory compilation
  and source-to-hot-reload orchestration;
- tests use a hidden parser/compiler implementation that production does not;
- syntax and semantic diagnostics can change stage, order, code, span, or
  message without a reviewed intentional decision;
- single-source method identity, module-qualified symbols, constants, schema
  defaults, verified MIR, budgets, or bytecode behavior lack regression proof;
- focused tests, full workspace checks, required examples, benchmark builds,
  zero-hit audits, documentation updates, or file-size audits have not passed;
- this document or `docs/progress.md` still reports the hard switch unfinished;
- required commits are missing or the final worktree is dirty.

Completion means the repository has one Engine-owned source-to-executable
route, each layer owns its correct abstraction, bytecode consumes a validated
HIR compilation scope, and hot reload begins at an Engine-linked artifact.
