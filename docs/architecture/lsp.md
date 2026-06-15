# Native LSP Architecture

Vela's language server is a full native LSP capability track that may land
before the MVP and may progress in parallel with M19/M20 optimization. It
should deliver diagnostics, completion, signature help, hover, go to
definition, symbols, semantic tokens, references, rename, code actions,
formatting, inlay hints, source overlays, static host schema facts, and
incremental invalidation without becoming a custom IDE product or changing
language/runtime semantics. It should be native-first for scale and platform
integration, while keeping the reusable language-service core independent from
LSP transport, editor APIs, and filesystem access.

The target scale is a workspace with around one million lines of `.vela`
source spread across multiple modules. The design must avoid per-keystroke full
project rebuilds, keep open-document overlays authoritative, and preserve the
existing host boundary, reflection, and hot-reload contracts.

## Layering

```text
VS Code / Zed / JetBrains / CLI
        |
        v
vela_lsp_server
  LSP JSON-RPC, file watching, editor config, cancellation, progress
        |
        v
vela_language_service
  workspace state, query API, diagnostics, completion, hover, definitions
        |
        v
workspace databases
  SourceDb, ProjectDb, ParseDb, HirDb, AnalysisDb
        |
        v
vela_syntax / vela_hir / vela_analysis / vela_reflect
```

Optional browser tooling may wrap `vela_language_service` through WASM later,
but WASM is not the primary LSP deployment target. Native binaries are the
default release unit for editor integrations because they can use threads,
filesystem watchers, memory mapping, and platform-specific process behavior.

## Crate Boundaries

`vela_language_service` owns reusable editor analysis logic:

```text
workspace snapshots
source overlays
module graph construction
incremental invalidation
query APIs for diagnostics, completion, hover, definitions, and references
conversion from Vela diagnostics to editor-neutral diagnostics
```

It must not:

```text
read the filesystem directly
depend on LSP protocol types
spawn editor processes
execute Vela programs
inspect live host state
mutate TypeRegistry or runtime type structure
```

`vela_lsp_server` owns protocol and platform integration:

```text
LSP initialize/shutdown lifecycle
JSON-RPC transport over stdio or sockets
didOpen/didChange/didClose/didSave handling
workspace folder and file-watch events
request cancellation
progress and work-done notifications
publishDiagnostics
mapping between LSP positions and language-service offsets
```

Editor plugins should be thin launchers around the native LSP binary. They may
provide configuration UI and binary discovery, but feature behavior should live
in `vela_language_service` or `vela_lsp_server`.

## Workspace Model

Vela is not a single-file scripting system. The LSP should prefer
`compile_dir`-style module graph semantics and use single-file behavior only as
a fallback.

The service should maintain:

```text
SourceDb
  FileId -> text, version, line index, content hash, source kind

ProjectDb
  workspace roots, config, file set, module path map, import reverse deps

SchemaDb
  host TypeRegistry/RegistryFacts snapshot, schema version, diagnostics

ParseDb
  FileId -> parsed AST/CST equivalent, parse diagnostics, declaration summary

HirDb
  ModuleId/FileId -> lowered declarations, binding maps, semantic diagnostics

AnalysisDb
  ModuleId/FileId -> TypeFacts, completion facts, hover facts, reference index
```

Open editor buffers are overlays:

```text
open document text wins over disk snapshot
disk snapshot wins over missing source
missing source produces diagnostics instead of panics
```

The source store should expose immutable snapshots to analysis queries. State
mutation, file events, and document changes should advance monotonically
increasing workspace generations so stale query results can be discarded.

## Project Configuration

The preferred project file is `vela.toml`:

```toml
[workspace]
roots = ["scripts"]

[host]
schema = "target/vela/schema.json"
```

The first implementation may support only one root. Multiple roots should be a
planned extension of the same model, not a separate mode.

Fallback behavior:

```text
explicit vela.toml                 -> configured compile_dir workspace
workspace folder with .vela files  -> inferred compile_dir workspace + warning
single opened .vela file only      -> compile_file-style scratch workspace
missing host schema                -> syntax/HIR/stdlib tooling only
```

## Host Schema Input

The language server must not run the host application to discover types,
fields, methods, capabilities, or permissions. Host integration should be
provided through a static schema artifact exported from the host's
`TypeRegistry`/`RegistryFacts` data.

The schema artifact should carry copied metadata:

