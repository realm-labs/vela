# Completion: S14 review

Scope: `completion/syntax/S14/{positive,negative}/{service,protocol}`. The
catalog binds exact tests; a fresh executed audit determines verification.

| Partition | Evidence | Assertions |
|---|---|---|
| Structured authoring contexts | `completion-analysis-contexts` | Fifteen shared LF/CRLF queries assert expression/module paths, typed/dynamic dot receivers, item/field declarations, record fields, calls, patterns and statements. Service assertions inspect the analysis enum, receiver range/fact, qualifier, expected type/name and visible scope before checking editor-neutral items. Protocol assertions independently check the resulting complete candidate sets and UTF-16 edits; internal analysis is not serialized. |
| Type locations | `completion-type-positions`; empty-slot analysis test | Twenty-six shared queries cover parameter, return, field, local/state and nested builtin arguments. Service assertions inspect TypeLocation; both layers retain owned candidates, edits, documentation and applied-source proof. Four additional service cases distinguish empty Array/Map/Result argument slots. Direct CST separators exclude nested tuple/container commas and comment punctuation. |
| Visible bindings | `completion-visible-bindings` | Thirteen shared cases cover loop value/index, shadowing, nested capture, iterator-source exclusion, restoration after loops, let initializer exclusion, pattern body/guard/capture, next-arm isolation and exclusion inside the binding pattern itself. Service assertions include exact visible scopes. Both layers assert complete candidate sets, binding types, applied references, exact definition owners and requery. |
| Expected argument facts | `completion-analysis-contexts` | Known positional and reordered named arguments share the selected parameter index, name and type. A named-value expression keeps its expression context and expected facts. Unresolved calls and ambiguous partial arguments retain syntax context without invented name/type facts. |
| Unified member index | Member-index test; `completion-members`, `completion-package-members`, `completion-authoring-surface` | Source/schema fields and methods, inherent/trait/default methods and builtin methods have exact owner sets, signatures and exclusions. The index test directly checks unified membership and source/schema precedence. Existing shared matrices prove owned edits, resolve and applied targets; eight builtin empty-dot families have complete inventories. |
| Editor-neutral rendering and projection | Analysis, type and member matrices | Service details, rendered detail parts, label details, insertions and byte edits are explicit. Protocol kind/detail/insertion and UTF-16 ranges are independently asserted. Repeated results and actual resolve preserve items. Fixture fills complete snippets or the user-entered suffix of plain keyword/argument/field insertions; applied text preserves surrounding source and parses. |

Three exposed defects are corrected: type location now follows CST ownership and
direct argument separators; loop and pattern bindings become visible at their
HIR introduction boundary instead of after the enclosing construct; and a
resolved call's active parameter index follows the same selection as its
expected name/type. Enclosing HIR scopes still restrict lifetime and shadowing.
No parser, language or runtime semantics change.

Dynamic and missing receivers/modules have exact empty sets. Missing call facts
remain absent. Local/template candidates do not acquire lazy identities or
documentation. Existing owned-member matrices exclude private, missing and
wrong-owner candidates. Protocol tests prove observable projection, not access
to the service's internal analysis metadata.

This review does not certify generated combinations, scale, UX04 editor rendering
or snippet sessions, other feature groups, or fresh macOS execution. B03 remains
open and B16 stays deferred.
