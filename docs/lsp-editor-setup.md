# Native LSP Editor Setup

This guide documents manual setup for editors that can launch a generic
Language Server Protocol server. It is intentionally editor-neutral: feature
behavior belongs in `vela_language_service` and `vela_lsp_server`, while editor
configuration only starts the native binary and passes workspace settings.

The current pre-release setup runs from a Vela source checkout. Packaged
editor extensions and release binaries are separate Phase 17 deliverables.

## Build Or Run The Server

From the repository root, build the native server:

```bash
cargo build -p vela_lsp_server
```

The resulting binary is:

```text
target/debug/vela_lsp_server
```

For source-checkout development, editors can also launch Cargo directly:

```bash
cargo run -p vela_lsp_server -- --stdio
```

The server uses stdio transport by default. Passing `--stdio` is recommended in
editor configuration because it makes the transport explicit.

The server also supports:

```bash
vela_lsp_server --version
vela_lsp_server --stdio --root scripts --schema target/vela/schema.json
```

`--root` may be repeated. `--schema` may be supplied once. These flags seed
fallback configuration before LSP initialization; a discovered `vela.toml`
remains the authoritative project configuration.

## Packaged Binaries

The `LSP Release` workflow builds native `vela_lsp_server` artifacts for:

```text
vela_lsp_server-linux-x64.tar.gz
vela_lsp_server-macos.tar.gz
vela_lsp_server-windows-x64.zip
```

Each archive contains the native server binary plus this setup guide as
`README.md`. The workflow also emits a `.sha256` checksum beside every
archive. Tag pushes matching `v*` publish those archives as a GitHub Release;
manual workflow runs upload the same archives as workflow artifacts.

Editor integrations should launch the unpacked binary over stdio and pass
configuration through `vela.toml`, initialization options, or
`workspace/didChangeConfiguration`.

## VS Code Extension Package

The repository includes a thin VS Code extension package under
`editors/vscode`. It contributes the `vela` language ID, `.vela` file
association, syntax metadata, settings, and a `vscode-languageclient` launcher
for the native server.

For local extension development:

```bash
cd editors/vscode
npm install
npm run validate
```

Set `vela.server.path` to a built or downloaded `vela_lsp_server` binary, or
place a packaged binary under `editors/vscode/server/`. The extension passes
`vela.workspace.roots` and `vela.host.schema` through the same native flags and
initialization-option shape used by generic clients. It does not implement
language analysis itself.

## Zed Extension Package

The repository includes a thin Zed extension package under `editors/zed`. It
contributes Vela language metadata, the packaged `tree-sitter-vela` grammar,
Tree-sitter highlight/indent/outline queries, and starts `vela_lsp_server`
over stdio via Zed's extension command hook.

For local extension validation:

```bash
node editors/zed/scripts/validate-package.js
```

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
If neither setting is present, put `vela_lsp_server` on `PATH`, or unpack a
release artifact and expose the binary to Zed. Project roots and schema
configuration still come from `vela.toml`; the Zed package does not implement
Vela analysis or LSP request behavior.

## Project Configuration

Prefer a `vela.toml` at the workspace root:

```toml
[workspace]
roots = ["scripts"]

[host]
schema = "target/vela/schema.json"
```

Workspace roots are interpreted with `compile_dir`-style module semantics.
Open editor buffers override disk snapshots. The schema path points to a
static host artifact exported from `TypeRegistry`/`RegistryFacts`; the language
server does not run host code to discover schema metadata.

If no schema is configured or the schema is missing, syntax, module, stdlib,
and script-owned editor features still work. Host facts degrade rather than
failing editor requests.

## Generic Client Settings

Configure the client to start the server for `.vela` files over stdio.

When launching the built binary:

```json
{
  "command": "/absolute/path/to/vela/target/debug/vela_lsp_server",
  "args": ["--stdio"],
  "filetypes": ["vela"],
  "rootPatterns": ["vela.toml", ".git"]
}
```

When launching through Cargo from a checkout:

```json
{
  "command": "cargo",
  "args": ["run", "-p", "vela_lsp_server", "--", "--stdio"],
  "filetypes": ["vela"],
  "rootPatterns": ["vela.toml", ".git"]
}
```

If an editor supports initialization options, it may pass the same fallback
settings used by the native flags:

```json
{
  "workspace": {
    "roots": ["scripts"]
  },
  "host": {
    "schema": "target/vela/schema.json"
  }
}
```

Clients that support `workspace/didChangeConfiguration` may send the same
shape after startup. The server reloads configured schema artifacts and
invalidates project-derived indexes when those settings change.

## Editor Notes

Use the editor's generic LSP mechanism:

- Set the language ID or filetype to `vela` for `*.vela`.
- Use stdio transport.
- Use the workspace folder containing `vela.toml` as the LSP root.
- Let the server handle diagnostics, completion, hover, definitions,
  references, rename, code actions, semantic tokens, formatting, and inlay
  hints through LSP requests.

Editor-specific packages should stay thin launchers and configuration UI. They
should not reimplement Vela analysis, read live host state, run Vela programs
for editor features, or mutate schema/type metadata.

## Troubleshooting

If the server starts but host-aware completions or hovers are missing, check
that `host.schema` points to an existing schema artifact and that the editor
started the server with the expected workspace root.

If imports resolve differently from the command line, check `workspace.roots`
in `vela.toml`; the editor should open the folder that contains that file.

If a generic client does not support dynamic file watching, reopen the
workspace after changing `vela.toml` or the schema artifact. Clients with
watcher support receive dynamic registrations for `.vela` sources,
`vela.toml`, and the configured schema artifact.
