# Progress

This file is the rolling implementation status for the current milestone. It
records what is true now and what remains to close next; it is not a changelog.

Detailed historical progress before the 2026-06-01 compaction lives in
[archive/progress-full-2026-06-01.md](archive/progress-full-2026-06-01.md).
Later history should be read from git unless a durable milestone summary needs
to be archived.

## Breaking Clean Architecture Track

The active clean-architecture refactor is a breaking internal architecture
track. Old handwritten stdlib IDs, raw `0xff00_...` identity spaces, old
bytecode shapes, old serialized `ProgramImage` assumptions, internal/public
APIs kept only for the old implementation shape, runtime string fallback
dispatch, and old internal `int`/`float` compatibility are not compatibility
requirements. The primitive scalar, bytes, type-hint contract, and guard-plan
checklist in
[archive/vela_primitives_type_hints_guards_plan.md](archive/vela_primitives_type_hints_guards_plan.md)
is complete and validated through the default full workspace checks.
The prior definition-registry and linked-bytecode checklist is complete and
validated through the default full workspace checks; follow-on work should
advance M20 cache/specialization prep rather than restoring old compatibility
paths.

This does not weaken product contracts: hot reload ABI/schema compatibility,
HostAccess safety, reflection permissioning, execution budgets, GC roots,
source-spanned diagnostics, and the no-Rust-`&mut` script boundary remain
required.

## Current Focus

M0-M19 are complete enough as a runnable prototype, embedding surface,
production hot-reload workflow, diagnostics/tooling foundation, runnable
embedding/conformance proof, measured performance baselines, and non-JIT
interpreter/heap optimization checkpoint. The primitive scalar, bytes,
type-hint contract, and guard-plan refactor is complete as a breaking M19.5
architecture continuation. M20 inline-cache work is now in close-out mode, not
open-ended cache expansion. Declared global reads, script record fields,
host access, native calls, linked method dispatch, broad stdlib value methods,
callbacks, string/bytes, Option/Result, and selected map/set/array targets have
guarded cache entries or explicit benchmark rows. Iterator/Sequence benchmark
rows now cover string chars/bytes, lazy array pipelines, iterator
short-circuit terminals, map views, range loops, and native-returned host
iterables. Remaining M20 work should
start from a cache-family audit and then do exactly one of these:

```text
close a named cache-family gap with hit, miss, guard, fallback, and invalidation tests
interpret a measured cache delta and record whether to keep, investigate, or defer it
defer a remaining cost to M21/M22/JIT/value-layout work with an explicit reason
```

The builtin parameterized container type-hint slice is in progress as an M20
type-contract continuation. Syntax, HIR, analysis TypeFacts, compiler
RuntimeTypeFacts, recursive guard plans, VM deep checks for materialized
array/map/set values, value-keyed map key guard scans,
compiler-owned typed container mutator checks, embedding
metadata display/validation, macro-inferred `Vec`/array/set/map hints,
including value-keyed Rust map/set inference such as `Map<i64, String>`,
value-keyed map/set runtime containers, detached `OwnedValue` map entries with
non-string serde key preservation, key-preserving reflection map reads,
hot-reload ABI structured type-hint comparison, and
execution-budget charging for deep guard scans are implemented. `Array.group_by`
now materializes value-keyed maps from callback keys instead of requiring
string keys, and iterator `collect_map` analysis now exposes erased value-keyed
`Map<Any, Any>` results rather than assuming string keys. Set add/remove
mutation paths now route through the `ValueKey`-indexed container entry instead
of scanning stored values, and `set::from_array` now lowers to a heap-aware
runtime constructor so identity key elements do not pass through detached
`OwnedValue` scalar filtering. Set relation and combination methods now use
`ScriptSet` key lookups/accumulators instead of temporary vector key scans, and
set higher-order/iterator `collect_set` materializers now accumulate directly
into `ScriptSet`. Cached and uncached `set.extend` paths now delegate
deduplication and insertion to the shared `ScriptSet` mutation boundary, and
the old `SetKey` aliases have been removed from set methods and cached
mutators.
Non-erased
`Iterator<T>` contracts now mark
iterator cursors with lazy item guards so checked boundaries do not consume
items, and yielded mismatches fail at `next()`/iteration time. Mutation-focused
benchmark/profile rows now cover proven typed, guarded erased-value, and
erased-container array/map updates, and external comparison rows cover string,
i64, and record-identity map/set lookup and mutation workloads. Heap-owned
container summaries and contract stamps now let stable array/map/set contracts
use O(1) summary/stamp checks
before falling back to budget-charged scans, and nested stamps are invalidated
when child containers mutate through aliases. Mixed map extensions update key
summaries for newly inserted keys even when the same batch also replaces
existing values. The object equality/order slice now has explicit runtime
equality semantics: ordinary `==`/`!=` no longer materialize detached
`OwnedValue` graphs for implicit structural comparison, `===`/`!==` compare
script-object and `HostRef` identity, manual
`impl PartialEq for Type { fn eq(...) -> bool }` drives record/enum semantic
equality, `#[derive(PartialEq)]` drives field-wise record equality without
`OwnedValue` materialization, array lookup/distinct helpers use `ValueKey`
container equivalence rather than user comparison traits,
manual `impl PartialOrd for Type { fn partial_cmp(...) -> Option<i64> }`
drives record/enum ordering operators, `#[derive(PartialOrd)]` drives
field-wise record ordering operators, manual
`impl Ord for Type { fn cmp(...) -> i64 }` and `#[derive(Ord)]` drive array
sorting and extrema helpers, statically known record/enum `==`/`!=`/ordering
operators now reject missing `PartialEq` or `PartialOrd` during compilation,
Map/Set/Array container lookup and dedup remain separate from user comparison
traits, and
array sorting rejects float keys until an explicit total-float ordering API
exists. `Eq` and `Ord` impl declarations now validate their required
comparison-trait prerequisites, and comparison derives now validate their
required trait chain plus unsupported fields such as float fields under
`Eq`/`Ord`. Statically known array `sort`, `sort_by`, `min`, and `max` calls
now reject non-`Ord` elements or keys at compile time when the compiler can
prove the element or callback key type, including record values and floats.
The object equality/order semantics slice is complete enough for the current
M20 checkpoint.

Post-MVP performance remains a separate track: measure first, then optimize the
non-JIT bytecode interpreter toward Lua 5.x comparable host-boundary workloads
through M19.5 architecture prep and M20 cache work. A full native LSP
capability track is now planned before the MVP and may proceed in parallel with
M19/M20 optimization. It remains analysis-only: no script or host execution, no
runtime semantic changes, and no custom IDE product. Debugger/DAP and Cranelift
JIT remain separate roadmap tracks.

## Milestone Snapshot

