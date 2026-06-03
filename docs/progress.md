# Progress

This file is the current implementation status. Detailed historical progress
before this compaction lives in
[archive/progress-full-2026-06-01.md](archive/progress-full-2026-06-01.md).

## Current Focus

M0-M13 are complete enough as a runnable prototype. Current work is centered on
the current checkpoint queue below: advance targeted M14/M15 Engine API and
hot-reload source workflow work as it unblocks embedding.

Post-MVP performance remains a separate track: optimize the non-JIT bytecode
interpreter toward Lua 5.x comparable gameplay workloads, then add debugger
runtime/DAP support and Cranelift JIT once the interpreter, inline-cache, and
conformance contracts are stable.

## Milestone Status

| Milestone | Status | Notes |
|---|---|---|
| M0-M6 | Complete | Source -> bytecode -> VM -> HostRef/HostPath/PatchTx -> hot reload loop exists. |
| M7 | Complete | Execution budgets, managed heap, GC roots, and managed heap entrypoints exist. |
| M8 | Complete enough | HIR, module graph, imports, declarations, binding maps, and compiler integration are active. |
| M9 | Complete enough | Broad executable language surface works; conformance catches edge cases. |
| M10 | Complete enough | Stable script metadata, shapes, slots, traits, and dispatch foundations exist. |
| M11 | Complete enough | HostRef, HostPath, PathProxy, PatchTx overlays, and rollback-safe host boundaries exist. |
| M12 | Complete enough | Reflection metadata, permission-aware queries, lookup budgets, candidate spans, and schema-safe mutation denial are covered. |
| M13 | Complete enough | Collections, strings, Option/Result propagation, math, context, random permissions, lambda facts, and demo helper coverage are validated. |
| M14 | Partial | Engine APIs, native descriptors, context helpers, and macros exist in slices. |
| M15 | Partial | Function, descriptor, module, trait, schema, and source reload ABI checks exist. |
| M16 | Partial | Runtime diagnostics, common rendering, and bytecode/runtime frame maps have started. |
| M17 | Partial | Conformance fixture and demo harnesses exist; game-server demo can still expand. |
| M18 | Partial | Baseline harnesses cover VM scalar, stdlib, host PatchTx, managed heap, GC pacing, hot reload, and available external runtime comparisons; official baselines remain. |
| M19-M20 | Not started | Interpreter optimization plus inline caches and specialization. |
| M21 | Not started | Debugger runtime hooks and DAP integration. |
| M22 | Not started | Cranelift JIT backend after interpreter/cache/debug contracts are stable. |
| M23 | Not started | Release hardening, public docs, validation gates, and performance targets. |

## Current Milestone Checkpoints

Use this queue to choose the next implementation task. Work on the first
checkpoint that is not satisfied, and update this section when a checkpoint
closes or exposes a more specific gap.

1. M14/M15 embedding and reload:
   - Advance only when it unblocks the demo or conformance workflow: Engine API
     registration, native descriptors, context helpers, macros, safe-point
     reload, ABI/schema/effect checks, or source-file update workflows.
   - Validation: targeted engine/hot-reload tests and CLI demo runs when
     workflow-facing.

## Active Capabilities

- Source files use `.vela`; future bytecode-only artifacts use `.vbc`.
- Static module, type, variant, and native-function paths use `::`; runtime
  field, method, host-path, and metadata-record access uses `.`.
- Parser covers declarations, statements, expressions, attributes, and recovery
  with source spans.
- HIR owns module graph resolution, imports, declaration IDs, binding maps,
  type-hint metadata, and top-level effect checks.
- Bytecode compiler consumes HIR diagnostics and emits register bytecode for
  functions, closures, control flow, collections, records, enums, slots,
  host paths, method dispatch, Option/Result-style propagation, and iteration.
- Bytecode code objects carry read-only frame metadata for named parameters,
  locals, loop bindings, match bindings, lambda parameters, and captures.
- VM supports managed heap execution, non-moving GC, execution budgets,
  script value methods, standard natives, reflection natives, and host-aware
  execution.
- Runtime error stack frames expose function names, call-site source spans,
  and caller bytecode offsets for debugger/tooling foundations.
- VM call frames can report register-to-GC-root mappings while preserving the
  existing flat root list used by collection.
- Shared diagnostic rendering expands multi-line source spans while preserving
  stable single-line and missing-source output.
- Host mutation goes through HostRef, HostPath, PathProxy, PatchTx, overlays,
  permissions, and safe-point apply.
- Reflection covers types, fields, methods, variants, traits, modules,
  functions, attributes, permissions, source spans, controlled reads/writes,
  and controlled calls.
- Engine API registers host types through `register_host_type::<T>()`, native
  functions, context helpers, standard natives, reflection permissions,
  compiler options, hot-reload policies, and a focused embedding prelude for
  common host setup imports.
