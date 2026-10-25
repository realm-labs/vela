# Checkpoint C consumer contract changes

This is a removal-contract review, **not a new native capture or whole-batch
acceptance**. Historical checkpoints, platform audits, screenshots, and raw
VS Code evidence remain intact. They must not be presented as current proof for
contracts changed below. Unaffected native input routes retain their historical
accepted evidence; no captures, hashes, or successful command receipts have been
fabricated or refreshed by this change.

## Changed contracts requiring fresh evidence

- Task path/call/operand/eligibility/value/effect matrices now assert rejected
  source and absent builtin hover/signatures. Async callable and await-context
  matrices likewise assert removal, not successful synchronous reinterpretation.
  Their old admission/completion/async-metadata acceptance is stale.
- Ordinary callable ownership, traits, collections, callback references/factories,
  returned members, navigation, and schema lifecycle matrices use synchronous
  sources. Excluded asynchronous callback declarations/registrations are removed,
  not turned into additional accepted callbacks. Existing independent complete
  candidate, edit, owner, range, and lifecycle assertions remain.
- Tooling schemas are v3. Asyncness, task effects/metadata, and the task-only execution effect ceiling are absent;
  legacy versions and legacy fields reject, including false/sync/null values.
  Old v2 schema-dependent installed evidence needs recapture. This includes the
  changed `input-completion`, `input-workspace-roots`, and
  `input-workspace-schema` artifact contracts. Other native input fixtures and
  existing accepted snapshots are not rewritten.
- Literal `task::Error` no longer has a special builtin type identity. The hover
  type fixture asserts unknown parameter facts and an ordinary unresolved `Error`
  hint (`Any`, no builtin identity). It is diagnosed removed source, not an
  executable example.
- Selection fixtures may retain async/await/task spellings only as nonexecuting
  CST recovery input. They now explicitly require `syntax::removed_async` or
  `syntax::removed_task` diagnostics while preserving full selection ancestry,
  immutable snapshots, UTF-16, and LF/CRLF assertions.
- Signature S10's five former builtin task success queries move to the dedicated
  rejection matrices. Ordinary source function aliases remain ordinary callable
  facts, without a compiler-owned task capability. Unrelated task-namespace
  schema decoys cannot stand in for deleted builtin facts.

## Validation ownership

Focused Rust, Node/editor package, site, examples, and fuzz commands must be
reported from actual runs. Successful backend tests alone do not recapture
installed or native evidence. The parent owns `scripts/**`, root Cargo/lock, and
`docs/**`; the syntax fingerprints in the in-scope baseline have been reviewed and updated.
The remaining hard-coded async signature expectation at
`scripts/lsp-matrix/workspace-symbol-fixture.test.js:20` needs its parent owner
to update it before a fresh whole-matrix gate. No root scripts, status docs, or commits are part of this
consumer handoff.
