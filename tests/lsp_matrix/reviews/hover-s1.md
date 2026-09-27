# Hover S1 top-level declarations

Scope: `hover/syntax/S1/{positive,negative}/{service,protocol}` only. Exact tests:
`hover::declaration_matrix_tests::hover_declaration_matrix_preserves_complete_metadata_and_static_parameters`
and `tests::hover::declarations::hover_declaration_matrix_preserves_complete_metadata_and_static_parameters`.

`hover-s1` independently authors 90 queries: 70 complete hovers and 20 explicit
null results. Positive tokens run at their start and interior, negative tokens
at their start. LF/CRLF and present/physically absent schema variants yield 640
positions per layer, each repeated. Expected metadata and Markdown are authored;
markers independently supply byte/UTF-16 ranges and local declaration locations.
Service asserts complete labels, kinds, details, docs, ranges and canonical
source/local identities. Protocol asserts the whole Markdown object and range,
including explicit JSON null, through real initialization/open/request dispatch.

| Partition | Independent expectations |
|---|---|
| Functions and parameters | Public/private, async, zero-parameter/no-return-hint and untyped functions; typed/defaulted/untyped parameter declarations and references. Untyped parameters remain unknown. Function hover retains its existing hint-based signature format; signature-help owns default markers. |
| Const and state | Public/private const, VM state and extern state, with present/absent docs and exact ownership text. Imported const/state/extern-state uses retain the defining identity. |
| Structs and enums | Source type docs, typed/Any struct fields, documented unit/tuple/record variants, tuple/record field declarations and cross-file field writes/variant expressions. Every variant and field has its precise owner rather than an enclosing enum or similarly named schema type. |
| Traits and impls | Documented interface/default methods, inherent/async/trait impl methods, impl trait/target headers, static interface parameters, default/inherent receiver parameters and method-body parameter references. Receiver types follow the trait/impl owner, and local identities point to exact declaration tokens. |
| Imports and modules | Individual aliases at import declaration and use, namespace aliases, qualified function paths and selected module prefixes. Alias hover names the original declaration; module identity retains the package prefix required by the existing source-module contract. |
| Negative boundaries | `pub`, `use`, `const`, `state`, `extern`, `fn`, `struct`, `enum`, `trait` and `impl` keywords and a literal have no symbol hover. Any members and unqualified/qualified missing names return null. A private source function or its inaccessible import target/alias cannot borrow a conflicting schema fact; same-module private uses remain available. |

The initial authored tuple-field case reproduced a null hover despite HIR field
metadata. Later cases exposed omitted impl docs, impl-header type hovers,
required interface parameters, method-scope bindings, unknown receiver parameter
types, import alias declarations, namespace paths, module identities and private import fallback. Repairs
use exact declaration tokens, current `QueryContext` bindings and scoped static
HIR/schema facts. Qualified expressions respect the selected segment's canonical
identity and own unresolved results; source visibility blocks registry fallback. Direct/aliased private import
declarations and uses return null; unresolved bindings cannot fall back to
short-name registry metadata or a placeholder hover.
No executable body is manufactured for required trait methods. The former S3
test identities and assertions are preserved through shared fixture drivers.

Encoded isolated roots contain Chinese, spaces and percent signs. These S1
positions have ASCII token prefixes; earlier accepted coordinate tests own the
same-line non-BMP/UTF-16 proof. Protocol verifies that all physical fixture inputs
remain unchanged. Other hover syntax/state and UX10 input/render obligations
remain open; Windows/macOS acceptance requires independent fresh evidence from
one complete registered profile. B16 remains deferred.
