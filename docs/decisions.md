# Decisions

This file is the active architecture decision index. Full pre-compaction
decision history lives in
[archive/decisions-full-2026-06-01.md](archive/decisions-full-2026-06-01.md).

## Standing Constraints

- Script-language generics are not supported.
- Function overloading by arity, type hint, or native signature is not
  supported.
- Scripts never receive real Rust `&mut T` references.
- Host mutation must go through `HostRef`, `HostPath`, `PathProxy`, and
  `PatchTx`.
- Reflection can query metadata and perform controlled reads, writes, and
  calls, but cannot mutate runtime type structure or implement monkey patching.
- The MVP does not include JIT, script async/coroutines, moving GC, or a full
  LSP.
- Pre-release code should replace obsolete internal APIs instead of preserving
  compatibility shims. Product-level hot reload ABI and schema compatibility
  checks remain required.

## Active Architecture Decisions

### Source And Artifact Naming

Vela source files use `.vela`. Future precompiled bytecode-only artifacts use
`.vbc`. If a future deployment package contains bytecode plus ABI manifests,
schema metadata, source maps, or reload metadata, it should use a separate
package extension rather than overloading `.vbc`.

### Module Imports And Exports

Public APIs should be imported from the module that owns them. Crate roots
should expose focused `pub mod` entries and avoid broad `pub use` facades unless
the item is an intentional crate identity entrypoint.

Rust source may use one direct-parent `super::...` reference inside a local
module group. Multi-level `super::super` paths are prohibited; cross-subsystem
imports should use explicit `crate::...` paths.

### Source Pipeline

The syntax layer owns tokens, AST, parser recovery, and source spans. HIR owns
module graph resolution, declaration IDs, binding maps, type-hint metadata, and
top-level semantic diagnostics. The bytecode compiler consumes HIR diagnostics
and metadata before bytecode emission.

There is no separate public IR crate yet. `HIR + TypeFacts + bytecode` is the
current semantic pipeline; a lower IR/MIR should only be introduced when
optimization, CFG/data-flow, register allocation, or lowering complexity
requires it.

### Function Identity

Vela does not support function overloading. A module has one function per
script-visible name, and a type or trait has one method per receiver/name pair.
Arity, type hints, default values, and native Rust signatures do not create
overload sets. Resolver, reflection, native registration, and hot-reload ABI
logic should model each function name as a single callable.

### Runtime And Heap

The VM is a register bytecode interpreter. Execution budgets cover
instructions, memory, call depth, and patches. Script heap values use stable,
generation-checked non-moving handles; host refs and path proxies remain
external handles and are not traced as Rust-owned state.

Managed heap entrypoints materialize return values at API boundaries. Native
calls materialize heap-backed values as needed so existing host/native APIs do
not own script GC state.

### Host Boundary

Host state is mutated only by recording patches. Direct host field, host path,
and host method bytecode routes through `HostExecution`, `ScriptStateAdapter`,
and `PatchTx`. RMW patches carry expected base values, overlays are read before
adapter state, and adapter mutation happens only at safe-point apply.

PathProxy wraps HostPath and requires PatchTx. Host values may represent
primitives, arrays, maps, records, enums, and HostRef handles, but not real Rust
references.

### Reflection

Reflection metadata is copied, permission-aware, and read-only with respect to
type structure. TypeRegistry descriptors are the source for reflected types,
fields, methods, traits, variants, modules, functions, source spans, docs,
attributes, effects, access, and required permissions.

Reflective reads, writes, and calls resolve descriptor metadata to stable IDs
and route host interaction through PatchTx. Private, effectful, host path, and
field-level operations require explicit reflection permissions.

### Hot Reload

Hot reload replaces function-level or module-level code objects at safe points.
Old ProgramVersion handles keep old code alive, rejected updates do not advance
versions, and reports carry copied diagnostics plus ABI details.

Function, method, module, trait, schema, effect, access, parameter, return, and
source-span metadata participate in ABI validation. Engine registries are the
source for host/native ABI manifests.

### Standard Library And Dynamic Types

Option and Result are dynamic enum-shaped values, not script generics. Stdlib
helpers and analysis TypeFacts may describe dynamic payloads, but the language
surface remains non-generic.

`null` is retained for no-value, void-like results, host nullable boundaries,
and missing metadata. Expected absence should use `Option.None`, recoverable
business failure should use `Result.Err`, and unrecoverable script/runtime
failures should use VM diagnostics rather than `Result.Err`.

Array, map, set, string, range, math, context, random, and gameplay helpers are
deterministic unless an Engine-installed permissioned native explicitly provides
controlled nondeterminism.

### Analysis And Tooling

TypeFacts, completions, hover, match exhaustiveness, effect diagnostics, null
narrowing, Option/Result predicate narrowing, and pattern diagnostics are
analysis/tooling data. They should not change VM semantics unless a separate
compiler/runtime decision says so.

### Debugger Support

Debugger support is a post-MVP runtime and Debug Adapter Protocol capability,
not a script-language feature. Runtime debug hooks may expose source
breakpoints, stepping, stack frames, watches, safe HostRef display, PatchTx
preview, and hot-reload breakpoint rebinding, but they must respect reflection,
host access, PatchTx, and TypeRegistry boundaries.

### Cranelift JIT

Cranelift JIT is a mandatory post-MVP backend after interpreter optimization,
inline caches, debugger contracts, and conformance are stable. JIT must remain
disableable, must be semantically equivalent to VM execution, and must preserve
ExecutionBudget, GC roots, PatchTx, reflection policy, hot reload invalidation,
and debugger-visible frame/source metadata.

## Validation Rules

- Multi-level `super` scan must return no matches:

```bash
rg -n '(super::){2,}|super\s*::\s*super' crates examples tests --glob '*.rs'
```

- Remaining `pub use` entries should be deliberate API surface:

```bash
rg -n '^\s*pub use\b' crates --glob '*.rs'
```

## Update Rules

- Add or update entries here when a change creates a durable architecture rule,
  compatibility policy, naming convention, module boundary, or semantic
  constraint.
- Do not record routine implementation steps, small refactors, or test-only
  details here.
- Keep active entries concise. Move detailed historical rationale to
  `docs/archive/` when this file stops being quick to scan.
