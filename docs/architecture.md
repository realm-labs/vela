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
    vela_bytecode/        # Instruction, CodeObject, compiler
    vela_vm/              # Runtime, VM, Value, GC, call frames
    vela_c_api/           # C ABI opaque handles and external FFI surface
    vela_reflect/         # TypeRegistry, TypeDesc, reflection API
    vela_host/            # HostRef, HostTargetPlan, HostAccess, adapters, diagnostics
    vela_macros/          # #[derive(ScriptHost)] and related macros
    vela_std/             # Native standard library implementation
    vela_hot_reload/      # ProgramVersion, ABI diff, code swap
    vela_lsp/             # Future language server, not part of MVP
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
split large functions when control flow stops being locally understandable
extract cohesive parameter structs when function signatures grow around one concept
replace accumulating conditional branches with match, enum-driven dispatch, tables, or focused helper types
move feature-specific policy out of generic execution loops when it starts to distort the loop
adjust architecture when a feature can only be added through awkward patch code
```

The 1200-line threshold applies to active implementation and test files.
Generated files, archived documents, and dense fixture data may exceed it when
splitting would reduce clarity, but those exceptions should be intentional.

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
- [Clean identity refactor](architecture/clean-identity-refactor.md)

Keep this file as the entrypoint and cross-subsystem contract. Subsystem files
carry the detailed rules so active architecture docs remain reviewable and stay
under the ordinary 1200-line source-file threshold.
