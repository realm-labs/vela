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
| Source methods and traits | `completion-members`, `completion-callable-hints`, `completion-callable-returns`: inherent/trait/default/Provider methods, hints and returned members. `completion-package-members`: exact sets for same-name types across packages; inherent/default methods, trait receiver signatures, applied parameter edits, resolved docs, definition targets and UTF-16 inlay positions; function/method returns and local bindings. `completion-return-flow`: required/default trait returns, awaited functions/methods, same-owner block/if/else-if/match/lambda results and negative erased/unit/unknown receivers. | `completion-receiver-assignments` checks reassignment, mixed-package branch/match/loop identities, shared field type unions and ambiguous field navigation. Shared-method combinations are indexed below; abrupt exits remain to be reviewed. Source-backed docs and navigation must agree through dependency edits; remaining async restrictions still need review. |
| Schema/native calls and methods | `completion-named-arguments`, `completion-callable-hints`: explicit and legacy metadata, known parameter prefixes, unknown/Any/extra slots. | Review complete async/await and returned-receiver combinations with replacement/removal of metadata. |
| Stdlib calls and methods | `completion-stdlib-arguments`: registered names, imported paths, collection mutation variants, builtin owners and negative boundaries. `completion-sync-callbacks`: 36 cases for static sync function references, async exclusions, named/reordered slots, dynamic values, canonical resolve/definitions, reflection signature alternatives and ordinary nested calls. `completion-callback-factories` checks existing calls. `completion-callback-contracts` and `completion-callback-results` retain declared signatures and distinguish direct-lambda result facts from erased callback results. | Review remaining contextual parameter/return combinations together with the complete S5 call workflow; these completion tests do not certify runtime callback admission. |
| Imports and unavailable owners | `callable-imports`, `completion-import-aliases`, `completion-expression-ownership`: source/schema/stdlib aliases, local/declaration shadowing, private/missing/duplicate owners. `completion-package-callables` checks direct dependency function/namespace aliases, `crate::` fallback and inaccessible/transitive declarations. `completion-package-members` distinguishes owned methods/returns from same-name direct/transitive and schema declarations. | Review remaining returned-receiver forms and dependency lifecycle combinations. |
| Service calls and authoring | `completion-service-calls`, `completion-service-paths`, `completion-service-roots`, `completion-service-parameters`, `completion-service-arguments`: unique/ambiguous origins, registered contracts, restricted contexts, schema changes and edits. | Retain these cases in the final S5/S8 mapping and lifecycle review. |
| Static scoped tasks | `completion-task-paths`, `completion-task-calls`, `completion-task-operands`, `completion-task-eligibility`: builtin ownership, unsupported aliases, positional outer operands, nested ordinary calls, static async worker and synchronous matching continuation sets, qualified/imported ownership, existing parentheses and exact applied diagnostics. `completion-task-values`, `completion-task-effects`, `completion-task-spawn-ceiling`, `completion-task-unknown-ceiling`: 36 applied-edit cases for owned/erased arguments, nested callable/iterator/HostRef rejection, parameter/result spans, per-capability ceilings, recursive worker/continuation effects and runtime-only resume parameters. | Review defaulted/opaque returned values, method effect chains, schema replacement and ranking; static target and diagnostic proof does not certify runtime admission or host resume binding. |
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

