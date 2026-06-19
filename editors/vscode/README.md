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

## Language Server Profiling

To diagnose editor stalls caused by the native language server, enable request
profiling in VS Code settings:

```json
{
  "vela.server.profile.enabled": true,
  "vela.server.profile.slowMs": 50
}
```

By default the server writes JSONL events to `.vela-lsp-profile.jsonl` in the
workspace root. Each LSP message has a `begin` event and an `end` event. If the
server hangs inside a handler, the last unmatched `begin` entry identifies the
stuck method; otherwise inspect `end.totalMs`, `end.handleMs`, `end.writeMs`,
and `end.outputBytes`.

## Validation

```bash
npm run validate
```

## Local VSIX Packaging

Build a debug `vela_lsp_server`, bundle it into the extension, validate the
package metadata, and create a local VSIX:

```bash
npm run package
```

Useful variants:

```bash
npm run package:release
npm run package:no-server
```

`package:no-server` creates a VSIX without a bundled language-server binary.
Users must set `vela.server.path` in VS Code settings when installing that
package.
