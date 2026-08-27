# Architecture

This document describes the technical architecture for a Hot Reload First
dynamic scripting language implemented in Rust for host-owned business logic.
Game server scripting is a primary application, but the core language, stdlib,
builtins, and runtime contract stay domain-neutral.

The core idea is:

```text
Scripts describe host-boundary business logic with natural syntax.
The VM represents mutations to the Rust world as HostAccess operations.
The runtime performs reliable function-level hot reload by replacing CodeObject mappings.
One sealed TypeBinding registry gives owned values, shared/exclusive views,
constructors, methods, indexes, iteration, and protocols a common Rust/Vela ABI.
Rust-facing hotfixes cross one generated service contract and publish one
complete immutable service generation; there is no callable-slot replacement
path.
```

## Reference Designs

These projects are useful references, but this language should not copy them directly.

| Project | Useful Ideas | Do Not Copy |
|---|---|---|
| Luau | High-quality interpreter, bytecode optimization, inline caches, game-logic performance focus | Lua syntax and table/metatable object model |
| Wren | Small embedded VM and restrained syntax | The Rust host access model needs custom design |
| Rhai | Rust embedding experience and small-language strategy | Expression power and hot reload are not enough for this goal |
| Rune | Rust-like dynamic language, VM, hot reload, Rust embedding | The host state HostAccess model is more specialized |
| Starlark | Determinism, restraint, and tool friendliness | It is not a direct fit for high-performance mutable host-boundary logic |
| Mun | Hot Reload First runtime ideas | Static typing and LLVM/AOT are different from this project |

References:

- Luau performance: https://luau.org/performance/
- Mun language: https://mun-lang.org/
- Mun GitHub: https://github.com/mun-lang/mun
- Codex goals: https://developers.openai.com/codex/use-cases/follow-goals
- Codex goal cookbook: https://developers.openai.com/cookbook/examples/codex/using_goals_in_codex
- Codex best practices: https://developers.openai.com/codex/learn/best-practices

## Compile And Runtime Pipeline

```text
Source Code
   ↓
Lexer / Parser
   ↓
CST / AST
   ↓
Resolver / Symbol Table / Semantic Model
   ↓
HIR / Lowered IR / TypeFacts
   ↓
Bytecode Compiler
   ↓
CodeObject / ProgramVersion
   ↓
VM Runtime / GC / Stack / CallFrame
   ↓
Host Bridge / Reflection / HostAccess
   ↓
Rust World / ECS / Actor State / Database Adapter
```

The source front door follows the same dependency direction as this pipeline.
`vela_syntax` owns parsing, `vela_hir` owns source-set ingestion and
`ModuleGraph` construction, and `vela_bytecode` accepts an already-built HIR
source set. Source-set ingestion fixes single-source versus module-graph mode;
function compilation accepts only a function selection bound to that same
source set. `vela_engine` orchestrates these layers for source, file, directory,
and hot-reload APIs. The bytecode compiler must not parse source text or depend
on `vela_syntax`.

`vela_engine` is the only production source orchestrator and the owner of
registry-aware linking. It passes a `LinkedArtifact` into `vela_hot_reload`;
the hot-reload crate owns version construction, ABI/policy comparison, and
update generation, but exposes no production source, HIR-graph, or
`CompiledProgram` compilation entrypoint. Reading immutable script metadata
carried by the linked artifact remains part of hot-reload ABI validation.
Source-ingestion, bytecode-compilation, and link failures remain structured
Engine errors and return immediately; only artifact ABI/policy outcomes enter
the staged hot-reload report path.

Package/project IO is owned by the dependency-light `vela_package` crate.
Engine and language tooling consume the same structured `vela.toml`, package
graph, deterministic source table, and `PackageId + ModulePath` identity.
Ordinary roots and optional provider selections extend one sealed package
compile request and enter the same HIR/compiler/linker pipeline. Only the linker
may seal selected provider metadata into `LinkedArtifact`.

## File Extensions

Vela source files use `.vela`.

Precompiled bytecode-only artifacts use `.vbc` when that cache/artifact format
is implemented. If a future deployment package needs bytecode plus ABI
manifest, schema metadata, source maps, and reload metadata, it should use a
separate package format rather than overloading `.vbc`.