```text
stable TypeId, FieldId, MethodId, VariantId, TraitId, FunctionId
qualified names and display names
type hints and builtin container contracts
field and method access metadata
EffectSet and required capabilities
docs and declaration origins
source spans when known
schema hash or version
```

When the schema is absent or stale, tooling should degrade to `Any` and report
schema diagnostics. It must not invent host facts or read host state.

## Incrementality

The first diagnostic slice may rebuild a complete module graph through the
language-service boundary, but the architecture must not bake that in. The
target invalidation model is:

```text
text changed
  -> update source version and line index
  -> reparse changed file
  -> compare declaration/import fingerprints
  -> update module path and reverse dependency indexes if needed
  -> invalidate changed module and affected reverse dependencies
  -> recompute open-file diagnostics first
  -> recompute remaining impacted files in the background
```

Requests should be cancellable. Long-running analysis should observe a
generation token and return stale results as discardable rather than publishing
them after a newer edit.

## Query Priorities

The service should treat editor work as latency-sensitive:

```text
completion at cursor       highest priority, may use partial/stale facts
hover/definition           high priority, current file first
open-file diagnostics      normal priority, debounced
workspace diagnostics      background priority
reference index rebuild    background priority
```

Queries should degrade gracefully. A syntax error in one file should not block
completion in another file when the last known project graph is still usable.

## Feature Mapping

Foundation features:

```text
diagnostics       parser + HIR + analysis diagnostics
completion        SymbolTable + TypeFact + TypeRegistry/RegistryFacts
signature help    call target facts + TypeRegistry/RegistryFacts
hover             TypeFact + docs + EffectSet + DeclOrigin
go to definition  BindingMap + DeclOrigin + source spans
```

Full capability phases:

```text
semantic tokens   CST/token kinds plus resolved symbol classes
find references   reference index derived from BindingMap
rename            symbol ownership, module visibility, and conflict checks
code actions      structured diagnostic repair hints
formatting        lossless CST/trivia policy plus deterministic formatter IR
inlay hints       stable TypeFacts only, never mandatory static typing
```

References, rename, code actions, and formatting require their underlying
indexes and syntax trivia policy first, but they are part of the native LSP
capability track rather than a separate custom IDE project.

## Threading And Memory

The LSP server should keep a single coordinator for workspace mutation and use
worker pools for parsing and analysis. Shared analysis inputs and results should
be immutable snapshots behind `Arc`-style ownership so request handlers do not
block each other on long mutable borrows.

Memory-sensitive structures should use stable IDs rather than copying paths or
source text through every layer:

```text
FileId, ModuleId, SourceVersion, WorkspaceGeneration
text hash and declaration fingerprint
line index cached per source version
interned module paths and symbol names where profiling proves value
```

The one-million-line target requires scale tests before declaring the LSP
architecture complete enough.

## Runtime And Debugger Boundary

The LSP is an analysis service, not a VM runtime. It must not execute scripts
for completions, hovers, or diagnostics. Runtime debugging belongs to the DAP
track and should share source maps, frame metadata, and schema display helpers
only through explicit data structures.

The LSP must preserve these product contracts:

```text
no Rust &mut exposure
no host state under script GC
no runtime TypeRegistry mutation
no monkey patching
no bypass of HostAccess, reflection policy, or capability metadata
```

## WASM Boundary

WASM may wrap `vela_language_service` for browser tooling. That wrapper should
receive source text, schema JSON, and configuration through a virtual
workspace API. It should not change the native LSP architecture or force the
native server to avoid platform capabilities.

## Non-Goals

The native LSP architecture must not:

```text
build a custom full IDE product beyond native server and thin editor launchers
couple editor diagnostics to VM execution
run the host application for schema discovery
read host object state for editor hints
change language or runtime semantics for editor convenience
preserve temporary pre-LSP APIs for compatibility
make WASM the primary LSP transport
make single-file analysis the dominant project model
block all editor features on perfect whole-workspace analysis
```

## Validation

LSP work should be validated with:

```text
language-service unit tests for source overlays and invalidation
module graph tests for multi-file import changes
snapshot fixtures for diagnostics, completion, hover, and definitions
LSP JSON-RPC fixtures for initialize, document sync, and requests
synthetic scale tests that approach one million lines without full rebuilds per edit
full workspace cargo checks before milestone checkpoints
```
