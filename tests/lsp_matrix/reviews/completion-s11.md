# Completion: S11 review

Scope: the four `completion/syntax/S11/{positive,negative}/{service,protocol}`
obligations. The catalog binds the tests below to their owning layers. Executed
audit results, rather than this review alone, determine verification status.

| Partition | Evidence | Assertions |
|---|---|---|
| Repeated queries | Shared `completion-incremental-ownership` fixture; service `requests_in_one_generation_share_a_single_analysis_facts_build` | Repeated whole completion lists match. Queries and resolve leave parse, project, HIR and generation counters unchanged. The separate memoization test bounds analysis-facts builds across mixed requests in one generation. |
| Body-only edits | Shared fixture, first two transitions | Switching a receiver to a different record and restoring it changes the exact owned field candidate without changing declaration/import fingerprints or rebuilding the project index. Exactly one document is reparsed and HIR rebuild count increases by one; only the edited module is invalidated. |
| Declaration/import fingerprints | Shared fixture, remaining six transitions | Import switches and return-type changes alter only the specified fingerprint. Project index rebuild count increases by one. Removing an imported callable or selecting a missing module yields an exact empty set; restoration returns only the current owner's field. |
| Reverse dependencies | Shared fixture | Independently specified exact invalidated module sets include direct and transitive dependents when required. Unrelated modules remain excluded. Service also checks HIR and analysis invalidation sets agree. |
| Cancellation | Shared service fixture; protocol `cancelled_completion_and_resolve_publish_only_errors_and_allow_later_requests` | Cancelled service tokens yield no result and do not poison a fresh token. Protocol tasks cancelled before or after computation publish only the cancellation error; later requests, including after late/unknown cancellation, return current owned facts. |
| Generation rejection | Shared service fixture; protocol `stale_completion_and_resolve_retry_current_facts_and_bound_repeated_changes` | Every edit invalidates the previous service token. Protocol publication suppresses stale facts, retries with current facts once, rejects a repeatedly invalidated retry, and permits subsequent requests. |
| Document versions | Protocol `old_and_duplicate_versions_preserve_completion_and_signed_edit_versions` | Old and duplicate full/ranged changes cannot alter current completion or cache counters; signed version boundaries and close/open lifetime are explicit. |

The new fixture has an initial state and eight successive mutations: seven
candidate states and two empty states. Both drivers run LF and CRLF, with Chinese
and a non-BMP character before the completion cursor. The protocol driver uses
actual didOpen/didChange notifications and completion/resolve/definition requests.
Every state is compared with an independently initialized analysis/server. Each
candidate has an independently specified kind, detail, source identity, byte or
UTF-16 edit and marked definition target. Applied edits parse, navigate to the
exact field, and retain its owner on a subsequent completion request.

The source field candidates have no documentation; actual protocol resolve
preserves their whole item. This is not a new documentation capability. Cancellation
and stale-publication proof comes from the deterministic task tests, not inferred
from the sequential fixture. This review does not certify other syntax groups,
UX04 editor interaction, generated combinations, scale gates or a fresh macOS run.
B03 remains open.