| Milestone | Status | Current note |
|---|---|---|
| M0-M6 | Complete | Source -> bytecode -> VM -> HostRef/HostPath/HostAccess -> hot reload loop exists. |
| M7 | Complete | Execution budgets, managed heap, GC roots, and managed heap entrypoints exist. |
| M8 | Complete enough | HIR, module graph, imports, declarations, binding maps, and compiler integration are active. |
| M9 | Complete enough | Broad executable language surface works; conformance catches edge cases. |
| M10 | Complete enough | Stable script metadata, shapes, slots, traits, and dispatch foundations exist. |
| M11 | Complete enough | HostRef, HostPath, PathProxy, and write-through HostAccess host boundaries exist. |
| M12 | Complete enough | Reflection metadata, permission-aware queries, candidate spans, and schema-safe mutation denial are covered. |
| M13 | Complete enough | Collections, strings, Option/Result propagation, math, context, random capability gating, lambda facts, and domain-neutral helper coverage are validated. |
| M14 | Complete enough | EngineBuilder registration, source compilation, Runtime::call, descriptors, stable-ID rejection, capability profiles, signature conversion, and macro parity are covered. |
| M15 | Complete enough | Safe-point staging, old-frame lifetime, new-call entry, source workflows, ABI/schema rejection, compatible additions, and repair reports are covered. |
| M16 | Complete enough | Parser, semantic, runtime/call-stack, host, reflection, hot reload, TypeFact, flow-narrowing, and completion snapshot fixtures exist. |
| M17 | Complete enough | Game-server demos, negative workflows, conformance fixtures, and parser fuzz harness exist. |
| M18 | Complete enough | Quick and full/default baseline captures exist with environment metadata and checksums. |
| M19 | Complete enough | Non-JIT interpreter and heap optimization has a recorded exit checkpoint. Accepted work includes GC pacing, direct heap aggregate construction, argument materialization/storage cleanup, borrowed receiver/runtime views, stdlib collection/string/Option/Result fast paths, scalar/equality/constant/peephole/range-loop lowering, small script-field and short-array construction, and expanded benchmark coverage. Remaining Lua 5.x deltas are measured and belong to M20 cache/specialization families rather than more unguarded M19 micro-optimization. |
| M19.5 | Complete enough | Primitive scalar, bytes, type-hint contract, guard-plan, verified-bytecode, profile ownership, HostTargetPlan/HostAccess, and linked-dispatch prep are complete and fully validated. |
| M20 | Active | Declared global, record field, host access, native call, resolved method dispatch, dynamic method dispatch, stdlib value-method, callback, string/bytes, Option/Result, and selected collection caches exist with benchmark coverage; active work is cache-family audit, measured delta interpretation, and closing only named remaining gaps. |
| M20.5 | Active follow-up | Protocol plumbing and baseline native language-service/LSP capabilities are validated enough for pre-MVP analysis-only tooling alongside M19/M20, and the rust-analyzer-aligned Phase 19 authoring correction slice is now validated for structured completion analysis, unified member completion, short labels with separate projection fields, statement snippets, native LSP fixtures, and compact type-hint formatting. Phase 1 workspace core and Phase 2 project/source loading now exist. Phase 3 now has source/project/parse/HIR/analysis database ownership, content hashes, declaration/import fingerprints, reverse-dependency invalidation, changed-file reparsing, stale-generation and cancellation result rejection, open-file-first scheduling, initial indexing metrics with larger synthetic workspace coverage, body-only edit avoidance of full HIR graph rebuilds, and open-file diagnostics priority while workspace work remains pending. Phase 4 diagnostics is complete enough with parser, HIR, initial analysis, and missing-schema diagnostics; open-document and workspace diagnostic batches; complete/partial/stale status; syntax-error isolation across unaffected modules; missing-schema degradation for schema-owned receivers; and structured candidate/repair-hint preservation. Phase 5 now has the native `vela_lsp_server` crate, lifecycle JSON-RPC handling, conservative initialize capabilities, full and incremental `didOpen`/`didChange` document sync, `didClose` overlay removal with disk-snapshot diagnostic restoration or scratch diagnostic clearing, `$/cancelRequest` rejection for stale queued requests, open-file diagnostic publication, initialized and changed workspace roots feeding workspace-mode diagnostics, and watched-file ingestion for `vela.toml` plus disk `.vela` source snapshots. File create/change/delete/rename events update disk snapshots, rebuild module paths, republish open diagnostics for removed imports, publish or clear `vela.toml` configuration diagnostics, and coalesce duplicate watcher events by final URI state within each batch. Module-path-only project reindexing invalidates HIR without reparsing unchanged files. Workspace-scale diagnostic batches now emit `$/progress` work-done begin/end notifications around open-file diagnostic publication. Phase 6 now has a versioned JSON schema artifact shape that round-trips `RegistryFacts`, exports module facts with docs and source spans, validates schema version/hash compatibility metadata, retains optional schema source-span locations, hydrates `SchemaDb`, loads configured schema files through the LSP server, watches schema artifact changes, keeps syntax diagnostics available when schema files are missing, updates host-member completion after schema reloads, and publishes or clears stale/invalid/missing schema diagnostics without running host code. Phase 7 now has an editor-neutral completion query with cursor-context extraction for global, module-path, member, record-constructor field, map-key, and named-argument contexts, plus overlay-backed declaration completions, schema-backed global completions, schema-backed host member completions for typed receivers, source/schema record-field completions for known constructors, typed map-key enum variant completions for source and schema facts, and source-backed script function named-argument completions with defaulted-parameter detail; the LSP server advertises and serves `textDocument/completion` from that query. Initial signature help for script and schema function calls now tracks active parameters and is served through `textDocument/signatureHelp`. Phase 8 now has an initial editor-neutral hover query for script parameters, declarations, schema-backed host members, and missing-schema type-hint degradation, with `textDocument/hover` served by the native LSP server. Initial go-to-definition now resolves local bindings, imported script declarations, and schema type/trait/function source spans through `textDocument/definition`. Phase 9 now has document symbols for script declarations and nested type/impl members through `textDocument/documentSymbol`, workspace symbols for module-qualified script declarations and schema facts through `workspace/symbol`, folding ranges for imports, declarations, functions, blocks, match arms, and multiline literals through `textDocument/foldingRange`, and syntax ancestry selection ranges through `textDocument/selectionRange`. Phase 10 has tokenizer-backed lexical semantic tokens, trivia-backed comment tokens, resolved script declaration/function/parameter/variable/type classifications, script-owned member declaration classifications, script/schema/stdlib member-use classifications plus schema/stdlib function-call and schema/builtin type-hint classifications with host and builtin modifiers, and deterministic full-token result IDs plus full-replacement delta responses served through `textDocument/semanticTokens/full` and `textDocument/semanticTokens/full/delta`. Phase 11 has local binding, source-owned script struct field read/write plus explicit and shorthand record-constructor field labels, source-owned enum variant constructor/pattern and record-variant field declaration/constructor/pattern, source-owned trait impl use, schema-backed field read/write plus explicit and shorthand schema record-constructor field labels, schema-backed enum record-variant field declaration/constructor/pattern, schema-backed method and trait-method call, schema-backed variant constructor/pattern, and imported script function references through `textDocument/references`, local read/write, enum-pattern, schema-variant pattern, and statically resolved script function call reference classifications, same-document highlights including schema-backed method and trait-method calls through `textDocument/documentHighlight`, initial source-backed script function call hierarchy for statically resolved calls, source-owned inherent, trait impl, and trait default/interface method call hierarchy for typed receiver calls, plus schema-backed method and trait-method call hierarchy for typed receiver calls. Phase 12 now has local binding `prepareRename`/`rename`, private value declaration/use workspace edits, private type declaration/type-hint workspace edits, private source-owned struct field declaration/member-use workspace edits, private source-owned inherent method declaration/member-call workspace edits, private source-owned enum variant declaration/constructor/pattern workspace edits, source-backed schema type/type-hint, function/call-site, variant declaration/constructor/pattern, and field/method declaration/member-use workspace edits, script function declaration/import/call-site workspace edits through `textDocument/prepareRename` and `textDocument/rename`, keyword/literal rejection, schema-only host rename rejection, local plus same-module declaration, struct-field, inherent-method, enum-variant, schema-variant, and schema-member collision rejection, public script function hot-reload ABI and source-backed schema ABI rename risk metadata through service edits and LSP change annotations, and versioned rename `documentChanges` for stale-edit protection. Phase 13 now has an editor-neutral code action model plus native `textDocument/codeAction` quick fixes for typo candidates, missing imports, unused-import removal, non-exhaustive match arms, and script-owned missing record constructor fields from structured diagnostics, plus conservative ambiguous/dynamic rejection and overlay range-stability coverage. Phase 14 now has stable syntax token/trivia extraction, an editor-neutral formatting IR that preserves raw comments, shebang trivia, spans, and blank-line groups, token-driven full-document formatting for expression/operator spacing, brace indentation, comment preservation, declaration/member layout, and final-newline insertion, plus native `textDocument/formatting`, conservative `textDocument/rangeFormatting` for trailing whitespace, whole top-level item selections including whitespace-padded selections around one item, indentation-aware selected impl/trait methods, nested member groups, completed top-level item and impl/trait method on-type reflow, and completed enum record-variant on-type reflow. Phase 15 now has script/schema function and typed source/schema method parameter-name inlay hints, stable inferred `let` local type hints, stdlib collection/iterator lambda parameter hints, schema-backed host-path type hints, and tuple-variant constructor payload-name hints served through native `textDocument/inlayHint`, with explicit annotation plus local/lambda/host-path/parameter/variant-payload `Any`/`unknown` suppression. Phase 17 now has a native stdio transport, launcher support for default stdio, `--stdio`, `--version`, `--root`, and `--schema`, editor initialization/configuration fallback, manual setup documentation for generic LSP clients, the Zed package now includes a generated `tree-sitter-vela` grammar plus highlight/indent/outline queries validated against all checked-in `.vela` fixtures, and editor packages remain thin launchers around native analysis; remaining broader method/schema call-site classification and broader dynamic-boundary hint suppression for future hint families remain open. |
| M21 | Not started | Debugger runtime hooks and DAP integration follow stable runtime/tooling contracts. |
| M22 | Not started | Cranelift JIT follows interpreter/cache/debugger/conformance stability. |
| M23 | Not started | Release hardening, public docs, validation gates, and performance targets. |

