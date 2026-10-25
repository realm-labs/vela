# E inventory join — reviewed contracts, not fresh acceptance

Base: D `226900e81`. No commits or parent docs/status edits. This follow-up uses
`tests/lsp_matrix/**` and the necessary `scripts/lsp-matrix/**` model/test scope.
It fixes inventory drift against the actual reduced E protocol and grammar;
removed language/host features are not restored to satisfy stale baselines.

## Exact migration

The immutable `execution-baseline.json` remains byte-identical, SHA-256
`bd00e36bb79cfb5efa01f3ea5875d7b40e138a94563afcab19d25cbce8f1cc06`.
All **1,327 semantic / 1,477 execution IDs and original owners remain**. No
feature applicability, polarity, layer, lifecycle/environment dimension, editor
route, exemption or batch deliverable was dropped. Feature files, interactions,
batch ownership and all old catalog assertions/test links are preserved and
hash-checked by the migration regressions.

`execution-manifest.json#migrations` records exact original and replacement
`id`/`owner`/`contractHash` arrays, reasons and retained behavior:

| Migration ID | Original → current contracts | Disposition |
| --- | ---: | --- |
| `checkpoint-e-s1-owned-state` | 105 → 105 | Bind reviewed S1 to initialized VM state and synchronous declarations; retire extern/async success claims, preserve ordinary declarations and rejection/recovery. |
| `checkpoint-e-s3-owned-values` | 73 → 73 | Bind reviewed S3 to registered owned Value schema facts; preserve primitive, builtin, source, trait, Any and unknown coverage, not Host/view success. |
| `checkpoint-e-inlay-script-members` | 46 → 46 | Review protocol row 35: host-path facts become script-member facts; preserve all other inlay positive/negative, state, environment and editor obligations. Includes the four inlay S3 cells, avoiding duplicate migration sources. |
| `checkpoint-e-retired-view-hover-proof` | 4 → 4, hashes unchanged | S10 semantics remain required. Retire only their old scoped-view test linkage as current proof; preserve historical names/assertions and record replacement tests as candidates, not certified evidence. |

Exactly **224 contract hashes change**; **1,253 remain unchanged**. The four
S10 records above are among those unchanged contracts. No execution baseline
hash was overwritten. The existing syntax-axis hashing hook now receives only
the explicitly reviewed S1/S3 contracts; other syntax axes retain their original
identities. A validator rejects an axis contract differing from its documented
dimension. This avoids unrelated wholesale rehashing while detecting drift in
these reviewed surfaces.

[The machine-readable journal](checkpoint-e-inventory.json) retains old/new
protocol and syntax text and hashes, the old grammar source fingerprint,
retired definitions/group assignments, reasons, exact pending evidence IDs and
checkpoint preservation hashes. The only protocol row edit is inlay's positive
`host-path type facts` → `script member type facts`; its dimensions and negative
contract are unchanged. S1 and S3 are the only changed dimension texts. Of the
four syntax source fingerprints, only `docs/grammar.ebnf` changes:

- Original: `bea64bde444fa2294f06ddbf307f8ebaa890d06ecdd52d871c6933f453bce833`
- Current: `c005493d1eefab75bd0fc6e1602077c17ee70ef05c3e3c44a2dd2d341617247d`

Exactly two positive grammar productions retire; all other productions retain
classification, leaving **131** current rules:

| Retired original ID | Original definition/group contract SHA-256 |
| --- | --- |
| `grammar/await_suffix` | `290b2b83f423b68d773e0fce3f42ce890fbea0401ca341dd68106aacc059601f` |
| `grammar/extern_state_decl` | `c6d74ed22f5533768d2c18424db89586383f32bceee3942492de28bac830e4aa` |

These hashes cover the journal's exact `{id, group, syntax, definition}` objects;
they are retired grammar contracts, not newly invented execution obligations.
Retained S9 negative IDs remain required. No malformed/rejection fixture was
removed or rewritten by this join.

## Historical evidence and checkpoint disposition

**158 catalog proof records are explicitly pending review**: 154 associated
with changed contracts, plus four S10 hover records referencing the retired
positive scoped-view tests. Their original assertions and exact test names stay
in place. Missing historical tests do not crash bare discovery for these
explicitly migrated records, but they also cannot satisfy the new contract.
Even an `ok` result for an old linked test cannot turn pending evidence green;
failed/ignored linked results still fail. Unrelated stale links still raise the
original hard error. Inventory loading checks every pending marker against its
exact manifest migration source and target.

B02–B08 reopen through the existing checkpoint mechanism, preserving each old
scope/hash/validation snapshot in `reopenedBatches`. **B00, B01 and B09 remain
accepted unchanged.** Profile audits/native artifacts, completed children and
existing open issues remain byte-equivalent as structured data. The prior
`B10.45` resume position is recorded in the journal; active work resumes at
**B02**, with **1,423 remaining execution obligations**. Later work was not
removed; it cannot bypass the earlier re-audit.

No installed/native captures, profile acceptance, fixture hashes or archived
native evidence were refreshed. Affected native evidence remains stale.
Parent-owned full gates/status still determine E closure.

## Validation actually run

- Direct `loadInventory` and checkpoint validation: passed; 1,327 semantic /
  1,477 execution obligations.
- Model/execution/checkpoint/migration tests: **44 passed**, zero failed.
- `node --test scripts/lsp-matrix/*.test.js`: **305 passed**, zero failed or
  skipped, including four new inventory migration regressions and two model
  regressions. They detect dropped IDs/owners, unrecorded or forged migrations,
  changed historical proof/native snapshots and stale-proof promotion.
- `node scripts/lsp-matrix/run.js`: **exit 0**, including actual service/protocol
  test discovery, stdio-test discovery, server build and live capability probe.
  This bare audit does not execute the Rust suites or installed editor routes.
  It reports **41 protocol rows / 131 grammar productions** and semantic counts:
  **0 verified / 872 mapped / 453 unreviewed / 0 failed / 2 N/A**.
- Execution assessment: **877 mapped / 598 unreviewed / 2 N/A / 0 verified /
  0 failed**. These report-time states are distinct from the 1,423 obligations
  remaining in the checkpoint's pending/reopened batches.
- Scoped `git diff --check` and JavaScript syntax checks: passed.

Audit reports/logs are under `target/lsp-matrix/run-*`; the root report points
to the latest completed invocation. Intermediate attempts exposed and then fixed
the stale scoped-view test links and an order-sensitive regression assertion;
they are not reported as successful final audits. No `--accept`, strict green
claim, fresh native capture or full workspace acceptance is claimed.
