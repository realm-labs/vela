# Completion: S6 review

Scope: `completion/syntax/S6/{positive,negative}/{service,protocol}`. Exact test
identities live in the catalog; a fresh executed audit establishes verification.

| Partition | Shared evidence | Assertions |
|---|---|---|
| Enum patterns | `completion-enum-aliases` | Unit, tuple and record variants keep source/schema ownership through direct and namespace aliases. Qualified and unqualified candidates have exact sets, insertions, details, resolve and applied definitions. Private, missing, non-enum, duplicate and shadowed owners have explicit exclusions. |
| Record-pattern labels | `completion-pattern-fields` | Thirty LF/CRLF Unicode queries cover source/schema labels, shorthand, aliases, namespace aliases, empty and mid-token positions, ordinary/indexed for patterns and nested records. Exact candidates, symbols, details, byte/UTF-16 edits and resolve are checked in both layers. Applied source parses; explicit labels navigate to their declaration or null for metadata-only schema. Shorthand labels introduce an independently ranged local binding. Protocol edits use actual didChange and restore the original query. |
| Pattern boundaries | `completion-pattern-fields` | Used fields are excluded; the current label remains replaceable. Nested records use their nearest owner, and the outer owner resumes after a nested pattern. A field label cannot gain enum variants. The pattern after a colon remains eligible for variants, including an empty slot, without inheriting enclosing field labels. Private, missing, non-enum and duplicate owners do not borrow schema fields. Nine queries have exact empty sets. |
| Bindings and guards | `completion-visible-bindings` | Loop value/index, pattern body/guard and nested capture have exact visible scopes and local types. Iterator sources, a binding's own pattern and subsequent arms exclude premature or expired bindings; outer shadows are restored. Applied references navigate to independent binding markers. |
| Loop and match flow | `completion-loop-match-flow`, `completion-abrupt-flow`, `completion-local-exits` | Normal/continue backedges, break/return exits, nested loops, retained aliases, wildcard/binding arms and evaluated guards preserve exact reachable receiver owners. Unreachable tails and later irrefutable arms cannot contribute candidates. Applied member edits retain definitions and requery ownership. |
| Control-flow keywords | `completion-authoring-surface` | break/continue, for/for-in, if/match and return retain exact labels, insertion forms, edits and explicit completed-source parses, including nested method/lambda positions. Operand positions cannot acquire statement keywords. These templates are context suggestions; this review does not claim compiler legality for every enclosing control-flow construct. |

The new fixture exposes a missing completion path: record-pattern labels were
treated as general enum patterns. A focused CST query now finds the nearest
record and its direct labels, excludes already present fields, and delegates to
the existing source/schema field provider. Colon-separated value patterns keep
the enum-pattern path. This changes analysis-only authoring behavior; grammar,
binding semantics, execution and runtime admission are unchanged.

Shorthand completion metadata describes the field being inserted, while a
definition query on the completed shorthand names its newly introduced local
binding. The oracle asserts both identities explicitly. It does not substitute
the field's declaration range for the binding range.

This review does not certify S1/S2/S7, generated partitions, performance/scale,
UX04 native snippet/render routes or fresh macOS evidence. B03 remains open;
B16 stays deferred.