## Suggested Workspace Structure

```text
vela/
  Cargo.toml
  crates/
    vela_common/          # Span, Symbol, IDs, diagnostics
    vela_syntax/          # Lexer, parser, lossless CST, AST
    vela_hir/             # Resolver, HIR, name binding
    vela_analysis/        # Semantic model, TypeFacts, completion data
    vela_language_service/ # Shared editor analysis service, no LSP protocol or platform IO
    vela_bytecode/        # Instruction, CodeObject, compiler
    vela_vm/              # Runtime, VM, Value, GC, call frames
    vela_c_api/           # C ABI opaque handles and external FFI surface
    vela_reflect/         # TypeRegistry, TypeDesc, reflection API
    vela_host/            # HostRef, HostTargetPlan, HostAccess, adapters, diagnostics
    vela_macros/          # #[derive(ScriptHost)] and related macros
    vela_std/             # Native standard library implementation
    vela_hot_reload/      # ProgramVersion, ABI diff, code swap
    vela_lsp_server/      # Native LSP server for the pre-MVP tooling track
    vela_cli/             # final CLI binary for direct script execution
  examples/               # standalone runnable embedding examples
  docs/
    architecture.md
    grammar.ebnf
    goal.md
    progress.md
    decisions.md
    blocked.md
    performance.md
    reflection.md
    hot_reload.md
    host_bridge.md
  tests/
    fixtures/
```

## Implementation Architecture Hygiene

The implementation should prefer clean architecture over compatibility with
old internal shapes. During pre-release development, obsolete internal APIs,
transitional behavior, and temporary artifacts should be replaced instead of
kept behind compatibility shims. This rule does not apply to product-level hot
reload ABI and schema compatibility checks, which remain part of the runtime
contract.

Code structure rules:

```text
keep ordinary source files under 1200 lines unless a clear exception is documented
review over-threshold active files and split them by responsibility when no exception exists
split large files by crate/module responsibility
use Rust `mod` boundaries for handwritten source; reserve `include!` for generated code
use standard Rust module file resolution in production; reserve `#[path]` for justified test or cross-target sharing
use explicit production imports; wildcard imports are allowed only in test code
split large functions when control flow stops being locally understandable
extract cohesive parameter structs when function signatures grow around one concept
replace accumulating conditional branches with match, enum-driven dispatch, tables, or focused helper types
move feature-specific policy out of generic execution loops when it starts to distort the loop
adjust architecture when a feature can only be added through awkward patch code
```

The 1200-line threshold applies to active implementation and test files.
Generated files, archived documents, and dense fixture data may exceed it when
splitting would reduce clarity, but those exceptions should be intentional.
The reviewed exception list is maintained in
[architecture/file-size-exceptions.md](architecture/file-size-exceptions.md);
an over-threshold file absent from that list is an audit failure.

Compatibility rules:

```text
do not add aliases, duplicate APIs, or migration paths only to preserve old internal callers
do not keep legacy behavior in parallel with new behavior unless a milestone explicitly requires both
update tests and examples to the current architecture instead of supporting old paths
document accepted product compatibility rules in hot reload, schema ABI, and artifact formats
```

## Critical Vertical Loop

The first phase should close this loop:

```text
Rust Host Type Metadata
        ↓
script dot-syntax access
        ↓
FieldId / MethodId compile-time resolution
        ↓
VM bytecode execution
        ↓
HostRef / PathProxy
        ↓
HostAccess validates and routes write-through host mutations
        ↓
Rust adapter state is updated immediately
        ↓
