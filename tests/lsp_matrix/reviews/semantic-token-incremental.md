# B06 S11 semantic-token incrementality and cancellation review

This child reviews the eight delta/range positive/negative service/protocol S11
obligations and the delta stale-version/cancellation protocol state obligations.
The independently authored six-file fixture has fifteen states with
73 tokens per state across main, relay and an unrelated module. Both layers run
LF and CRLF, yielding thirty workspace states and ninety complete document streams
per layer. Markers supply source text, byte/UTF-16 coordinates, kinds and complete
modifier sets. Dense fixture data is shared by both layers to preserve a single
reviewable oracle; it never imports a provider or executes script/host code.

| Partition | Exact expectations |
| --- | --- |
| Repeated queries | Complete full/delta/range results and IDs agree on repeat. Queries do not parse, rebuild project/HIR, or advance the generation. Per-token, per-line and empty token-end/document-start ranges are independently checked. Every complete stream and token range agrees with a newly initialized workspace. |
| Body edits | An unknown member loses property/source ownership and repair restores it. Only main is reparsed/invalidated, and declaration/import fingerprints and the project index stay unchanged. |
| Declaration changes | Changes to dependency return types, main's return type and source fields change only the declaration fingerprint and update exact member roles. The edited document alone reparses; the index/HIR rebuild counts and complete invalidated module sets are explicit. A changed member role changes the result ID even when the consumer text is unchanged. |
| Import changes and reverse dependencies | Switching choose from api to other changes only the import fingerprint and invalidates main plus relay. Changes to each selected dependency invalidate direct/transitive consumers. After removing the other import edge, changes and repair in other invalidate only other; all three queried token streams remain the original streams. The unrelated module never joins any invalidation set. |
| Delta application | Deltas from the immediately preceding stream and the original, older stream apply exactly to current full data. No provider supplies an expected token. Complete repair restores original token streams and IDs. |
| Service cancellation and generation | Real full/delta/range values wrapped in service background tokens are accepted only for the current uncancelled generation. Cancellation before computation and after computation rejects them; every source edit rejects held old values. A pre-edit immutable database still returns the old complete stream. A new token permits subsequent results even when token content/ID did not change. |
| Protocol cancellation | Real queued full/delta/range tasks are cancelled before collection or after computation, including a concurrent source edit. They publish only RequestCancelled, without a result. Late/completed and unknown ID cancellation do not poison a later queued request; its decoded current stream is independently checked. |
| Protocol generation rejection | Held delta/range tasks from the old generation publish only ContentModified. Full retains one retry: no stale publication, current retry success, or ContentModified when the retry is invalidated again. A later queued request succeeds and its data agrees with an independent current workspace. |
| Document versions | Older/duplicate full and ranged didChange events cannot modify streams, IDs or cache/generation counters. Subsequent valid changes still yield exact current full/delta/range results. Encoded Chinese/space/percent URIs and non-BMP prefixes pin protocol coordinates. |

The lifecycle reproducer found that the previous latency/worker-named delta and
range dispatcher wrappers executed synchronously on the main loop. Delta now uses
the existing cancellable snapshot-task publication path on the latency lane;
range uses the worker lane. Cancelled or stale tasks cannot publish token data.
The coordinator regression keeps its projection assertions and now verifies the
actual scheduled lanes. Full's retry behavior and language semantics are intact.

This proof certifies those ten obligations. Other B06 state/editor cells,
scale and generated checks remain separate. B00-B05 accepted snapshots stay fixed;
macOS needs independent fresh evidence and B16 remains deferred.
