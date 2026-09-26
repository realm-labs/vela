"use strict";

// Test-only extension-host bridge: fixture setup and observations. Input actions
// under acceptance are sent by the external workbench driver, never this bridge.
const fs = require("node:fs");
const path = require("node:path");
const http = require("node:http");
const crypto = require("node:crypto");
const vscode = require("vscode");
const { fileUri } = require("./paths");
const {
  parseMarkers,
  safeFile,
} = require("../../../../scripts/lsp-matrix/fixtures");
const spec = require("../../../../tests/lsp_matrix/fixtures/input-driver.json");

async function run() {
  const root = process.env.VELA_TEST_RESULT_DIR;
  const token = crypto.randomBytes(32).toString("hex");
  let finish;
  const finished = new Promise((resolve) => {
    finish = resolve;
  });
  const file = vscode.Uri.joinPath(
    vscode.workspace.workspaceFolders[0].uri,
    "scripts/main.vela",
  );
  const document = await vscode.workspace.openTextDocument(file);
  const fixture = parseMarkers(spec.files["scripts/main.vela"]);
  const cursor = fixture.markers.cursor.start;
  await vscode.window.showTextDocument(document, {
    selection: new vscode.Range(
      cursor.line,
      cursor.character,
      cursor.line,
      cursor.character,
    ),
  });
  const deadline = Date.now() + 15000;
  let ready = false;
  while (Date.now() < deadline) {
    const symbols = await vscode.commands.executeCommand(
      "vscode.executeDocumentSymbolProvider",
      file,
    );
    if (symbols?.some((symbol) => symbol.name === "main")) {
      ready = true;
      break;
    }
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
  if (!ready) throw new Error("installed language client did not become ready");
  const extension = vscode.extensions.getExtension("vela-lang.vela-vscode");
  if (
    !extension?.isActive ||
    !path
      .resolve(extension.extensionPath)
      .startsWith(path.resolve(process.env.VELA_TEST_EXTENSIONS_DIR) + path.sep)
  ) {
    throw new Error("Vela must load from installed VSIX");
  }
  const inspect = async () => {
    const editor = vscode.window.activeTextEditor;
    const settings = Object.fromEntries(
      Object.keys(require("../../../../scripts/lsp-matrix/profiles").inputProfile(path.resolve(__dirname, "../../../..")).settings).map((key) => [
        key,
        vscode.workspace.getConfiguration().get(key),
      ]),
    );
    return {
      vscodeVersion: vscode.version,
      platform: process.platform,
      arch: process.arch,
      locale: vscode.env.language,
      extensionPath: extension.extensionPath,
      settings,
      documents: vscode.workspace.textDocuments
        .filter((doc) => doc.uri.scheme === "file")
        .map((doc) => ({
          uri: fileUri(doc.uri.fsPath),
          text: doc.getText(),
          dirty: doc.isDirty,
          version: doc.version,
          languageId: doc.languageId,
        })),
      active: editor && {
        uri: fileUri(editor.document.uri.fsPath),
        text: editor.document.getText(),
        dirty: editor.document.isDirty,
        selections: editor.selections.map((selection) => ({
          anchor: {
            line: selection.anchor.line,
            character: selection.anchor.character,
          },
          active: {
            line: selection.active.line,
            character: selection.active.character,
          },
        })),
      },
    };
  };
  const server = http.createServer(async (request, response) => {
    try {
      if (
        request.headers.authorization !== `Bearer ${token}` ||
        request.method !== "POST"
      ) {
        response.writeHead(403).end();
        return;
      }
      const chunks = [];
      let length = 0;
      for await (const chunk of request) {
        length += chunk.length;
        if (length > 65536) throw Error("bridge request too large");
        chunks.push(chunk);
      }
      const message = JSON.parse(Buffer.concat(chunks).toString());
      let value;
      switch (message.op) {
        case "inspect":
          value = await inspect();
          break;
        case "diagnostics": {
          const uri = vscode.Uri.joinPath(
            vscode.workspace.workspaceFolders[0].uri,
            safeFile(message.file),
          );
          value = vscode.languages.getDiagnostics(uri).map((item) => ({
            code: String((item.code !== null && typeof item.code === "object" ? item.code.value : item.code) ?? ""),
            message: item.message,
            severity: item.severity,
            range: {
              start: { line: item.range.start.line, character: item.range.start.character },
              end: { line: item.range.end.line, character: item.range.end.character },
            },
            related: item.relatedInformation?.length ?? 0,
          }));
          break;
        }
        case "token-setting": {
          value = { key: "editor.semanticHighlighting.enabled",
            value: vscode.workspace.getConfiguration().get("editor.semanticHighlighting.enabled") };
          break;
        }
        case "setup": {
          const uri = vscode.Uri.joinPath(
            vscode.workspace.workspaceFolders[0].uri,
            safeFile(message.file),
          );
          if (message.reset === true) {
            const previous = vscode.workspace.textDocuments.find((item) => item.uri.toString() === uri.toString());
            if (previous?.isDirty) {
              await vscode.window.showTextDocument(previous);
              await vscode.commands.executeCommand("workbench.action.revertAndCloseActiveEditor");
            }
          }
          const doc = await vscode.workspace.openTextDocument(uri);
          await vscode.window.showTextDocument(doc, {
            selection: new vscode.Range(
              message.line,
              message.character,
              message.line,
              message.character,
            ),
          });
          value = await inspect();
          break;
        }
        case "command": {
          const allowed = Object.values(require("../../../../scripts/lsp-matrix/navigation-contracts").commands);
          if (!allowed.includes(message.command)) throw Error("unsupported navigation command");
          await vscode.commands.executeCommand(message.command);
          value = await inspect();
          break;
        }
        case "finish":
          value = { finished: true };
          setImmediate(finish);
          break;
        default:
          throw Error("unsupported bridge operation");
      }
      response.setHeader("content-type", "application/json");
      response.end(JSON.stringify({ value }));
    } catch (error) {
      response.writeHead(500).end(JSON.stringify({ error: error.stack }));
    }
  });
  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolve);
  });
  fs.writeFileSync(
    path.join(root, "bridge.json"),
    JSON.stringify({ port: server.address().port, token }),
  );
  let expired = false;
  const timer = setTimeout(() => {
    expired = true;
    finish();
  }, 180000);
  try {
    await finished;
    if (expired)
      throw new Error("input driver did not finish within its deadline");
  } finally {
    clearTimeout(timer);
    await new Promise((resolve) => server.close(resolve));
  }
}
module.exports = { run };