1. Finish the S5 partition review and remaining callback boundaries. The
   `completion-sync-callbacks` fixture covers static sync callback reference
   insertion across builtin collection/Option/Result/Iterator methods, named and
   reordered slots, same-name source/schema/import owners and exact applied
   definitions. Dynamic callbacks remain dynamic; ordinary source/schema methods
   do not acquire a guessed sync-only callback contract.
   `completion-callback-factories` adds 21 contexts and 82 candidate applications:
   existing argument lists, named/nested arguments, comments/newlines, aliases,
   source/schema methods, async factories and reflected Any returns. It asserts
   exact edits, definitions, ownership and await diagnostics while retaining
   direct reference and new-call policies. Four `completion-callback-contracts`
   cases apply 32 choices across Array/Map filters, Option fallback and Iterator
   fold. Zero, one, two and defaulted parameters retain their declared signatures;
   different return types do not acquire an invented compatibility filter. Known
   async functions remain excluded; Any/Closure parameters remain plain values.
   Fourteen `completion-callback-results` cases distinguish direct expression/block
   lambdas with proven record returns from named callbacks, factories, bound local
   closures and erased returns through eager and iterator map chains. Exact result
   facts, candidate sets, edits, resolve and source field definitions are checked
   under Unicode LF/CRLF. Named callbacks currently do not specialize the map
   result from their declared return signature: this records the conservative
   direct-lambda analysis boundary, not a runtime compatibility guarantee.
   `completion-callback-slots` adds sixteen result/parameter queries for
   positional, named, reordered and mixed fold arguments; two lambdas in distinct
   slots; and unknown, duplicate, extra or misplaced arguments. A red case showed
   reordered fold arguments losing their initial fact. Analysis now orders facts
   by registered parameters and specializes only the actual callback lambda.
   Unrelated initial lambdas cannot fabricate result members; a real callback
   keeps its item/accumulator facts. Both layers assert complete member sets,
   exact edits, resolve ownership and field definitions under Unicode LF/CRLF.
   Twenty-four
   `completion-task-eligibility` cases verify complete static
   target sets, async metadata, canonical resolve ownership, LF/CRLF Unicode edits
   and applied diagnostics. Unknown/dynamic workers do not invent an outcome type;
   continuation choices still require static synchronous functions. Nested ordinary
   expressions keep their ordinary completion policy. Five existing task-call
   cases now explicitly exclude dynamic local operands while preserving builtin
   signature and positional-argument assertions. The
   `completion-async-callables` fixture checks eighteen awaited/unawaited source,
   alias, trait, host and schema cases with opposite asyncness in same-name
   package declarations. Completion details preserve asyncness; applied calls
   retain exact signatures, named parameters, inlays, definitions, missing-await
   diagnostic ranges and labels under Unicode LF/CRLF.
   `completion-await-contexts` adds 25 contexts and 58 candidate applications
   across source/dependency/schema functions, methods and traits. Sync functions,
   lambdas and sync methods retain exact invalid-await diagnostics; async methods
   and blocks retain no await error. Parenthesized operands distinguish the
   operand rejection from a still-unawaited inner async call. Complete candidate
   sets, ownership, applied signatures, named parameters, inlays and definitions
   remain asserted with explicit parser and diagnostic range/message oracles.
   Broader callback combinations and remaining opaque/defaulted task values,
   method effect chains, schema replacement and host resume contracts still
   require review. Thirty-six task-admission cases now retain complete candidate
   sets through application, assert exact rejection messages, codes, severity,
   ranges and related locations, and restore protocol completion results after
   edits. Owned/Any arguments and absent effect ceilings have explicit negative
   diagnostic proof; trailing resume parameters are not detached worker inputs. The
   `completion-receiver-assignments` fixture adds twenty cases for sequential
   writes, retained aliases, if/match/loop joins and chained method returns with
   same-name package types. It checks complete sets, stable merged field details,
   signatures, parameters, inlays, edits and exact or ambiguous definition targets.
   `completion-shared-methods` adds eighteen contexts and 45 candidate applications
   for same-name inherent/default methods and returned receivers across root,
   dependency and inaccessible/schema collisions. It checks deterministic merged
   details, complete signature alternatives (including identical labels from
   distinct owners), exact edits, resolve and source definitions or null targets.
   Known branches of a source/Any union remain authoring candidates, but incomplete
   origins cannot prove a unique navigation target; that uncertainty survives
   field and method return chains. Pure Any and unresolved returns supply no
   invented members. Schema signatures cannot replace a source branch of a union.
   The shared protocol runner now actually requests and checks null definitions,
   matching the service oracle, and locates the callee independently of qualified
   paths inside inserted arguments. `completion-abrupt-flow` adds seventeen
   contexts and twenty candidate applications for unreachable tails after
   return/break/continue, nested exits, one/all returning branches, match arms and
   explicit lambda returns. Ordinary block results exclude exits; invocation
   results include reachable explicit returns and fallthrough values. The shared
   HIR traversal also supplies control flags, respects argument evaluation order,
   consumes loop break/continue exits and keeps nested lambda returns separate.
   Independent analysis assertions cover thirteen scalar/control-flow cases.
   MIR checks preserve exits from iterable evaluation; nested-loop tests assert
   that iterable break/continue target the enclosing loop without an inner iterator.
   `completion-local-exits` adds twenty contexts and forty candidate applications
   for assignment owners after return/break/continue, nested loops, iterable exits,
   short-circuiting, match arms/guards and lambda creation. Both runners check
   exact sets, metadata, applied edits, signatures, parameter/inlay contracts and
   definition ownership under Unicode LF/CRLF. Twenty-five independent scalar
   cases assert surviving values, retained loop exit writes, guard evaluation
   order and ignored unreachable arguments. The local walk joins only reachable
   successors and snapshots loop exits before later statements can overwrite them.
   Loop-carried state and match-result reachability still need review; these cases
   do not certify every possible control-flow combination.
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
