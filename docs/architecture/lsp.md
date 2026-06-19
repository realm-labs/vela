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
  typed LSP transport, file watching, editor config, cancellation, progress
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
typed `lsp_server::Message` transport over stdio or optional loopback TCP
didOpen/didChange/didClose/didSave handling
workspace folder and file-watch events
request cancellation
progress and work-done notifications
publishDiagnostics
mapping between LSP positions and language-service offsets
```

## Protocol And Main Loop Boundary

Vela follows rust-analyzer's editor-server shape: editor packages launch a
native process, the process builds an `lsp_server::Connection`, and a single
main loop owns the mutable coordinator state. The current rust-analyzer
production entry in the local reference checkout is stdio-only; Vela keeps
stdio as the default editor transport for the same reason. Vela's TCP mode is
a debug/remote-integration extension, not a rust-analyzer compatibility
requirement.

The protocol boundary is typed:

```text
stdio or loopback TCP bytes
  -> lsp_server::Message
  -> main_loop event pump
  -> GlobalState request queue, cancellation, generations, and config
  -> typed request/notification dispatch
  -> vela_language_service snapshot query
  -> typed LSP response/progress/diagnostic messages
```

Production code must not route normal stdio or TCP traffic through
hand-written JSON-RPC envelopes, custom request IDs, manual Content-Length
parsers, or stringly request builders. JSON serialization is limited to the
wire boundary, tracing/profiling byte counts, and tests that inspect final
protocol shapes. Invalid params, method-not-found, cancellation, stale
generations, handler panics, and response projection errors flow through the
shared dispatcher/main-loop path.

`GlobalState` is the only mutable protocol coordinator. It owns lifecycle
flags, workspace roots, editor configuration, watcher settings, request queue
state, generation checks, cancellation handles, and task scheduling. Request
handlers receive typed params and either mutate `GlobalState` directly for
coordinator work or take immutable snapshots for read-only language-service
queries. Long-running or latency-sensitive work is scheduled through explicit
main-loop lanes:

```text
mutable notifications/lifecycle/config   main loop, ordered
latency-sensitive reads                  bounded latency lane
formatting                               formatting lane
workspace diagnostics/reload/indexing    background lane
```

Optional TCP must feed the same typed connection, main loop, request queue,
`GlobalState`, handler dispatch, cancellation, profiling, and projection
modules as stdio. TCP binding defaults to loopback-only addresses and must not
accept unauthenticated non-loopback listeners unless a future explicit opt-in
flag defines that risk and its validation.

Editor plugins should be thin launchers around the native LSP binary. They may
provide configuration UI and binary discovery, but feature behavior should live
in `vela_language_service` or `vela_lsp_server`.

## Query And Projection Boundary

Editor requests flow through one shared service boundary:

```text
LSP request
  -> vela_lsp_server protocol params and cancellation
  -> WorkspaceSnapshot / LanguageServiceDatabases query
  -> QueryContext with source, module, HIR, analysis, schema, and generation facts
  -> CursorContext classification from parser/token spans
  -> feature-specific producer
  -> editor-neutral result model
  -> vela_lsp_server protocol projection
```

`QueryContext` is the request-local fact carrier. It may expose service-owned
document IDs, source ranges, module paths, binding maps, receiver facts,
callable facts, local bindings visible before the cursor, schema facts, and
generation/cancellation tokens. It must not expose LSP protocol structs,
filesystem watchers, JSON-RPC request IDs, or editor-specific configuration
objects.

`CursorContext` is syntax-owned and shared by completion, hover, signature
help, definition, references, rename, code actions, formatting entry points,
and inlay hints where cursor classification matters. Feature code may extract
additional syntax facts after the shared classifier selects a context, but it
must not reclassify broad request kind through independent string scanning.

Shared result models carry editor-neutral identity and display data:

```text
SymbolRef      source, schema, builtin, local, member, variant, and module identity
DisplayParts   structured labels/details/diagnostic text before protocol rendering
EditPlan       checked source-owned workspace edits with versions and conflicts
Relevance      service-owned completion ranking metadata projected to sort/preselect
```

Completion producers return rich service items with replacement ranges,
filter/lookup text, label details, snippet intent, deprecation, symbol
identity, and optional resolve payloads. Expensive schema documentation is
resolved lazily through a service-owned payload and projected by
`vela_lsp_server` into `completionItem/resolve`.

### Rust-Analyzer-Style Authoring Core

Vela's Rust-like syntax should use a rust-analyzer-style authoring model where
the syntax overlaps, adapted to Vela's dynamic and host-schema contracts. The
goal is the shape of the editor architecture, not Rust-only semantics. The LSP
must not import Rust macros, borrow checking, Rust trait solving, or
script-language generics.

Completion should be a two-phase pipeline:

```text
syntax recovery + semantic facts
  -> structured CompletionAnalysis
  -> feature producers
  -> editor-neutral completion items
  -> LSP projection
```

`CompletionAnalysis` is owned by `vela_language_service` and should replace
patch-only request handling with explicit contexts:

```text
PathCompletionCtx { kind, type_location, qualifier }
DotAccess { receiver_range, receiver_fact, access_kind, expected_type }
RecordFieldContext { owner_type, field_mode }
CallArgumentContext { callable, active_parameter }
PatternContext
StatementContext
expected_type
expected_name
visible_scope
```

Path contexts should distinguish expression paths, type paths, item paths,
module/import paths, pattern paths, and builtin type-argument positions.
Type-location data should distinguish local annotations, function return
hints, parameters, struct fields, enum fields, and nested builtin container
arguments. Dot access should be a first-class context even with an empty
prefix after `.`, and dynamic `Any` receivers should suppress member guesses
instead of falling back to global completions.

Member completion should flow through a unified `MemberCompletionIndex` built
from source facts, schema facts, stdlib/builtin facts, and source trait/impl
facts. Feature producers consume this index; they should not independently
scan broad text contexts or invent receiver facts. Completion rendering must
keep insertion text and display identity separate: labels stay short and
insertable, while module paths, owner paths, docs, effects, and provenance are
carried by detail, label details, documentation, or resolve payloads.

Formatting should follow the same boundary principle. Rust-analyzer delegates
Rust formatting to rustfmt; Vela needs a syntax-owned formatter that uses
lossless CST/AST layout facts instead of a token-only whitespace state machine.
In particular, builtin container type hints must share one compact layout rule
across local annotations, parameters, return hints, struct fields, enum fields,
and nested `Option`/`Result` arguments.

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
typed in-memory LSP fixtures for initialize, document sync, and requests
stdio and loopback-TCP smoke tests through the same main loop
synthetic scale tests that approach one million lines without full rebuilds per edit
full workspace cargo checks before milestone checkpoints
```
