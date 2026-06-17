# Vela Zed Extension

This package is a thin Zed launcher for the native `vela_lsp_server` binary.
It contributes Vela language metadata, the packaged `tree-sitter-vela`
grammar, Tree-sitter highlight/indent/outline queries, and starts the native
server over stdio. Language intelligence remains implemented by
`vela_lsp_server` and `vela_language_service`.

## Server Binary

Install `vela_lsp_server` on `PATH`, or unpack one of the native release
artifacts from the `LSP Release` workflow and expose the binary to Zed.

Project configuration still belongs in `vela.toml`:

```toml
[workspace]
roots = ["scripts"]

[host]
schema = "target/vela/schema.json"
```

## Validation

From the repository root:

```bash
node editors/zed/scripts/validate-package.js
cd editors/zed/grammars/vela
npx --yes tree-sitter-cli@0.25.10 generate
npx --yes tree-sitter-cli@0.25.10 parse --quiet ../../../../site/src/syntax/fixtures/complete.vela
```