M20.5 Phase 12 update: rename is complete enough for the current LSP track.
The plan checklist is closed with service and native LSP fixtures for local,
private declaration, public import-aware, source-backed schema, collision,
hot-reload ABI, schema ABI, and versioned-edit behavior.

M20.5 Phase 12 update: service and native LSP rename now cover source-owned
trait default method calls where the receiver is produced by a source function
return, updating the trait declaration and returned-receiver call sites.

M20.5 Phase 17 update: native LSP workspace-folder coverage now includes
removed-root degradation, proving stale disk module facts are cleared while
open overlays remain authoritative for republished diagnostics.

M20.5 lifecycle update: repeated `initialize` requests now return a stable
invalid-request error before any workspace roots, editor configuration, or
capability state can be reset.

M20.5 lifecycle update: `shutdown` now requires successful initialization;
early shutdown requests return server-not-initialized without closing the
server or blocking a later valid `initialize`.

M20.5 lifecycle update: unsupported providers now have broader protocol
coverage: unadvertised request methods return method-not-found, unsupported
notifications are no-response no-ops, and later supported requests still work.

M20.5 lifecycle update: malformed `initialize` params now return a stable
invalid-request error without marking the server initialized, leaving a later
valid `initialize` request free to complete normally.

M20.5 lifecycle update: notification-shaped `initialize` messages are now
covered as no-response no-ops that leave the server uninitialized and do not
poison a later valid `initialize` request.

M20.5 lifecycle update: native LSP capability fixtures now pin
`textDocumentSync.save = false`, and `textDocument/didSave` notifications are
no-response no-ops so editor correctness does not depend on save events.

M20.5 lifecycle update: repeated `initialized` notifications are now stable
no-ops after the first dynamic watched-file registration, avoiding duplicate
`client/registerCapability` requests.

M20.5 lifecycle update: native LSP shutdown handling now rejects subsequent
requests with a stable invalid-request error while still allowing the final
`exit` notification.

M20.5 lifecycle update: notification-shaped `shutdown` messages are now
covered as no-response no-ops that do not close the server or block a later
valid `shutdown` request.

M20.5 lifecycle update: native LSP feature requests before `initialize` now
return a stable server-not-initialized error, and an early `initialized`
notification alone does not unlock request handling.

M20.5 lifecycle update: native LSP cancellation fixtures now cover stale
queued requests plus unknown and already-completed request IDs as no-response
no-ops that do not poison later requests.

M20.5 lifecycle update: native LSP `exit` now terminates the in-memory
dispatcher contract as well as process intent; later requests, notifications,
and malformed input are ignored with no responses.

M20.5 lifecycle update: request-shaped `exit` messages now have explicit
coverage: they return invalid-request while still ending the in-memory
dispatcher so later input is ignored.

M20.5 Phase 11 update: references and call hierarchy are complete enough for
the current LSP track. The plan checklist is closed with service and native
LSP fixtures for reference indexing, reference kinds, `textDocument/references`,
`textDocument/documentHighlight`, and statically resolved call hierarchy.

M20.5 Phase 10 update: semantic tokens are complete enough for the current LSP
track. The plan checklist is closed with service and native LSP fixtures for
resolved modifiers, script/schema/builtin token classes, full tokens, range
tokens, parser-recovery degradation, client fallback projection, and delta
responses.

M20.5 Phase 8 update: hover, definition, declaration, and type-definition are
complete enough for the current LSP track. The plan checklist is closed with
analysis, service, and native LSP fixtures for source, schema, stdlib, missing
schema, dynamic/unresolved, cross-file, and source-span-backed navigation
behavior.

M20.5 schema hover update: schema artifacts now carry optional docs metadata
for exported facts, and schema-backed hovers surface docs for types, fields,
variants, methods, trait methods, and functions.

M20.5 Phase 8 update: native LSP hover fixtures now cover schema method
effects and required permissions loaded from the static schema artifact,
matching the language-service metadata surface.

M20.5 Phase 8 update: native LSP hover fixtures now cover missing-schema
type-hint degradation, matching the language service's `Any` fallback without
running host code.

M20.5 Phase 8 update: service and native LSP hover fixtures now cover
schema-backed methods and trait methods where the receiver is produced by
another schema method return, matching chained schema receiver facts used by
completion, signature help, references, call hierarchy, semantic tokens, and
inlay hints.

M20.5 Phase 8 update: service and native LSP definition/declaration fixtures
now follow source spans for schema-backed methods and trait methods where the
receiver is produced by another schema method return, matching the chained
schema hover path.

M20.5 Phase 8 update: service and native LSP hover fixtures now cover
source-owned inherent methods on source function-return and source
method-return receivers, plus source trait default methods on source
method-return receivers.

M20.5 Phase 8 update: service and native LSP definition/declaration fixtures
now follow source-owned inherent methods on source function-return and source
method-return receivers, plus source trait default methods on source
method-return receivers. Inherent source-method navigation now anchors on the
method name instead of the lowered method body span.

M20.5 Phase 8 update: service and native LSP type-definition fixtures now pin
dynamic receiver-member boundaries, returning null for `Any` member targets
instead of fabricating a type location.

M20.5 clean LSP architecture Phase 6 update: references now have a
`reference_query()` result model that distinguishes source-owned, schema-owned,
builtin, dynamic `Any`, and unresolved targets; source/local reference identity
is pinned by a shadowing fixture; prepare-rename rejection covers schema-owned,
builtin, dynamic, unresolved, and ambiguous targets; rename results now route
through checked `EditPlan`s; and semantic-token recovery fixtures cover stable
HIR-backed classifications under parser recovery.

M20.5 Phase 12 update: native LSP rename fixtures now cover same-module
declaration collision rejection, matching the language-service module
declaration collision guard.

M20.5 Phase 12 update: service and native LSP rename fixtures now reject
imported declaration renames that would collide with an existing import alias
or import binding in an importing module.

M20.5 Phase 15 update: parameter-name inlay hints now cover typed source and
schema method calls in addition to script/schema function calls. The hint
suppression path treats dynamic or unknown source and schema function/method
parameters the same way as variant payload parameters, while keeping lambda
callback arguments and explicit annotations quiet.

M20.5 Phase 15 update: native LSP inlay fixtures now cover missing-schema
degradation, preserving the language-service behavior that unstable host facts
do not surface as `Any` hints.

M20.5 Phase 15 update: service and native LSP inlay fixtures now cover
schema-backed tuple-variant payload hints crossing dynamic `Any` facts,
matching source-owned variant payload suppression.