- Engine validation rejects duplicate native callable names and stable IDs
  across pure, host, context-host, and standard native registrations.
- Engine API registers macro-derived host bindings through
  `register_script_host::<T>()`, combining `ScriptHost` schema metadata with
  generated host method registration for embedders that use both derives.
- Engine API registers derive-generated reflection schemas through
  `register_reflect_schema::<T>()`.
- `ScriptReflect` derives host enum variant metadata with stable variant and
  payload field IDs, and `register_reflect_schema::<T>()` can register those
  reflected enum schemas.
- Macro-generated context native registrations flow through Engine permission
  checks and `NativeCallContext` budget charging, including budget-aware
  PatchTx helpers for host path writes, read-modify-write operations, removals,
  pushes, and host method-call patches.
- Engine typed native Rust signature conversion covers copied `Vec<T>` array
  arguments and returns, fixed arrays, maps, sets, Option/Result, strings, and
  common numeric types.
- The game-server demo registers Player, Monster, Inventory, ItemStack, and
  Config host schemas through `ScriptHost` derives and
  `register_host_type::<T>()`, and registers HostQuestProgress variant
  metadata through `ScriptReflect` plus `register_reflect_schema::<T>()`, while
  preserving reflected host trait and method metadata.
- Hot reload validates function, method, module, trait, schema, effect, access,
  stable-ID schema rename compatibility, and source diagnostics before version
  advancement.
- Hot reload reports distinguish actual bytecode-changed functions from
  source-changed modules and reverse-import impacted modules.
- Engine and Runtime hot-reload source workflows accept changed `.vela` file
  events inside a module root while recompiling the full root for import and
  ABI correctness.
- Runtime source-file, directory, and changed-file reload staging keeps source
  path/load errors immediate while deferring accepted updates and ABI/policy
  rejections to the next explicit safe-point report.
- Runtime staged directory and changed-file reload acceptance reports preserve
  changed-module and reverse-import impacted-module metadata at the safe point.
- Runtime source-file, directory, and changed-file reload staging accept
  default-policy private helper additions at safe points while keeping old
  calls on the previous version until the report is consumed.
- Runtime source-file, directory, and changed-file reload staging accept
  default-policy public function additions at safe points and make the new
  public entry callable only after the accepted report is consumed.
- Runtime source-file reload staging reports removed script function
  rejections at safe points without advancing the active version.
- Runtime source-file reload staging accepts compatible defaulted script schema
  additions at safe points without activating the new version early.
- Runtime source-file reload staging accepts compatible defaulted script enum
  variant field additions at safe points without activating the new version
  early.
- Runtime directory and changed-file reload staging accept compatible defaulted
  script enum variant field additions at safe points without activating the new
  version early.
- Runtime directory and changed-file reload staging accept compatible defaulted
  script schema additions at safe points without activating the new version
  early.
- Runtime source-file, directory, and changed-file reload staging accept
  stable-ID script schema field and variant renames at safe points without
  activating the new version early.
- Runtime source-file, directory, and changed-file reload staging report event
  handler parameter ABI rejections at safe points without advancing the active
  version.
- Runtime source-file, directory, and changed-file reload staging report event
  target ABI rejections at safe points without advancing the active version.
- Runtime source-file, directory, and changed-file reload staging report
  function return ABI rejections with repair hints at safe points without
  advancing the active version.
- Runtime source-file, directory, and changed-file reload staging report
  required function parameter additions with repair hints at safe points
  without advancing the active version.
- Runtime source-file, directory, and changed-file reload staging report native
  descriptor effect, access, parameter, return, and removal ABI rejections at
  safe points without advancing the active version, with repair hints covered
  for effect, access, parameter, return, and removal ABI reports.
- Runtime source-file, directory, and changed-file reload staging report host
  method descriptor effect, access, parameter, return, and removal ABI
  rejections at safe points without advancing the active version, with repair
  hints covered for effect, access, parameter, return, and removal ABI reports.
- Runtime source-file, directory, and changed-file reload staging report
  required script enum variant field ABI rejections at safe points without
  advancing the active version, with repair hints covered.
- Runtime source-file, directory, and changed-file reload staging report
  required script struct field ABI rejections at safe points without advancing
  the active version, with repair hints covered.
- Runtime source-file, directory, and changed-file reload staging report script
  struct field type ABI rejections at safe points without advancing the active
  version, with repair hints covered.
- Runtime source-file, directory, and changed-file reload staging report removed
  script schema ABI rejections at safe points without advancing the active
  version, with repair hints covered.
- Runtime source-file, directory, and changed-file reload staging report script
  enum variant field type ABI rejections at safe points without advancing the
  active version, with repair hints covered.
- Runtime source-file, directory, and changed-file reload staging report removed
  script trait implementation ABI rejections at safe points without advancing
  the active version, with repair hints covered.
