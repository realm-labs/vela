# Vela VS Code Extension

This package is a thin launcher for the native `vela_lsp_server` binary. It
registers the `vela` language ID, basic editor syntax metadata, user
configuration, and a stdio `vscode-languageclient` connection. Language
features remain implemented by `vela_lsp_server` and `vela_language_service`.

## Development Setup

Install dependencies from this directory:

```bash
npm install
```

Configure either:

- `vela.server.path` to point at an installed `vela_lsp_server` binary.
- A packaged binary under `server/vela_lsp_server` or
  `server/vela_lsp_server.exe`.

The extension passes `vela.workspace.roots` and `vela.host.schema` both as
native launch flags and initialization options. A project `vela.toml` remains
the authoritative workspace configuration when present.

## Validation

```bash
npm run validate
```
