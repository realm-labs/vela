# Completion: S1 review

Scope: `completion/syntax/S1/{positive,negative}/{service,protocol}`. Exact test
identities are bound in the catalog; a fresh audit establishes verification.

| Partition | Shared evidence | Assertions |
|---|---|---|
| Top-level declaration authoring | `completion-authoring-surface` | Exact pub/use/const/state/extern-state/fn/struct/enum/trait/impl template inventory, labels, insertions, ranges and explicit completed-source parses. Item boundaries survive trivia and exclude value/module fallback. |
| Const and state references | `completion-declaration-contexts` | Typed const, per-runtime state and extern state have exact candidates, types, source identity and applied definition targets. Unhinted const metadata retains unknown detail under the existing declaration-fact contract; completion does not infer or execute its initializer. Const/state initializers and struct-field defaults resolve their source-owned references. |
| Parameters and defaults | `completion-declaration-contexts` | Function, default trait, inherent and trait-impl bodies/defaults retain exact parameter names, types and targets. An executable default can capture an earlier parameter through a lambda. Later parameters, body locals and sibling/closed methods are excluded. Single-identifier defaults retain their body at the half-open end boundary. |
| Required trait signatures | `completion-declaration-contexts` | Methods without bodies use existing signature metadata in their default-expression span. Typed/unknown parameters, exact visible names, later/sibling exclusion, and same-name globals/import aliases are explicit. A shadowed global keeps a qualified insertion and its own definition. Parameters have no lazy resolve identity or invented docs. |
| Types, fields and variants | `completion-type-positions`, `completion-type-ownership`, `completion-enum-aliases` | Source/schema type positions, struct and enum payload fields, tuple/record/unit variants, trait names and aliases retain exact owner sets, edits, resolve policy and applied targets. Private/missing/non-type/ambiguous owners have explicit exclusions. |
| Functions, methods and visibility | `completion-package-callables`, `completion-package-members`, `completion-import-sites` | Public/private and root/dependency ownership, functions/constants, inherent/trait/default methods, parameter names, return facts and import-site insertion are independently checked. Applied references/signatures cannot borrow a colliding owner. |

The new fixture has 26 queries, including seven exact empty sets, each run with
Unicode and LF/CRLF. Both layers assert complete candidate sets, detail, identity,
resolve, independent byte/UTF-16 edits, parsed applied source and exact definition
locations. Service results repeat; protocol uses actual completion/resolve,
didChange, definition and restored queries.

Two executable-query defects are fixed: an identifier can anchor its body when
the cursor is exactly at its half-open end, and the selected body's canonical
binding map includes const/state/field initializers. Required trait methods have
no executable body; direct signature-parameter authoring reads HIR metadata and
uses the same visible-name set for rendering and shadow checks. No runtime body
or language semantics are added.

This declaration review does not certify arbitrary nested lexical bodies inside
required trait defaults; that remains part of S2/body review. It also does not
certify S7, generated combinations, scale, UX04 or fresh macOS execution. B03
remains open and B16 stays deferred.