M20.5 Phase 15 update: service and native LSP inlay fixtures now cover
schema-backed method calls where the receiver is produced by a schema function
return, preserving `Any` parameter suppression through shared expression
receiver facts.

M20.5 Phase 15 update: service and native LSP inlay fixtures now cover
schema-backed trait method calls where the receiver is produced by a schema
function return, preserving `Any` parameter suppression and stable return type
hints through the same shared expression receiver facts.

M20.5 Phase 15 update: service and native LSP inlay fixtures now cover
source-owned method calls where the receiver is produced by a source function
return, preserving `Any` parameter suppression through the same callable
receiver path as direct typed source method calls.

M20.5 Phase 15 update: service and native LSP inlay fixtures now cover
source-owned method calls where the receiver is produced by another source
method return, preserving `Any` parameter suppression through shared
expression receiver facts.

M20.5 Phase 15 update: service and native LSP inlay fixtures now also cover
source-owned trait default method calls where the receiver is produced by a
source function return, preserving `Any` parameter suppression through the
same callable receiver path.

M20.5 Phase 15 update: service and native LSP inlay fixtures now cover
source-owned trait default method calls where the receiver is produced by
another source method return, preserving dynamic `Any` parameter suppression
through shared expression receiver facts.

M20.5 Phase 14 update: selected struct fields, enum record fields, adjacent
nested declaration member groups, completed multi-line top-level items,
completed nested impl/trait methods, and completed enum record variants now
use parser-owned spans for context-aware formatting through both the language
service and native LSP server. Remaining formatting follow-up is richer
AST-aware formatter polish rather than a named blocking LSP capability gap.

M20.5 Phase 14 update: range formatting now has service and native LSP
coverage for exact bodyless trait method selections, preserving surrounding
same-line text without injecting an extra newline into unselected whitespace.

M20.5 Phase 10 update: semantic tokens now classify source-owned trait
receiver method call sites and host-modified schema-backed trait receiver
method call sites through both the language service and native LSP server,
matching the existing reference and call-hierarchy treatment for typed trait
receivers.

M20.5 Phase 10 update: shared expression receiver facts now include
source-owned method return facts, so chained source method calls such as
`player.inventory().grant(...)` classify the second call site as a source
method through both the language service and native LSP server.

M20.5 Phase 10 update: schema function and method return facts now flow
through shared expression fact analysis, and semantic-token member
classification uses expression receiver facts so schema method calls on schema
function-return receivers classify as host/schema methods in both the service
and native LSP protocol.

M20.5 Phase 10 update: semantic-token member classification now also covers
schema-backed trait method calls where the receiver is produced by a schema
function return, preserving host/schema modifiers through shared expression
receiver facts.

M20.5 Phase 10 update: semantic-token member classification now has service
and native LSP coverage for source-owned method calls where the receiver is
produced by a source function return, matching the schema-return receiver path
through shared expression receiver facts.

M20.5 Phase 10 update: semantic-token member classification now has service
and native LSP coverage for source-owned trait method calls where the receiver
is produced by a source function return, extending the direct typed trait
receiver coverage through shared expression receiver facts.

M20.5 Phase 10 update: service and native LSP semantic-token fixtures now
also cover source-owned trait method calls where the receiver is produced by
another source method return, matching chained source method classification
through shared expression receiver facts.

M20.5 Phase 10 update: service and native LSP semantic-token fixtures now
also cover schema-backed methods and trait methods where the receiver is
produced by another schema method return, preserving host/schema modifiers for
chained schema receivers.

M20.5 Phase 7/19 update: service and native LSP member completion now cover
source-owned fields and methods where the receiver is produced by another
source method return, preserving member-context suppression of unrelated
globals through shared expression receiver facts.

M20.5 Phase 7 update: signature help and shared member callable queries now
use expression receiver facts, so schema method signatures resolve when the
receiver is produced by a schema function return in both the language service
and native LSP protocol.

M20.5 Phase 7 update: signature help now also resolves schema-backed trait
method signatures when the receiver is produced by a schema function return,
matching schema method and semantic-token receiver-fact coverage.

M20.5 Phase 7 update: signature help now also resolves source-owned method
signatures when the receiver is produced by a source function return, matching
the source-return member completion path through the shared expression
receiver facts.

M20.5 Phase 7 update: service and native LSP signature-help fixtures now cover
source-owned method signatures where the receiver is produced by another
source method return, matching member completion and semantic-token coverage
through shared expression receiver facts.

M20.5 Phase 7 update: service and native LSP signature-help fixtures now cover
source-owned trait default methods where the receiver is produced by a source
function return, matching direct record receiver and semantic-token coverage.

M20.5 Phase 7 update: service and native LSP signature-help fixtures now cover
schema-backed methods and trait methods where the receiver is produced by
another schema method return, matching completion, references, document
highlights, call hierarchy, and inlay coverage for chained schema receivers.

M20.5 Phase 7 update: member completion now has service and native LSP
coverage for schema-backed fields and methods on receivers produced by schema
function returns, preserving member-context suppression of unrelated globals.

M20.5 Phase 7 update: member completion now also has service and native LSP
coverage for source-owned fields and methods on receivers produced by source
function returns, matching the schema-return receiver path without falling
back to unrelated globals.

M20.5 Phase 7/19 update: service and native LSP member completion now also
cover schema-backed fields, methods, and trait methods where the receiver is
produced by another schema method return, preserving member-context
suppression of unrelated globals through shared expression receiver facts.

M20.5 Phase 11 update: references now have service and native LSP coverage for
schema-backed method calls where the receiver is produced by a schema function
return, using the shared expression receiver facts instead of binding-only
receiver spans.

M20.5 Phase 11 update: references and document highlights now have service and
native LSP coverage for source-owned method calls where the receiver is
produced by a source function return, matching the schema-return receiver path
through shared expression receiver facts.

M20.5 Phase 11 update: references and document highlights now also cover
source-owned trait default method calls where the receiver is produced by a
source function return, matching the signature-help and semantic-token
returned-receiver coverage.

M20.5 Phase 11 update: references and document highlights now cover
source-owned inherent and trait default method calls where the receiver is
produced by another source method return, matching completion, signature-help,
semantic-token, and inlay returned-receiver coverage.

M20.5 Phase 11 update: document highlights now also have service and native
LSP coverage for schema-backed method calls where the receiver is produced by
a schema function return, matching the existing references coverage.

M20.5 Phase 11 update: references and document highlights now also cover
schema-backed trait method calls where the receiver is produced by a schema
function return, matching the schema method returned-receiver path.

M20.5 Phase 11 update: references and document highlights now cover
schema-backed method and trait-method calls where the receiver is produced by
another schema method return, matching chained schema receiver facts already
used by semantic tokens and inlay hints.

M20.5 Phase 11 update: call hierarchy now has service and native LSP coverage
for prepare, incoming, and outgoing schema method calls where the receiver is
produced by a schema function return.

M20.5 Phase 11 update: call hierarchy now also covers prepare, incoming, and
outgoing schema-backed trait method calls where the receiver is produced by a
schema function return, matching references and document highlights.

M20.5 Phase 11 update: call hierarchy now covers prepare, incoming, and
outgoing source-owned inherent and trait default method calls where the
receiver is produced by another source method return, reusing the same
trait-default receiver resolution as references.

M20.5 Phase 11 update: call hierarchy now covers prepare, incoming, and
outgoing schema-backed methods and trait methods where the receiver is
produced by another schema method return, matching the chained schema receiver
coverage in references and document highlights.

M20.5 Phase 10 update: native LSP semantic-token fixtures now cover
source-owned field and inherent-method call-site classifications, matching the
existing language-service member-use classification path.

M20.5 Phase 10 update: semantic tokens now classify imported module path
segments as module/namespace tokens through both the language service and
native LSP server while preserving the resolved declaration class for the
imported item.

M20.5 Phase 10 update: native LSP semantic-token fixtures now cover
schema-backed enum variant constructor and pattern classifications, matching
the existing language-service schema enum variant token path.