hot reload replaces function CodeObject values
```

## Rust/Vela Interop Contract

Vela-held scoped Host capabilities are explicit resources. The compiler never
inserts a release from last-use, lexical-scope, branch-edge, register-reuse, or
pre-await analysis. Authors close a live group with strict
`host::release(value)` or narrowly idempotent
`host::try_release(value) -> bool`; generated terminal Service transfer and
unconditional root teardown are the only non-authored closures. Children must
release before parents, aliases expire as one group, and every await rejects
the complete set of still-active scoped resources, including dead locals and
lazy Host iterators.

Service compiler capabilities use only static namespace paths:
`service::base::method(...)` calls the current method's registered Rust
default, and `service::pinned::service_name::method(...)` calls through the
root-pinned generation. Neither namespace path is a value. The deleted
`base.method(...)` and `services.service.method(...)` contextual spellings have
no aliases, leaving `base` and `services` available as ordinary local names.
Every admitted Service signature has complete direct-Rust, Vela-selected,
same-generation nested, typed Rust-base, async, and return-restoration paths;
unsupported signatures fail during macro expansion or schema sealing.

Non-`'static` Host parameters reach Rust defaults through generated root-local
typed thunks. One reviewed unsafe erased-reborrow boundary runs only after
stable type, root generation, alias, capability, and lease validation; real
Rust references never enter Vela values, reflection, GC state, persistent
state, or artifacts. Controlled HostAccess adapters remain a valid ordinary
Host representation: generated synchronous methods use their registered
adapter vtable only when the receiver itself cannot provide a typed lease,
without retrying other lease, permission, or invocation failures.

Portable program, Service bundle, and detached Service metadata format version
5 encode the explicit-release, scoped-task, static HostRef iteration, and
verified physical-plan contracts. Versions 1 through 4 are rejected at
decode/load boundaries before staging or activation; there is no legacy
interpreter or compatibility mode.

Host-scoped detached async execution extends this model without sharing a
Runtime or exposing an executor. `task::spawn_scoped` admits a statically linked
ordinary async function into an explicit host lifecycle scope;
`task::spawn_scoped_then` additionally requests a synchronous Vela
continuation at a later host safe point. Every child owns transferable values,
an isolated Runtime, finite budgets, and the originating linked artifact. A
Service-rooted child also pins the complete originating Service generation, so
its nested `service::base` and `service::pinned` calls cannot mix releases.
HostRef, PathProxy, scoped leases, closures, live iterators, and host contexts
cannot cross admission. Vela exposes no TaskHandle, Future value, join, script
cancellation, manual resume, unscoped spawn, or framework-specific task API.
Portable program, Service bundle, and detached deployment metadata now use
format version 5. It retains the M20.75 detached-task and static HostRef
collection-iteration contracts and adds canonical selected physical plans,
coverage, source points, exits, and profile layouts. Versions 1 through 4 are
rejected rather than upgraded or interpreted through a compatibility path.


## Detailed Contracts

The active architecture contract is split by responsibility. Read the relevant
contract before changing that subsystem:

- [Language semantics](architecture/language.md)
- [Primitive types, type hints, and guards](architecture/primitives-type-hints-and-guards.md)
- [Host bridge and registration](architecture/host-and-registration.md)
- [Reflection](architecture/reflection.md)
- [Runtime, bytecode, threading, and GC](architecture/runtime.md)
- [Dynamic method dispatch](architecture/dynamic_method_dispatch.md)
- [Hot reload](architecture/hot-reload.md)
- [Standard library and embedding](architecture/stdlib-and-embedding.md)
- [Tooling, performance, security, and testing](architecture/tooling-performance-security-testing.md)
- [Native LSP architecture](architecture/lsp.md)
- [Packages and service providers](packages-and-providers.md)
- [Rust/Vela unified service model](architecture/rust-vela-service-model.md)
- [Rust/Vela unified service hard-switch plan](rust-vela-service-hard-switch-plan.md)
- [Rust/Vela final interop and explicit-release hard switch](rust-vela-interop-final-shape-hard-switch-plan.md)
- [Rust/Vela interop hard-switch implementation](rust-vela-interop-hard-switch-implementation.md)
- [Rust/Vela interop authoring and deployment](rust-vela-interop.md)
- [Clean identity refactor](architecture/clean-identity-refactor.md)
- [Executor-neutral async execution plan](async-execution-model-plan.md)
- [Host-scoped detached async execution plan](host-scoped-detached-async-execution-plan.md)
- [State storage model execution plan](state-storage-model-plan.md)
- [Actor-owned Runtime and cache model execution plan](archive/actor-runtime-cache-execution-plan.md)

Keep this file as the entrypoint and cross-subsystem contract. Subsystem files
carry the detailed rules so active architecture docs remain reviewable and stay
under the ordinary 1200-line source-file threshold.
