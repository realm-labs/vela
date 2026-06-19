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
profiling and server trace JSONL in VS Code settings:

```json
{
  "vela.server.profile.enabled": true,
  "vela.server.profile.slowMs": 50,
  "vela.trace.server": "messages"
}
```

By default the server writes JSONL events to `.vela-lsp-profile.jsonl` in the
workspace root. The leading dot makes it a hidden file on macOS. Set
`vela.server.profile.path` to an absolute path such as
`/path/to/workspace/vela-lsp-profile.jsonl` when a visible file is easier to
inspect. When `vela.trace.server` is not `off`, the extension also passes
`--log` to the native server and prints the `.vela-lsp-trace.jsonl` path in the
Vela output channel.

To compare server handler time with a VS Code-side stall, reproduce the stall
and inspect the JSONL files around the same request `id`, `method`, and `lane`.
The profile file gives coarse request timing through `totalMs`, `handleMs`,
`writeMs`, and `outputBytes`. The trace file shows the main-loop sequence:
`message_received`, `request_queued`, `task_started`, `task_ended`, and
`response_sent`.

Interpret incomplete trace sequences this way:

- `message_received` without a matching `response_sent`: synchronous main-loop
  handling or response writing did not finish.
- `request_queued` without `task_started`: the background lane did not start
  the queued request.
- `task_started` without `task_ended`: the background handler is still
  running.
- `task_ended` without task `response_sent`: the handler finished, but result
  handling or client response writing did not finish.

If the profile and trace show low `handleMs`, low `writeMs`, and completed
`response_sent` events while VS Code still appears stalled, the delay is likely
outside the native server process. Use VS Code's extension host profiling or
temporarily disable file watchers to isolate editor-side work.

If the profile shows fast server handlers but VS Code still stalls, temporarily
disable server-registered file watchers:

```json
{
  "vela.server.watchFiles.enabled": false
}
```

This keeps open-document language features available, but disk changes to
closed `.vela` files, `vela.toml`, or schema artifacts will not be pushed to
the server until file watching is enabled again.

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