- Runtime source-file, directory, and changed-file reload staging accept added
  script trait implementations at safe points without activating the new
  version early.
- Runtime source-file, directory, and changed-file reload staging report script
  trait method return ABI rejections at safe points without advancing the
  active version, with repair hints covered.
- Runtime source-file, directory, and changed-file reload staging report added
  required script trait method ABI rejections at safe points without advancing
  the active version, with repair hints covered.
- Runtime source-file, directory, and changed-file reload staging accept added
  defaulted script trait methods at safe points without activating the new
  version early.
- Runtime source-file, directory, and changed-file reload staging report removed
  script trait ABI rejections at safe points without advancing the active
  version, with repair hints covered.
- Runtime safe-point reload staging reports external module export ABI
  rejections without advancing the active version, with repair hints covered.
- Runtime safe-point reload staging reports removed external function ABI
  rejections without advancing the active version, with repair hints covered.
- Runtime safe-point reload staging reports removed external method ABI
  rejections without advancing the active version, with repair hints covered.
- Runtime safe-point reload staging reports removed external module ABI
  rejections without advancing the active version, with repair hints covered.
- Runtime source-file, directory, and changed-file reload staging report
  top-level const side-effect compile rejections at safe points without
  advancing the active version.
- Runtime directory reload staging reports compile diagnostics at safe points
  without advancing the active version.
- Runtime source-file, directory, and changed-file reload staging report script
  function access ABI rejections at safe points without advancing the active
  version.
- Runtime directory and changed-file reload staging reports removed script
  function rejections at safe points without advancing the active version.
- Runtime changed-file reload staging reports compile diagnostics at safe
  points without advancing the active version.
- Analysis diagnostics can report non-exhaustive matches for known script
  enums and dynamic Option/Result facts used by propagation-style control flow.
- Analysis diagnostics can use TypeRegistry field access metadata to flag
  known read-only host field assignment targets with script-author write hints.
- Unknown host-field diagnostics include ranked candidate labels with copied
  read/write access hints for likely field names.
- Unknown host-method diagnostics include ranked candidate labels with copied
  method access, effect, and permission hints for likely method names.
- Reflection field, method, and function access-denial diagnostics carry copied
  declaration source spans when schema metadata provides them.
- Core reflection call policy enforces `reflect::call_methods` for direct
  method calls and reflected function invocation, before effect-specific call
  permissions are considered.
- Script-defined struct and enum fields expose writable reflection metadata,
  and copy-returning `reflect::set` respects `reflect_writable` plus field
  permissions for script values.
- Reflection metadata records are read-only at the `reflect::set` boundary, so
  copied descriptors cannot be rewritten into schema-mutation stand-ins.
- Global `reflect::fields()` metadata includes enum variant payload fields with
  policy filtering and `Type::Variant` ownership.
- Standard Option/Result enum variants and payload fields expose copied docs
  and stdlib attrs through direct registry metadata and script reflection.
- Standard Context host schema metadata tags its type, time fields, and
  event/log methods for stdlib and gameplay-domain reflection queries.
- Standard library runtime and analysis coverage spans arrays, maps, sets,
  strings, Option/Result helpers and propagation, math, context time/event/log
  helpers, controlled random permissions, lambda TypeFacts, and gameplay demo
  helper scripts.
- Hot reload updates can be staged during gameplay and consumed only by an
  explicit runtime safe-point check.
- Runtime event calls can consume staged hot-reload updates or rejections at
  an explicit event-end safe point while the completed event still runs on the
  previous active version.
- Runtime tick-boundary safe points can consume staged hot-reload updates or
  rejections without running script code, matching production tick-loop
  workflows.
- Engine runtimes can bracket `PatchTx` apply with before/after hot-reload
  safe-point checks.
- Macro-exposed host types, fields, methods, and native functions derive stable
  `u64` IDs from script-facing paths, with `alias` preserving compatibility
  across rename-safe host API changes.
- Macro-exposed native functions can set public, reflection visibility, and
  reflective callability metadata while preserving permissioned Engine
  registration and hot-reload ABI visibility.
- CLI demo scripts and conformance fixtures use `.vela`, and the hot-reload
  demo exercises staged updates through an explicit tick-boundary safe point.

## Current Gaps

- Continue hardening M14/M15 embedding and production safe-point reload
  workflows.
- Expand M16/M17 diagnostics, fixtures, and game-server demo coverage.
- Keep M18+ performance work benchmark-driven and separate from semantic
  changes.
- Plan M21 debugger and M22 Cranelift JIT from stable source-span, frame-map,
  GC-root, budget, PatchTx, hot-reload, and conformance contracts.

## Update Rules

- Update this file when milestone status, current focus, active capability
  coverage, or major gaps change.
- Do not append every small implementation detail here; that belongs in commit
  history or the relevant module tests.
- Move long historical sections into `docs/archive/` when this file stops being
  quick to scan.