M20.5 Phase 13 update: native LSP code-action fixtures now cover open-overlay
range stability for schema-backed typo repairs when disk snapshots differ from
the active editor buffer.

M20.5 Phase 17 update: editor initialization options and
`workspace/didChangeConfiguration` settings now map `workspace.roots` and
`host.schema` into the server `WorkspaceConfig`, native `--root` and
`--schema` launch flags seed the same fallback configuration for stdio
sessions, configured schema artifacts load without running host code, project
indexes invalidate after config changes, and `vela.toml` remains the
authoritative project config when present. The native LSP release workflow now
builds Linux, macOS, and Windows server artifacts with checksums for workflow
artifacts or tagged releases. The VS Code package under `editors/vscode` is a
thin launcher/configuration extension for the native server and keeps feature
behavior in the shared LSP/language-service layers. The Zed package under
`editors/zed` follows the same boundary with Vela language metadata and a
native-server stdio command hook. Both editor package validators now assert
that launcher packages do not implement LSP request behavior.

M20.5 Phase 18/19 update: the protocol matrix acceptance gate is complete for
the current advertised native LSP surface. `cargo test -p
vela_language_service` passes all 383 active language-service tests plus
doctests, and `cargo test -p vela_lsp_server` passes all 279 library tests, 3
CLI/main tests, and doctests. Parser/HIR/analysis focused tests also pass with
`cargo test -p vela_syntax`, `cargo test -p vela_hir`, and `cargo test -p
vela_analysis`. The explicit many-file scale checkpoint
`cargo test -p vela_language_service million_line_synthetic_workspace_checkpoint
-- --ignored` passes at roughly one million synthetic lines while preserving
single-file reparse and no full HIR rebuild on edit. Full workspace validation
passes with `cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets -- -D warnings`, and
`cargo test --workspace`. This validates the baseline protocol plumbing,
analysis-only capability track, and Phase 19 rust-analyzer-aligned authoring
core for formatter, completion, and snippet behavior. Phase 19 now has
structured completion analysis; explicit path, type, dot, declaration, call,
pattern, and statement contexts; a unified source/schema/stdlib/builtin member
surface; separated completion display, insertion, and projection fields;
native LSP JSON-RPC fixtures for the correction set; statement snippets; and
syntax-owned compact type-hint formatting.

M20.5 cleanup update: the clean LSP architecture refactor has completed its
shared query/display/symbol Phase 5 checkpoint. Language-service feature
results now route source, schema, builtin, local, completion, hover,
definition, references, rename, symbol, diagnostic, and inlay identities
through shared `SymbolRef` constructors and `DisplayParts` metadata where
relevant, and schema-owned definitions no longer fall back to enclosing script
declarations when no schema source span exists.

M20.5 Phase 9 update: workspace symbols now include source-file symbols with
LSP `SymbolKind.File` mappings and module detail/source locations, closing the
file/module/class/function/method/field/enum/variant symbol-kind checklist.

M20.5 Phase 8 update: source global hover now has a first-class `global`
kind through the editor-neutral service and native LSP markdown, with HIR
declaration metadata used as a fallback when analysis facts are unknown.

M20.5 Phase 8 update: source and schema trait hovers now use a first-class
`trait` kind through the language service and native LSP markdown. Source
type-hint hover now consults module-graph type declarations before falling
back to schema or unknown type-hint degradation.

M20.5 Phase 8 update: service and native LSP hover now resolve source-owned
trait default method calls where the receiver is produced by a source function
return, using trait declaration docs and signatures instead of inventing a
record-owned method fact.

M20.5 Phase 8 update: service and native LSP definition/declaration now also
resolve source-owned trait default method calls where the receiver is produced
by a source function return, landing on the trait method declaration.

M20.5 Phase 16 update: deleting `vela.toml` now has explicit watcher coverage
that clears published configuration diagnostics and returns the server to
workspace-root/editor fallback configuration.

M20.5 Phase 16 update: deleting a configured host schema artifact now has
explicit watcher coverage that publishes the missing-schema diagnostic without
running host code.

M20.5 Phase 8 update: native LSP definition fixtures now cover schema-backed
host method and trait-method source spans in addition to schema types, fields,
and variants.

M20.5 Phase 8 update: service and native LSP type-definition fixtures now
cover imported source trait method return types, extending the W1 cross-file
navigation audit beyond inherent method return types.

M20.5 Phase 5 update: native LSP `didClose` coverage now proves completion
queries return to disk-snapshot declarations after an open overlay is closed,
matching the existing diagnostic and definition restoration fixtures.

M20.5 Phase 8 update: native LSP declaration fixtures now cover schema-backed
type and host-method source spans, proving clients that separate declaration
from definition get the same schema-origin navigation.

M20.5 Phase 8 update: native LSP declaration fixtures now also cover
schema-backed trait-method and variant source spans, matching the existing
definition behavior for those schema origins.

M20.5 Phase 8 update: native LSP declaration fixtures now cover schema-backed
field source spans as well, closing the explicit protocol fixture parity gap
with schema field definitions.

M20.5 Phase 8 update: language-service and native LSP type-definition fixtures
now resolve through type facts instead of sharing definition fallback behavior.
Local source values plus source/schema field member expressions jump to
source/schema type declarations when source-backed, while primitive fields,
schema methods, schema trait methods, and schema variants without owner type
spans return null for `textDocument/typeDefinition`.

M20.5 Phase 15 update: language-service and native LSP inlay fixtures now
cover schema-backed method and trait-method calls where the receiver is
produced by another schema method return, preserving dynamic `Any` parameter
suppression through chained schema receiver facts.

M20.5 Phase 15 update: language-service and native LSP inlay fixtures now
cover stdlib callback/lambda parameter hint suppression when inferred
collection facts cross dynamic `Any` boundaries, while preserving stable
callback parameter hints for concrete collection element types.

M20.5 Phase 15 update: language-service and native LSP host-path type inlay
fixtures now cover schema-backed fields where the host receiver is produced by
another schema method return, preserving stable field hints while suppressing
dynamic `Any` field hints.

M20.5 Phase 7 update: completion now resolves schema-backed trait receiver
method members in addition to schema-backed host receiver members.
`textDocument/signatureHelp` now resolves stdlib function calls, typed
source-owned inherent method calls, schema-backed host and trait receiver
method calls, and stdlib callback method calls in addition to script and
schema function calls. The language service tests cover schema host/trait
member completion plus script, schema, stdlib function, source method, schema
host/trait method, and stdlib callback method signatures, and the LSP fixtures
cover the same paths. The Phase 7 completion/signature checklist is now closed
against the focused service, analysis, LSP fixture, and capability tests.

M20.5 Phase 8 update: hover now reports stdlib global function facts,
typed stdlib receiver-method facts, and schema-backed trait receiver method
facts through both `vela_language_service` and `textDocument/hover` fixtures.
Source-owned struct field declarations/member uses, source-owned method
declarations plus typed record and trait receiver calls, and source-owned enum
variant declarations/constructor uses now report facts through the same hover
path, including docs where the HIR metadata carries them.
`vela_language_service` now exposes explicit declaration and type-definition
navigation queries, and `vela_lsp_server` advertises and serves
`textDocument/declaration` plus `textDocument/typeDefinition`; declaration
uses source/schema-backed definition spans, while type definition uses explicit
type-fact targets and null degradation for non-type values.

M20.5 highlighting completion update: semantic highlighting now has a
service-owned Vela token taxonomy, native LSP full/delta/range projection with
client-specific fallback, Zed Tree-sitter syntax fallback captures, VS Code
TextMate plus semantic-token scope metadata, and a shared consistency table
validated across service, LSP, and editor package checks. Editor packages
remain thin launchers/configuration layers and do not compute semantic
classification.

M20.5 Phase 16 update: clients that support dynamic watched-file registration
now receive a `client/registerCapability` request for configured `.vela`
source roots, workspace `vela.toml`, and the configured host schema artifact
after `initialized`.

