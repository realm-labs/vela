# Completion: S13 review

Scope: `completion/syntax/S13/{positive,negative}/{service,protocol}`. The
catalog binds exact tests below; an executed audit determines verification.

| Partition | Evidence | Assertions |
|---|---|---|
| Container type hints | `completion-authoring-surface`, `completion-type-positions` | Nested Array/Map/Set/Option/Result hints retain exact semantic details, labels and edits. Type-position completion covers compact nested source hints. Semantic TypeFact display deliberately uses `Array(Option(i64))`-style notation; angle-bracket source formatting belongs to formatting feature tests and is not redefined by completion. |
| Empty-prefix typed dot | `completion-authoring-surface`, `completion-members`, `completion-package-members` | Exact complete method inventories for Array, Map, Set, String, Bytes, Iterator, Option and Result; every builtin candidate has its own identity, insertion and empty resolve policy. A representative zero-argument method per family is applied, parsed and queried again. Source fields, inherent and trait/default methods and schema fields/methods have separate exact owner, detail, edit and target proof. |
| Field declarations | `completion-authoring-surface`, `completion-type-positions` | Empty and partial field-name positions offer exactly the two field templates. Placeholder text, insertion format, range and completed field source are exact. After `:` only type candidates apply. Default expressions cannot acquire field templates; their legitimate reflection functions remain explicit in the complete candidate set. |
| Labels and details | New authoring fixture plus existing type-position and type-ownership matrices | Label, lookup/filter text, detail and labelDetails are separate assertions. Source/schema type labels retain owner descriptions and actual usable insertions; applied references keep exact owners. Template/local candidates lack a lazy identity; actual protocol resolve preserves them. |
| Statement templates | `completion-authoring-surface`; service boundary test | `for in` and `match` retain exact numbered placeholders and final stops. All supported statement keywords, top-level declaration templates and both field templates are completed with explicit fixture fills and parsed. Requery checks the relevant complete template inventory. Nested blocks, methods and lambdas keep keyword-prefix completion; expression operands and call arguments cannot become statement positions. |

The shared fixture has 34 queries, each run with LF and CRLF and Unicode before
the cursor. Exact candidate order is asserted for the template/parameter probes;
the eight builtin dot inventories list their complete ordered labels, including
negative exclusion of unsupported flatten on non-nested Option/Result. Initial
edits use independent byte/UTF-16 markers. Completed source is specified by the
fixture; applying an edit preserves surrounding text. Template bodies contain LF
while surrounding CRLF remains intact. This models protocol edits, not editor
indentation or snippet-session behavior.

Every builtin dot candidate has an asserted symbol and edit; representative
methods additionally have exact details and applied-source checks. In a return
prefix query the legitimate `reflect::returns` candidate is explicitly retained
alongside the return keyword. Field-default negatives list `reflect::field` and
`reflect::fields` and assert their function kind and builtin identity; they are
not incorrectly claimed as empty results. Unknown items/fields, dynamic or unknown
receivers and operand keyword prefixes have explicit empty candidate sets.

Two defects exposed by this fixture are corrected: top-level trivia must not hide
the previous declaration boundary, and statement keyword prefixes must work in
nested statement lists, including methods and lambdas. Statement recognition uses
the first significant token and excludes operands after it. Ordinary expression
prefixes retain their expression context. Two existing assertions at `f` and at
the end of `return` now expect Statement; their original value/cancellation
assertions remain, and the former also requires the for-in snippet.

This review does not certify other syntax groups, UX04 rendering/native snippet
acceptance, generated combinations, scale gates, or fresh macOS execution. B03
remains open.
