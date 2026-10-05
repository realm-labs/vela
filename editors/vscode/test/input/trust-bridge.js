"use strict";
// Installed test-only observer. No configuration, document or trust mutation.
const fs = require("node:fs"), path = require("node:path"), http = require("node:http"), crypto = require("node:crypto"), assert = require("node:assert/strict");
const vscode = require("vscode"), { fileUri } = require("./paths");
async function run() {
  const root = process.env.VELA_TEST_RESULT_DIR, workspace = process.env.VELA_TEST_WORKSPACE_BASE, extensions = process.env.VELA_TEST_EXTENSIONS_DIR;
  const host = await vscode.extensions.getExtension("vela-tests.vela-test-driver").activate();
  const installed = path.relative(extensions, host.extensionPath);
  assert(host.mode === vscode.ExtensionMode.Production && installed && !installed.startsWith("..") && !path.isAbsolute(installed));
  const logDirectory = await require("./host-identity").observeHostLogs(vscode, host);
  // Restricted Mode omits unsupported runtime extensions from the host API.
  // Read only the already verified installed archive's own manifest; re-query
  // runtime activation on every observation after the user's trust transition.
  const extensionPath = process.env.VELA_TEST_VELA_EXTENSION_DIR;
  const extensionRelative = path.relative(extensions, extensionPath);
  assert(extensionRelative && !extensionRelative.startsWith("..") && !path.isAbsolute(extensionRelative));
  const manifest = JSON.parse(fs.readFileSync(path.join(extensionPath, "package.json"), "utf8"));
  const m = require("../../../../scripts/lsp-matrix/workspace-trust-contracts").workspaceTrustModel(process.platform);
  const token = crypto.randomBytes(32).toString("hex"); let finish;
  const finished = new Promise(resolve => { finish = resolve; });
  const server = http.createServer(async (request, response) => {
    try {
      if (request.headers.authorization !== `Bearer ${token}`) { response.writeHead(403).end(); return; }
      let data = ""; for await (const chunk of request) { data += chunk; assert(data.length < 16384); }
      const { op } = JSON.parse(data); let value;
      if (op === "inspect") {
        const editor = vscode.window.activeTextEditor;
        const extension = vscode.extensions.getExtension("vela-lang.vela-vscode");
        value = { trusted: vscode.workspace.isTrusted, active: extension?.isActive ?? false, extensionPath,
          policy: manifest.capabilities?.untrustedWorkspaces ?? null, configured: vscode.workspace.getConfiguration("vela").get("server.path"),
          vscodeVersion: vscode.version, platform: process.platform, arch: process.arch, locale: vscode.env.language,
          folders: (vscode.workspace.workspaceFolders ?? []).map(f => fileUri(f.uri.fsPath)),
          settings: Object.fromEntries(Object.keys(require("../../../../scripts/lsp-matrix/profiles").inputProfile(path.resolve(__dirname, "../../../..")).settings).map(key => [key, vscode.workspace.getConfiguration().get(key)])),
          documents: vscode.workspace.textDocuments.filter(d => d.uri.scheme === "file" && path.dirname(d.uri.fsPath).toLowerCase() === workspace.toLowerCase())
            .map(d => ({ uri: fileUri(d.uri.fsPath), text: d.getText(), dirty: d.isDirty })),
          activeEditor: editor && { uri: fileUri(editor.document.uri.fsPath), text: editor.document.getText(), dirty: editor.document.isDirty,
            selections: editor.selections.map(s => ({ anchor: { line: s.anchor.line, character: s.anchor.character }, active: { line: s.active.line, character: s.active.character } })) } };
      } else if (op === "query") {
        const p = m.dirty.markers.call.start, uri = vscode.Uri.file(path.join(workspace, m.o.caller));
        const rows = await vscode.commands.executeCommand("vscode.executeDefinitionProvider", uri, new vscode.Position(p.line, p.character));
        value = (rows ?? []).map(r => ({ uri: fileUri((r.targetUri ?? r.uri).fsPath), range: { start: { line: (r.targetSelectionRange ?? r.range).start.line, character: (r.targetSelectionRange ?? r.range).start.character },
          end: { line: (r.targetSelectionRange ?? r.range).end.line, character: (r.targetSelectionRange ?? r.range).end.character } } }));
      } else if (op === "finish") { value = { finished: true }; setImmediate(finish); }
      else throw Error("unsupported read-only trust observer operation");
      response.setHeader("content-type", "application/json"); response.end(JSON.stringify({ value }));
    } catch (error) { response.writeHead(500).end(JSON.stringify({ error: error.stack })); }
  });
  await new Promise((resolve, reject) => { server.once("error", reject); server.listen(0, "127.0.0.1", resolve); });
  const temporary = path.join(root, `bridge-${process.pid}.tmp`);
  fs.writeFileSync(temporary, JSON.stringify({ port: server.address().port, token, pid: host.pid, logDirectory, hostLogDirectory: host.logDirectory, mode: host.mode }));
  fs.renameSync(temporary, path.join(root, "bridge.json"));
  const timer = setTimeout(finish, 210000);
  try { await finished; } finally { clearTimeout(timer); await new Promise(resolve => server.close(resolve)); identity.dispose(); }
}
module.exports = { run };