M20.5 Phase 11 update: source-owned inherent script methods now participate in
the editor reference index. `textDocument/references` returns method
declarations plus typed receiver call sites, and
`textDocument/documentHighlight` marks same-document method declarations and
calls. Source-owned inherent, trait impl, and trait default/interface script
methods also participate in `textDocument/prepareCallHierarchy`, incoming
calls, and outgoing calls for typed receiver call sites. Source-owned trait declarations and
`impl Trait for Type` paths now participate in `textDocument/references` and
same-document highlights. Schema-backed fields with source spans now
participate in `textDocument/references` for declarations and typed host
receiver read/write uses. Schema-backed methods with source spans now
participate in `textDocument/references` for declarations and typed host
receiver call sites. Schema-backed trait methods with source spans now
participate in `textDocument/references` for declarations and typed trait
receiver call sites. Schema-backed variants with source spans now participate
in `textDocument/references` and `textDocument/documentHighlight` for
constructor reads and match-pattern uses. Schema-backed methods and trait
methods with source spans now participate in `textDocument/prepareCallHierarchy`,
incoming calls, and script-caller outgoing calls for typed receiver call sites.

M20.5 Phase 11 update: native LSP reference fixtures now also cover
schema-backed record-constructor shorthand field labels, matching the existing
language-service shorthand coverage and explicit-label LSP fixture path.

M20.5 Phase 11 update: document highlights now have explicit language-service
and native LSP coverage for schema-backed field reads and writes in the active
document, matching the existing schema method and variant highlight paths.

M20.5 Phase 11 update: native LSP document-highlight fixtures now cover
source-owned trait declaration and impl-use highlights, matching the
language-service trait highlight coverage.

M20.5 Phase 11 update: native LSP document-highlight fixtures now cover
imported script function imports and same-document call sites, matching the
language-service imported-function highlight coverage.

M20.5 Phase 11 update: imported module path segments now participate in
language-service and native LSP references across workspace imports, and
document highlights mark matching module segments in the active document.

M20.5 Phase 13 update: the conditional null-check to Option/Result guard
rewrite is intentionally deferred until a structured diagnostic or syntax
pattern can prove the rewrite is local, source-owned, and
semantics-preserving. Current code actions remain diagnostic-backed and keep
ambiguous/dynamic fixes rejected rather than offering speculative semantic
rewrites.

M20.5 clean LSP architecture Phase 7 update: code actions now build all quick
fix edits through checked `EditPlan`s, keep ambiguous imports, dynamic receiver
typos, and unproven semantic rewrites rejected, and pin semantic rewrite
helpers to local syntax patterns. Formatting remains syntax-owned through
`vela_syntax::formatting`, preserves comment/blank-line trivia plus
semicolonless `use` item boundaries, and still formats when HIR analysis has
unresolved imports. Range and on-type formatting are gated by parser-owned
item/member spans with trivia-limited fallbacks. Inlay hints now use shared
callable parameter facts, stable `TypeFact`s, `Any`/unknown suppression, and
`DisplayParts` labels before native LSP rendering.

## Current Milestone State

### Available Now

- `.vela` source parsing, HIR lowering, bytecode compilation, VM execution
  with instruction, memory, call-depth, and collection growth budgets,
  ordinary and indexed `for-in`, inherent `impl Type` methods, trait
  `impl Trait for Type` methods, single-line and multiline strings, explicit
  `f"..."` and `f"""..."""` string interpolation,
  managed heap entrypoints, execution budgets, and non-moving GC foundations.
- Host mutation through `HostRef`, `HostPath`, `PathProxy`, write-through
  `HostAccess`, and capability-gated effects.
- Reflection for types, fields, methods, variants, traits, modules, functions,
  attributes, permissions, controlled reads/writes/calls, and candidate spans.
- Standard library runtime and analysis coverage for arrays, maps, sets,
  strings, Option/Result helpers and propagation, math, deterministic time,
  context event/log helpers, controlled random capability gating, opt-in
  stdio and sandboxed filesystem helpers with `io_read`/`io_write`
  capability gating, lambda TypeFacts, explicit iterator creation methods, core
  one-shot iterator terminals and lazy `map`/`filter`/`take`/`skip` adapters,
  iterator-backed array value, map key/value/entry, and set value views, and
  domain-neutral helpers.
- Engine registration for host types, native functions, context helpers,
  standard natives, capability profiles, reflection permissions, compiler options, dynamic
  `CallArgs`, direct call-boundary `&T`/`&mut T` host object bindings,
  module-level `global` declarations backed by persistent Rust-defined host objects
  or Runtime-owned script values with unified `insert_global` support for
  `OwnedValue`, `OwnedValue::iterator` snapshot iterables, serde snapshots, and
  same-runtime `VelaValue` handles,
  feature-gated serde conversion between Rust structs/enums and script-owned
  `OwnedValue` records/enums for snapshot-style arguments and results, direct
  serde decoding from runtime-managed `VelaValue` and globals without
  materializing detached `OwnedValue`,
  runtime-managed `VelaValue` call returns that can be passed back to later calls
  on the same runtime without owned materialization, cached `VelaFunction`
  entry handles and `VelaMethod` script-value method handles for high-frequency
  embedding calls, `Send` Runtime and `VelaValue` handles for worker/actor ownership transfer,
  direct host object method dispatch with receiver paths, unified concrete host
  type specs, host index capability metadata, typed host path arguments,
  string-key host path segments, hot-reload policies, derive-generated host
  bindings, and reflection schemas.
- A dedicated `vela_c_api` crate exists for the external C ABI boundary,
  separate from hot-reload ABI. The first slice exposes opaque engine/runtime
  handles, source compilation, no-argument entry calls, scalar C result values,
  and ABI-owned string/value cleanup.
- Macro-generated host and native bindings with stable IDs, rename aliases,
  effect-aware registration, and budget-aware context helper coverage.
- Hot reload staging and safe-point reports for source-file, directory, and
  changed-file workflows, including accepted compatible additions/renames and
  rejected ABI/schema/effect/access/source changes without advancing the active
  version.
- Standalone `vela_examples` bins and conformance fixtures covering domain-neutral stdlib helpers,
  reflection, schema-safe mutation denial, capability gating, read-only host boundary
  rejection, host read/write/call capability denial, stale host ref generation
  rejection, host write/call denial diagnostics, reflection candidate
  diagnostics, bad schema diagnostics, unsupported generic type hint rejection, and
  tick-boundary hot reload. A standalone host iterable example covers
  native-returned `OwnedValue::iterator` snapshot traversal through `for-in`
  and lazy iterator adapters without first returning a script array. A
  standalone host type method example covers concrete host type specs,
  receiver-path methods, keyed host paths, child receiver method calls, and
  typed host path arguments. A standalone script
  global example covers VM-owned global initialization, script mutation, Rust
  `OwnedValue` constructor/macro updates, and later script reads of the same
  persistent value. A standalone I/O stdlib example covers stdout plus
  sandboxed file read/write.
- A GitHub Pages documentation site source exists under `site/` as an Astro
  Starlight project with English root routes, a complete Chinese `/zh/`
  mirror, Starlight navigation/search/i18n, first-pass formal documentation
  across the main guide, language, data, methods, stdlib, host, hot reload,
  reflection/tooling, and reference sections, and a browser playground backed
  by the `vela_playground_wasm` wrapper. The Pages workflow builds the WASM
  target, generates `wasm-bindgen` browser bindings into Astro public assets,
  builds the npm site, and deploys the Astro static artifact.
- A parser fuzz target exists under `fuzz/` and can be compile-checked even
  when the local machine has not installed `cargo-fuzz`.
- Current benchmark rules, baseline summaries, and M19 exit conclusions live in
  [performance.md](performance.md). Detailed M18/M19 benchmark history is
  archived in [archive/performance-full-2026-06-06.md](archive/performance-full-2026-06-06.md).
