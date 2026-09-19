# Completion: S2 review

Scope: `completion/syntax/S2/{positive,negative}/{service,protocol}`. Exact test
identities are bound in the catalog; a fresh executed audit establishes verification.

| Partition | Shared evidence | Assertions |
|---|---|---|
| Nested default scopes | `completion-signature-scopes` | Eighty-eight LF/CRLF Unicode queries run the same scenarios in required trait signatures, ordinary function defaults, default trait methods and inherent methods. Lambda parameters, outer captures, typed/inferred block locals, nested captures, loop bindings and pattern guards have exact candidate kinds, types and independent declaration targets. |
| Scope exclusions | `completion-signature-scopes`, `completion-visible-bindings`, `completion-declaration-contexts` | Inner shadows win; outer bindings resume after closed blocks; initializers see the prior binding. Later locals/parameters, iterator-source bindings, expired loops and subsequent match arms are excluded. Eight new queries have exact empty sets. Required-signature globals and shadowed aliases retain distinct ownership. |
| Assignments and explicit hints | `completion-signature-scopes`, `completion-receiver-assignments` | All five compound operators preserve explicit i64 local candidates on their right-hand side and afterward. Ordinary assignments, branches, overwrites and retained aliases select current source/dependency member sets; scalar, erased and mixed owners exclude stale members. |
| Returns, blocks and branches | `completion-return-flow`, `completion-abrupt-flow`, `completion-local-exits` | Block/if/match results and explicit lambda returns preserve owned members. Return/break/continue and nested exits exclude unreachable tails and assignments; creating a closure does not execute its assignments. Terminated blocks and erased facts cannot borrow concrete members. |
| Loops and match flow | `completion-loop-match-flow`, `completion-visible-bindings` | Normal/continue backedges, break/return exits, nested loops, guarded arms and irrefutable patterns preserve reachable receiver owners and exact binding visibility. Later unreachable arms cannot contribute candidates. |
| Lambdas, closures and callbacks | `completion-callback-results`, `completion-callback-contracts`, `completion-sync-callbacks` | Direct lambda results, declared named callbacks, captures and callback slots keep owned candidates and function-value insertions. Dynamic Any/Closure, factories, erased results and async-in-sync positions have explicit negative boundaries. |
| Body authoring | `completion-authoring-surface` | let/return/if/match/for/break/continue templates retain exact labels, edits and explicitly completed source parses in nested blocks, methods and lambdas. Operand positions stay expression contexts. |

Both layers independently assert complete candidates, kind/detail, identity and
resolve policy, byte/UTF-16 edits, parsed applied source and marked definition
locations. Service queries repeat; protocol queries apply actual didChange and
restore the original source. Existing flow matrices additionally exercise
colliding source, dependency and schema owners. These are analysis assertions,
not execution or unrestricted type-inference claims.

Required trait signature defaults now have canonical analysis-only HIR expression
roots. The existing binder supplies parameters, nested scopes and captures; the
language service no longer maintains a separate parameter metadata fallback.
Equal-span body selection prefers the deeper body. At an expression's half-open
end, scope probing uses its identifier before any enclosing signature name.
HIR tests check root ownership, same-span lambda selection, shadowed binding IDs,
imported/qualified declarations in both source insertion orders, and metadata
indices after nameless recovery nodes. MIR rejects the new root as a runtime
compilation entry. Bytecode target tests check both whole-program and selected
function preparation: runtime lambdas remain present while signature lambdas
never enter their targets. Required methods remain bodyless.

CST parameter parsing limits an outer type annotation to the range before its
default. Direct typed-lambda defaults keep their parameter commas inside the
lambda list. The parser regression asserts lossless text, diagnostics, nested
type hints and parent-list ownership; this fixes existing syntax handling without
adding grammar productions.

This review does not certify S7, generated partitions, performance/scale, UX04
native snippet/render routes or fresh macOS execution. B03 remains open and B16
stays deferred.
