# Vela Zed Extension

This package is a thin Zed launcher for the native `vela_lsp_server` binary.
It contributes Vela language metadata and starts the native server over stdio.
Language features remain implemented by `vela_lsp_server` and
`vela_language_service`.

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
```