- The typed scalar bytecode optimization pass has landed through the first
  non-JIT i64 slice: opcode visibility exists for external comparison
  workloads, linked jump/range structural checks are verifier-owned, verified
  `i64` arithmetic/immediate bytecode executes with checked semantics and
  source-spanned errors, the compiler lowers only proven i64 facts to typed
  scalar ops, direct integer `for` ranges lower to `I64RangeNext`, and linked
  execution has a no-hook mode split for inactive budget/profiler paths.
  Superinstructions are intentionally deferred until a profile-backed fused
  condition lowering can prove temporary-register liveness or lower directly
  from source-owned condition structure.
- The M19 interpreter/heap phase is complete enough for M20. Accepted work
  covered GC pacing, direct heap aggregate construction, argument
  materialization and storage, borrowed receiver/runtime views, collection and
  string fast paths, Option/Result helpers, scalar equality and constant loads,
  peephole/range-loop lowering, small record/enum field construction, and short
  array construction.
- The remaining Lua 5.x deltas are concentrated in cache-shaped paths:
  script record fields use shape/slot representations, host field/path reads
  and writes use `HostTargetPlan` and resolved access boundaries, method
  dispatch uses resolved targets, broader stdlib and callback dispatch has
  receiver-guarded targets, callback and closure calls need lower
  materialization overhead, and hot bytecode offsets need interpreter-vs-cache
  measurement.
- M19.5 has started with native call operands: compiled native calls can carry
  stable `FunctionId` metadata while preserving names for diagnostics and
  fallback, and Engine-installed plus standard native functions register ID
  lookup targets. Native call dispatch is routed through a focused VM call
  boundary, preserving ID-first lookup, name fallback, HostAccess routing
  checks, and source-spanned errors. Standard value method calls can also carry
  optional `HostMethodId` metadata, with string/range/collection
  `len`/`is_empty`, string predicates/transforms/Option/split/parse helpers,
  collection predicates, array lookup/transform helpers, array/map/set mutators,
  and Option/Result predicates using an ID fast path before name fallback, and
  script/value method dispatch is routed through a focused VM call boundary.
  Host field/path reads, writes, compound
  mutations, and host method calls are routed through a focused VM
  host-access boundary, giving later path-key or direct-adapter work one
  replacement point. The host adapter boundary now resolves `HostTargetPlan`
  shapes into `ResolvedHostAccess` handles before executing read, write,
  mutate, remove, or call operations, and the mock adapter stores successful
  values by target instance identity while materializing diagnostic paths only
  for current error/reporting surfaces. HostPath construction now has an exact-capacity/static
  segment materialization boundary so field-only paths can bypass dynamic
  index/key conversion, and HostPath no longer carries a root-inclusive cache
  key sidecar. Unlinked bytecode
  `UnlinkedCodeObject` values now own interned `HostTargetPlan` tables and the
  collapsed `HostRead`/`HostWrite`/`HostMutate`/`HostRemove`/`HostCall`
  instruction family has verifier coverage for target bounds, contiguous
  dynamic arguments, and cache-site kind matching. Source compiler lowering
  now interns host field, path, mutation, remove, push, and method-call targets
  into those tables and emits the collapsed family through the focused
  host-access boundary, with registered host type IDs preserved for typed root
  plans and mock storage canonicalized across static and dynamic key shapes.
  `PathProxy` now stores a root `HostRef`, `HostTargetPlan`, and owned dynamic
  args, routing operations through `HostTargetInstance` and materializing
  `HostPath` only at explicit diagnostic/embedding conversion edges.
  Runtime inline caches now have host access entries guarded by root type,
  target-plan ID, operation, and host schema epoch; collapsed host bytecode
  resolves through that cache boundary while adapter execution still validates
  generations, permissions, and source-spanned slow paths. Runtime inline
  caches are scoped to the active runtime image, undersized cache providers are
  rejected before execution, and accepted hot reloads clear stale entries
  before reused cache-site indexes can repopulate from the new bytecode.
  `ProgramImage` rebases embedded global and host cache-site operands to
  image-wide IDs so multi-function images cannot alias cache entries by local
  site index.
  The HostPath/HostAccess M19.5 gap is complete: hot execution uses
  `HostTargetPlan`, `HostTargetInstance`, and `ResolvedHostAccess`, with
  `HostPath` reserved for diagnostics, reflection, embedding materialization,
  and fixture setup.
  Host-boundary
  conversion failures are covered as HostAccess slow paths that leave adapter
  state unchanged.
  Source and module compilation now verifies bytecode before returning
  `UnlinkedCodeObject` or `UnlinkedProgram` values, covering register, constant, jump,
  frame-slot, call-argument, host-path dynamic segment, and nested closure
  invariants before future unchecked register, operand, or cache fast paths
  are introduced. Bytecode verification also validates cache-site sidecar IDs,
  instruction offsets, and instruction-kind matches for cacheable operations.
  Program verification also rejects script method metadata whose resolved
  target function is missing, keeping MethodId dispatch and future method-cache
  metadata target-complete before M20.
  Compiler output is now explicitly unlinked bytecode:
  `UnlinkedProgram`, `UnlinkedCodeObject`, `UnlinkedInstruction`, and
  `UnlinkedInstructionKind` carry semantic IDs without requiring runtime
  handles during compilation.
  The linked-bytecode representation now exists separately as `LinkedProgram`,
  `LinkedCodeObject`, `Instruction`, and `InstructionKind`, with executable
  operands shaped as dense handles or slots and debug names stored in a side
  table. Linked bytecode verification now rejects invalid debug-name
  references, out-of-bounds dense handles, and invalid local register,
  constant, jump, cache-site, and host-target operands before execution, and
  validates linked cache-site sidecar IDs, offsets, and instruction kinds.
  ProgramVersion now owns bytecode-offset profile layout metadata for each
  function and rebuilds that sidecar when hot reload creates a new version, so
  future counters, cache state, or JIT decisions can be version-scoped and
  invalidated with the version; rejected reloads keep the previous version
  profile unchanged. Runtime-owned bytecode profile counters now record linked
  instruction-offset hits through nested script, method, closure, and callback
  calls, and accepted hot reload resets the counter sidecar for the new image.
  The VM now has linked-program execution for scalar, comparison, branch,
  return, budget-charged instructions, script/native/value/script-method calls,
  array/map/range/index/iterator/global/host operations, and record slot
  construction/read/write plus enum construction/slot/tag operations without
  rebuilding unlinked code; linked closure opcodes now carry linked function
  handles through closure values, and linked host-method `CallMethod` dispatch
  routes through HostAccess. All linked instruction variants now have explicit
  VM execution paths; engine runtime raw calls and normal `Runtime::call` /
  script `Runtime::call_method` paths now require the image's linked program
  for persistent and fresh heap entrypoints instead of falling back to
  `ProgramImage` execution. Engine linking now uses the definition
  registry plus installed native implementation IDs, and engine-compiled
  initial and accepted hot-reload versions carry version-owned linked layouts
  that runtime images reuse after safe-point acceptance. Standalone hot-reload
  compilation now attaches linked layouts for linkable script-only versions,
  and hot-reload behavior tests execute those linked version layouts instead
  of rebuilding unlinked programs through `ProgramVersion::to_program()`.
  Engine hot-reload linking now rebuilds linker input from version/update-owned
  function metadata instead of the `ProgramImage::to_program()` compatibility
  path, and `ProgramImage::to_program()` has been removed. No-heap raw runtime
  `run_program_runtime*` VM APIs and their diagnostic fixture callers have been
  replaced with linked-program execution, and dead managed-heap runtime wrapper
  aliases plus their helper have been deleted. The unlinked
  `run_program_with_managed_heap_and_budget` API has also been removed; its VM
  test callers now link before execution, with standard-registry facts used for
  stdlib/value methods and empty aggregate literals carrying unknown element
  shapes instead of falling back to unresolved method names. The unlinked
  `run_program_with_budget` wrapper has also been deleted after its callers
  moved to linked execution. The remaining public direct unlinked VM execution
  convenience entrypoints have been deleted, and single-function VM benchmark
  modes now link before execution while preserving linked heap-budget coverage.
  Linkable `execution_core` coverage and the compiled conformance fixture now
  run through linked bytecode after ad-hoc source record literals, enum pattern
  fields, stdlib callback receiver facts, and linked callback closures gained
  linker-ready operands/runtime ownership. Script function calls are linked
  through `ScriptFunctionHandle` tables, with mismatched call IDs rejected by
  the linker and linked execution calling by dense handle.
  Script function dispatch is being isolated behind a focused call boundary so
  later resolved-target work does not grow the main VM loop or change current
  hot-reload rename semantics. Closure creation and invocation now have a
  focused VM boundary that preserves protected roots and call-site offsets
  while materializing common capture counts through inline small storage.
  Higher-order callback dispatch now reuses the shared execution-call
  descriptor and borrows closure metadata instead of cloning the full closure
  value for each callback, and linked stdlib callback bodies receive the active
  inline-cache provider for cacheable nested operations.
  Persistent runtime-managed `VelaValue` handles are now included in
  script-global collection roots, so retained call results survive later
  `insert_global`/`update_global` heap collections.
  Runtime `CallOptions` budget checkpoints now cover both instruction limits
  and recursive call-depth limits at the embedding boundary, including
  source-spanned call-stack reports.
  Script array/map/range construction, record/enum construction, and script
  field reads/writes now route through focused script aggregate/object
  boundaries while preserving current name fallback, small-field construction,
  and slot guards. Generic iterator and range-loop stepping now route through a
  focused iteration boundary with jump validation kept on the VM side of the
  bytecode contract. Declared global reads now carry `GlobalSlot` metadata so
  VM-owned script globals and runtime host globals can use slot lookup on the
  common path while preserving names for diagnostics and fallback. Native
  dispatch no longer has string-name fallback maps: standard and host-native
  source-name aliases install as explicit `FunctionId` bindings, reflection
  calls resolve callable descriptors to IDs, and linked bytecode keeps native
  handles plus debug names separated from runtime dispatch. Native-call
  cache-site operands are preserved from compiler output through linked
  bytecode verification and benchmark cache-site rebasing, and linked native
  dispatch now caches resolved pure, host, and borrowed-host targets behind a
  `FunctionId` guard while retaining current slow-path behavior on misses.
  Linked method
  dispatch now uses dense method handles for script, host, and value method
  paths; linked value method execution calls standard methods by `MethodId`
  only, with debug names reserved for error reporting. Runtime Option/Result
  heap values now carry standard `TypeId`/`VariantId`/payload-field identity,
  and standard method plus `try` propagation paths classify them through those
  IDs and slot reads instead of string-name fallback. Linked script enum
  construction now stores `TypeId`/`VariantId` identity in heap enum values,
  and linked enum tag checks compare those IDs while retaining names for
  diagnostics and reflection. Linked record construction now stores
  `TypeId` plus `ShapeId` identity in heap record values, while linked record
  field reads/writes continue through `FieldSlot` operands and diagnostic
  names remain side-table metadata. Engine definition registry construction now
  consumes registered host type, field, method, and native function inputs
  directly instead of rebuilding compiler identity from reflection-only
  descriptors; reflection metadata remains a separate runtime view. Linked
  method-call and record field read/write instructions now preserve cache-site
  operands from cache-site sidecars, with linked verifier and runtime image
  rebasing coverage. Linked script record field reads and writes now populate
  guarded runtime inline-cache entries keyed by `TypeId`, `ShapeId`, and
  `FieldSlot`, and guard misses fall back to the existing slot slow path before
  replacing stale entries. Linked method calls now populate runtime
  inline-cache entries keyed by `MethodDispatchHandle`, caching resolved
  script, value, or host targets before falling back to linked method-dispatch
  lookup on misses; accepted hot reloads clear those record-field and
  method-dispatch cache entries before the new image repopulates them. Native
  call cache entries now have the same accepted-hot-reload clearing coverage.
  The primitive scalar, bytes, type-hint contract, and guard-plan refactor is
  complete: source `int`/`float` hints are gone, runtime/owned/host/constant
  values share `ScalarValue` and bytes representations, type hints are
  contracts with compile-time and linked runtime guard enforcement, numeric
  operators require identical concrete scalar tags, byte strings and bytes APIs
  are covered, and final validation passes. Root host receiver index reads,
  writes, compound mutations, and removals lower for typed roots with
  configured host index capabilities, and numeric key contracts emit dynamic
  index target parts for cache-ready host access plans.

