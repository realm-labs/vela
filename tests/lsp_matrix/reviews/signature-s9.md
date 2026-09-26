# Signature help: S9 recovery and unavailable schema review

Scope: four `signature-help/syntax/S9/{positive,negative}/{service,protocol}`
obligations and protocol states `recovery`, `unresolved`, `dynamic`,
`missing_schema`, `stale_schema`. Exact test identities are
`signature::import_tests::signature_recovery_matrix_preserves_known_facts_and_clears_unavailable_schema`
and `tests::signature::imports::signature_recovery_matrix_preserves_known_facts_and_clears_unavailable_schema`.

`signature-s9` independently authors 14 complete signature templates and 44
queries through 13 sequential phases: 572 positions per layer and line ending.
Both drivers execute LF and CRLF, repeat complete results and compare each phase
with a fresh database/server. Expected signatures, parameter names/types,
required/defaulted labels, active parameters, owners and null boundaries are
authored before querying providers. Service also checks named argument policy.
Protocol requests use independently projected UTF-16 positions and isolated
encoded Chinese/space/percent paths, initialization and typed notifications.

| Partition | Independent proof |
|---|---|
| Incomplete calls | Empty, second, named and multiline argument positions retain exact known signatures. Missing expressions, reordered named arguments, fake delimiters in Unicode strings/comments and a nested incomplete math call select the authored active parameter. |
| Parser recovery | Malformed declarations before/after a valid call, an incomplete builtin type hint and incomplete source/schema/builtin/member calls preserve known facts. Each static query also independently asserts whether parser diagnostics exist. A malformed unrelated file cannot contaminate healthy calls; repair restores it. |
| Receiver ownership | Imported records, inherent methods, implemented trait defaults, direct trait receivers and source/schema factory returns retain complete owner signatures while the outer call remains unclosed. Source functions with unavailable type hints or absent annotations retain known callable names and explicit unknown parameter/return facts. |
| Null boundaries | Missing/partial callees, modules and members, member/type positions without calls, shadowing noncallable locals, closure bindings, Any or unknown receivers/returns, positions after completed calls and empty/trivia files return null. The misspelled `Array.frist` produces a real diagnostic repair candidate `first` while signature help remains null; candidates never become callable facts. Protocol opens this closed file to inspect its published diagnostic and closes it back to disk. |
| Unsaved recovery | The opened caller changes from valid to incomplete known call, then unresolved call, then repairs and closes. Known signatures survive recovery; the unresolved spelling is null; close restores disk facts. Open/change/close preserve physical disk bytes. |
| Unavailable schema | Physical schema deletion, stale hash, invalid JSON and unsupported format remove all affected schema function/member signatures and degrade a known source function's schema-dependent hints to unknown. Unrelated source and builtin signatures remain exact. Each phase asserts the expected schema diagnostic category and degradation contract. |
| Schema replacement | A valid replacement changes parameter names/types and async signature metadata. All affected calls have full replacement expectations; restoring the original schema restores every baseline result. Actual created/changed/deleted watched-file event kinds are used. |
| Fresh equivalence | Every phase additionally rebuilds a service database or independently materializes and initializes a new server with current overlays. Full results agree with incremental execution after independently checking authored expectations. Final results and disk bytes equal the initial baseline, with all overlays closed. |

The matrix exposed an incomplete factory-returned member call returning null
despite a known receiver type. The cursor was beyond the recovery HIR call's
end, in trailing trivia after a missing argument. Shared cursor refinement now
selects the smallest HIR call containing the scanner's actual active opening
bracket within the lexical body and its ancestors. The cursor need not remain
inside a recovery span. Nested calls and unknown receivers remain independently
checked; this does not invent receiver types or change syntax/runtime semantics.
The accepted S8 matrix and shared query-context tests are regression targets.

This review does not certify S10/S11, client capability variants or installed
signature UX. Those remain separate B07 work. Acceptance requires fresh evidence
from one registered platform; this Windows run cannot supply macOS evidence.
