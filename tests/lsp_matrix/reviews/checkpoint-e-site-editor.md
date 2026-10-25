# Checkpoint E site/editor cleanup — scoped evidence only

Base: D `226900e81`. Write scope: `site/**`, `editors/**`,
`tests/lsp_matrix/**`. No commits, new runtime features, replacement objects,
conversion APIs, or native captures. This is not E repository acceptance.

## Changes

- English/Chinese Map, Set, set-method and type-hint chapters describe actual
  `ValueKey` semantics: immutable keyable leaves by value; supported script heap
  values by identity; no HostRef identity keys or PathProxy runtime values.
  Existing Rust conversion produces script-owned data without implicit write-back.
  Related capture, array lookup, assignment, reflection and validation claims
  no longer advertise removed access. No opaque support is promised.
- Existing host path/lifetime/write-through URLs remain; both navigation
  locales label them as removal notes. The Chinese write-through note is translated.
- `test:docs` now runs targeted reduced-surface regressions after the placeholder
  check. It checks all eight key-policy chapters, retained chapters and runnable
  examples, and bilingual navigation. Wrapped English claims and former Chinese
  claims are explicit negative controls.
- Tree-sitter removes the positive extern-state production. Generated grammar,
  node types and C parser are refreshed with CLI **0.25.10**. The new corpus pins
  ordinary initialized state, removed extern-state error recovery, and malformed
  ordinary-state recovery; existing LSP rejection/recovery fixtures are untouched.
- Zed, VS Code and site lexical grammars no longer advertise extern/async/await
  keywords. Ordinary state captures retain correct keyword/name groups. The site
  Shiki check includes actual removed-keyword probes. The consumer guard checks
  required initialized state and both TextMate capture maps.
- The existing consumer guard incorrectly required syntax-only diagnostic codes
  for removed type hints. It now pins the existing `hir::removed_host_type_hint`
  and `syntax::removed_host_borrow` codes, without changing fixture semantics.

## Zed checkout investigation and safety

`editors/zed/grammars/.gitignore` ignores `vela/` and `vela.wasm`. The local
`grammars/vela` is a generated Git clone whose origin is this repository, at
historical `621ee83`, **with substantial local modifications**. It is not safe to
delete or reset. No clone content, Git state, generated WASM or user work was
removed, cleaned, updated or recaptured.

The old validator rejected any existing checkout, contradicting the README's
build/cache contract. It now rejects duplicate sources tracked in the owning
repository while allowing an ignored local checkout. Three isolated temporary
repository tests cover an absent cache, an ignored nested repository with staged
and unstaged local work, and tracked duplicate rejection without deletion.

Once that failure was fixed, validation exposed a stale `@constant.builtin`
requirement absent from both the actual query and shared capture table. That
obsolete requirement is removed, not satisfied by a fake capture. Compiling the
actual query also exposed a nonexistent `let_statement.name` field; the redundant
invalid pattern is removed in favor of the existing binding-pattern capture.
Highlight, outline, indent and textobject queries compile against the new parser.
The local ignored checkout remains historical; this is not installed Zed proof.

## Validation actually run

| Command / check | Result |
| --- | --- |
| `npx --yes tree-sitter-cli@0.25.10 generate` in `editors/tree-sitter-vela` | Passed; a second generation was byte-identical for grammar JSON, node types and C parser. |
| `npx --yes tree-sitter-cli@0.25.10 test` | **3 passed**, zero failed, including explicit ERROR trees. |
| Tree-sitter parse of `site/src/syntax/fixtures/complete.vela` | Passed, no parse errors. |
| Tree-sitter query compilation/execution: Zed highlights, outline, indents, textobjects against the complete fixture | All four passed. |
| `npm --prefix site run test:docs` | Placeholder check plus **4 regression tests passed**. |
| `npm --prefix site run test:syntax` | **36 scopes / 36 token checks**, removed-keyword probes and ordinary state capture passed. |
| `npm --prefix site run build` | Passed; Astro **0 errors / 0 warnings / 0 hints**, **145 pages**. |
| `node editors/vscode/scripts/validate-package.js` | Passed. |
| `node editors/zed/scripts/validate-package.js` | Passed with the existing ignored checkout left intact. |
| `node --test editors/vscode/scripts/check-removed-consumers.test.js editors/zed/scripts/check-grammar-checkout.test.js` | **11 passed** (8 consumer / 3 checkout), zero failed. |
| `git diff --check -- site editors tests/lsp_matrix` | Passed. |

Generation still reports preexisting unnecessary conflicts; parse/query commands
report missing global parser-directory configuration but load the local grammar
and succeed. Build still reports a large Vite chunk and the missing docs/404
entry message; neither prevents the build. No new dependency was added.

## Acceptance boundaries

No installed VS Code suite, native editor interaction capture, cache/WASM refresh,
full Rust workspace gate, or strict LSP acceptance gate was run for this scoped
cleanup. Existing accepted unrelated evidence, hashes, baseline contracts,
semantic expectations and acceptance flags remain intact. Affected old native
captures for removed extern-state/schema/token surfaces remain **stale**, as
recorded at D; no hashes were rewritten to imply fresh acceptance. Parent-owned
final validation and status decide E closure.