### Remaining Gaps

M20 should now be driven by close-out criteria instead of broad "continue
guarded inline-cache specialization" tasks. A remaining cache task is valid
only when it names the specific family and one missing proof:

```text
coverage: no cache entry exists for a measured hot path
correctness: hit, miss, wrong-guard, fallback, reload, or schema invalidation coverage is missing
measurement: interpreter-only, profile-only, and cache-enabled rows cannot yet be compared
decision: measured cache delta has not been classified as keep, investigate, or defer
```

Current M20 close-out gates:

- Cache-family audit: list existing cache families and mark each as complete,
  incomplete, or explicitly deferred. Do this before adding another cache
  family.
- Correctness proof: every completed family keeps generic fallback behavior and
  covers guard failures, hot reload invalidation, and schema or version
  invalidation where applicable.
- Measurement proof: cache-enabled rows must be compared against the right
  interpreter-only or profile-only baseline with `measurement_kind`,
  `delta_kind`, `measurement_summary`, and `cache_delta_summary`.
- Decision proof: slower or flat cache deltas must be assigned to a named
  follow-up, accepted as neutral overhead, or deferred to JIT/value-layout work;
  do not leave them as generic M20 work.
- Scope proof: new M20 implementation should be a small named family, not a
  cross-cutting cache expansion. Larger representation or value-layout changes
  belong to a separate milestone decision.

Lua 5.x comparable performance remains a measured target for cache-enabled
non-JIT host-boundary workloads. Scalar, array, string, function-call,
callback, and host-boundary deltas should stay separated so M20 can close
without hiding unrelated future JIT work.

### Validation

Use the relevant subset of [validation.md](validation.md) for each change.
Default full validation remains:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

For remaining M20 cache-entry work, run focused correctness tests for touched
bytecode, runtime dispatch, host-boundary, and stdlib/native call paths plus
the relevant interpreter-only/profile-only/cache-enabled benchmark rows.
Preparatory fast paths must preserve ExecutionBudget, HostAccess, reflection
policy, GC roots, hot reload ownership, schema invalidation, and source-spanned
diagnostics.

## Next Up

- Audit M20 cache families and classify each as complete, incomplete, or
  deferred before starting more implementation.
- Close only named cache-family gaps with focused tests and paired benchmark
  evidence. Avoid generic "continue specialization" tasks.
- Keep the completed primitive scalar, bytes, type-hint contract, and guard-plan
  refactor as the baseline; do not reintroduce old `int`/`float` compatibility
  paths or string fallback dispatch.
- Keep the clean LSP architecture refactor as the validated M20.5 editor
  tooling baseline; future LSP work should start from the shared
  query/context/result/projection boundary instead of restoring old
  completion or protocol-coupled paths.
- Plan M21 debugger and M22 Cranelift JIT only from stable source-span,
  frame-map, GC-root, budget, HostAccess, hot-reload, tooling, and conformance
  contracts.

## Update Rules

- Update this file when current focus, milestone status, available capability
  coverage, validation expectations, or remaining current gaps change.
- Do not append routine implementation details, small refactors, or every
  commit result here; those belong in commit history or focused tests.
- Keep the file quick to scan. If durable historical context becomes necessary,
  summarize it once and archive the long form under `docs/archive/`.
