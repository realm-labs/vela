# Completion Coverage Review

This is the current B03 semantic review index, not batch acceptance. The
machine-readable checkpoint and catalog remain authoritative. Fixture names below
refer to `tests/lsp_matrix/fixtures/`; test entry points are in the language
service's `completion/` modules and the server's `tests/` modules. A passing
fixture does not certify every combination in an S dimension.

## S5 Calls And Arguments

| Partition | Existing shared proof | Remaining review |
|---|---|---|
| Source calls, defaults and named arguments | `completion-named-arguments`, `call-parameter-mapping`, `call-argument-context`, `completion-call-expressions`: occupied/current/future slots, nested delimiters, labels versus values, active/expected parameters, edits. `completion-package-callables` checks same-path dependency functions/constants, exact applied signatures, named parameters and definition locations. `completion-package-members` checks qualified/aliased function returns. | Extend package identity checks to dependency lifecycle combinations and remaining returned-receiver forms. |
| Source methods and traits | `completion-members`, `completion-callable-hints`, `completion-callable-returns`: inherent/trait/default/Provider methods, hints and returned members. `completion-package-members`: exact sets for same-name types across packages; inherent/default methods, trait receiver signatures, applied parameter edits, resolved docs, definition targets and UTF-16 inlay positions; function/method returns and local bindings. `completion-return-flow`: required/default trait returns, awaited functions/methods, same-owner block/if/else-if/match/lambda results and negative erased/unit/unknown receivers. | `completion-receiver-assignments` checks reassignment, mixed-package branch/match/loop identities, shared field type unions and ambiguous field navigation. Shared-method ambiguity and abrupt exits remain to be reviewed. Source-backed docs and navigation must agree through dependency edits; remaining async restrictions still need review. |
| Schema/native calls and methods | `completion-named-arguments`, `completion-callable-hints`: explicit and legacy metadata, known parameter prefixes, unknown/Any/extra slots. | Review complete async/await and returned-receiver combinations with replacement/removal of metadata. |
| Stdlib calls and methods | `completion-stdlib-arguments`: registered names, imported paths, collection mutation variants, builtin owners and negative boundaries. | Final S5 review must consider both expression and parameter-name candidate sets, rather than treating named-argument proof as the whole call workflow. |
| Imports and unavailable owners | `callable-imports`, `completion-import-aliases`, `completion-expression-ownership`: source/schema/stdlib aliases, local/declaration shadowing, private/missing/duplicate owners. `completion-package-callables` checks direct dependency function/namespace aliases, `crate::` fallback and inaccessible/transitive declarations. `completion-package-members` distinguishes owned methods/returns from same-name direct/transitive and schema declarations. | Review remaining returned-receiver forms and dependency lifecycle combinations. |
| Service calls and authoring | `completion-service-calls`, `completion-service-paths`, `completion-service-roots`, `completion-service-parameters`, `completion-service-arguments`: unique/ambiguous origins, registered contracts, restricted contexts, schema changes and edits. | Retain these cases in the final S5/S8 mapping and lifecycle review. |
| Static scoped tasks | `completion-task-paths`, `completion-task-calls`, `completion-task-operands`: builtin operation ownership, unsupported paths/aliases, ordinary source aliases, positional-only outer operands, named inner calls, callable edits and continuation paths. | Broader target eligibility/ranking and async returned-member combinations remain part of final S5 review; this fixture does not claim them. |
| Dynamic/unresolved boundaries | The call, member and callable fixtures explicitly distinguish Any, unknown, missing owners, unsupported aliases and invalid arity. | Review all fixture families together before mapping the complete S5 negative cells. |

The scoped-task review exposed three concrete defects: unknown source/schema task
operations appeared as completions; literal task calls borrowed same-name source
signatures; and continuation completion inserted a call instead of a static
function path. The shared fixtures now exercise those contracts in both layers
under Unicode LF/CRLF. Source aliases remain ordinary source calls; importing a
builtin spelling does not create the compiler's lexical task capability.

The callable import runner now computes the post-edit query position from the
replacement start plus inserted length. Adding a nonempty prefix exposed the
previous extra prefix-length shift. This assertion repair preserves the exact
post-edit signature check.

## Remaining B03 Acceptance Work

1. Finish the async restriction and S5 partition review. The
   `completion-receiver-assignments` fixture adds twenty cases for sequential
   writes, retained aliases, if/match/loop joins and chained method returns with
   same-name package types. It checks complete sets, stable merged field details,
   signatures, parameters, inlays, edits and exact or ambiguous definition targets.
   Shared-method ambiguity and abrupt control-flow exits still need review; these
   cases do not certify every possible control-flow combination.
2. Complete the semantic review of every applicable syntax dimension, including
   item/statement/lexical/control-flow/recovery and async boundaries. S3/S4
   evidence already in the catalog does not close the remaining dimensions.
3. Complete dirty/close/reopen/dependency/schema/cancellation/stale-version
   sequences and required URI/client transformations against fresh workspaces.
4. Complete UX04 installed-workbench input/render routes. The existing input
   baseline does not substitute for all completion workflows.
5. Link exact compiled test identities and independent assertions for every B03
   obligation, then pass the selected profile's strict gate. Preserve B00-B02
   acceptance snapshots and run their current regression gate throughout.

B14 still owns the later overall partition completeness audit. B16 environment
expansion remains deferred; it cannot absorb missing local completion proof.
