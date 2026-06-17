# Vela Zed Extension

This package is a thin Zed launcher for the native `vela_lsp_server` binary.
It contributes Vela language metadata, the packaged `tree-sitter-vela`
grammar, Tree-sitter highlight/indent/outline queries, and starts the native
server over stdio. Language intelligence remains implemented by
`vela_lsp_server` and `vela_language_service`.

The Tree-sitter grammar source lives outside this extension directory at
`../tree-sitter-vela`. Zed checks grammar repositories out under
`grammars/<name>` while compiling a dev extension, so `grammars/vela` is kept
as an ignored build/cache directory rather than a source directory.

## Server Binary

For local development, point Zed at a locally built server binary with a
workspace `.zed/settings.json` file:

```json
{
  "lsp": {
    "vela-language-server": {
      "binary": {
        "path": "/absolute/path/to/vela/target/debug/vela_lsp_server",
        "arguments": ["--stdio"]
      }
    }
  }
}
```

The extension also accepts the shorter compatibility key `lsp.vela.binary`.
If neither setting is present, install `vela_lsp_server` on `PATH`, or unpack
one of the native release artifacts from the `LSP Release` workflow and expose
the binary to Zed.
Zed compiles this Rust extension before launching the server, so local dev
installs also need the Rust WASI target used by the installed Zed version:

```bash
rustup target add wasm32-wasip1
```

Recent Zed/Rust toolchains may use `wasm32-wasip2`; installing both targets is
safe when switching between Zed channels:

```bash
rustup target add wasm32-wasip2
```

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
cd editors/tree-sitter-vela
npx --yes tree-sitter-cli@0.25.10 generate
npx --yes tree-sitter-cli@0.25.10 parse --quiet ../../site/src/syntax/fixtures/complete.vela
```
