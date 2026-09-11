"use strict";

const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vscode = require("vscode");
const { provenance } = require("../../../scripts/lsp-matrix/provenance");

const delay = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

async function bounded(label, action, milliseconds = 15000) {
  let timer;
  try {
    return await Promise.race([
      action(),
      new Promise((_, reject) => {
        timer = setTimeout(() => reject(new Error(`${label}: timed out after ${milliseconds}ms`)), milliseconds);
      })
    ]);
  } finally {
    clearTimeout(timer);
  }
}

// Retry only during startup. Once ready, every feature assertion runs once;
// retries must not hide lost didChange notifications or stale results.
async function waitForProvider(document) {
  const deadline = Date.now() + 14000;
  await bounded("language server readiness", async () => {
    while (Date.now() < deadline) {
      const symbols = await vscode.commands.executeCommand("vscode.executeDocumentSymbolProvider", document.uri);
      if (symbols?.some((symbol) => symbol.name === "main")) return;
      await delay(100);
    }
    throw new Error("language server did not register a working document symbol provider");
  });
}

function position(document, marker, offset = 0) {
  const index = document.getText().indexOf(marker);
  assert.notEqual(index, -1, `missing marker: ${marker}`);
  return document.positionAt(index + offset);
}

async function definition(document, marker, expectedDocument, expectedMarker, offset = 0) {
  const locations = await vscode.commands.executeCommand(
    "vscode.executeDefinitionProvider", document.uri, position(document, marker, offset)
  );
  assert.equal(locations?.length, 1, `expected one definition for ${marker}: ${JSON.stringify(locations)}`);
  const location = locations[0];
  const uri = location.targetUri ?? location.uri;
  const range = location.targetSelectionRange ?? location.range;
  assert.equal(uri.toString(), expectedDocument.uri.toString(), `wrong target for ${marker}`);
  assert.deepEqual(range.start, position(expectedDocument, expectedMarker), `wrong range for ${marker}`);
  assert.equal(expectedDocument.getText(range), expectedMarker, `wrong target text for ${marker}`);
}

async function run() {
  const results = [];
  const check = async (name, action) => {
    const started = Date.now();
    try {
      await bounded(name, action);
      results.push({ name, passed: true, durationMs: Date.now() - started });
      console.log(`PASS ${name}`);
    } catch (error) {
      results.push({ name, passed: false, error: error.stack });
      throw error;
    }
  };
  try {
    const workspace = vscode.workspace.workspaceFolders[0].uri;
    let document;
    await check("installed VSIX activates on opening a Vela file", async () => {
      document = await vscode.workspace.openTextDocument(vscode.Uri.joinPath(workspace, "scripts/main.vela"));
      await vscode.window.showTextDocument(document);
      assert.equal(document.languageId, "vela");
      await waitForProvider(document);
      const extension = vscode.extensions.getExtension("vela-lang.vela-vscode");
      assert.ok(extension?.isActive, "installed extension must activate through onLanguage");
      const installedRoot = path.resolve(process.env.VELA_TEST_EXTENSIONS_DIR);
      const relative = path.relative(installedRoot, extension.extensionPath);
      assert.ok(relative && !relative.startsWith("..") && !path.isAbsolute(relative), "must load installed VSIX");
      assert.equal(vscode.workspace.getConfiguration("vela").get("server.path"), "", "must use bundled server");
    });
    await check("local definition uses UTF-16 positions after Chinese and emoji", () =>
      definition(document, "local);", document, "local"));
    await check("cross-file definition resolves a file that was never opened", async () => {
      const targets = await vscode.commands.executeCommand(
        "vscode.executeDefinitionProvider", document.uri, position(document, "increment(local)")
      );
      assert.equal(targets?.length, 1, `missing cross-file definition: ${JSON.stringify(targets)}`);
      const target = targets[0];
      const expectedUri = vscode.Uri.joinPath(workspace, "scripts/helpers.vela");
      assert.equal((target.targetUri ?? target.uri).toString(), expectedUri.toString());
      const helper = await vscode.workspace.openTextDocument(expectedUri);
      const range = target.targetSelectionRange ?? target.range;
      assert.deepEqual(range.start, position(helper, "increment"));
      assert.equal(helper.getText(range), "increment");
    });
    await check("F12 command opens the definition in the editor", async () => {
      const editor = await vscode.window.showTextDocument(document);
      const cursor = position(document, "increment(local)");
      editor.selection = new vscode.Selection(cursor, cursor);
      const arrived = new Promise((resolve, reject) => {
        const timer = setTimeout(() => {
          subscription.dispose();
          reject(new Error("F12 did not move the editor selection to the definition"));
        }, 10000);
        const subscription = vscode.window.onDidChangeTextEditorSelection((event) => {
          if (event.textEditor.document.uri.path.endsWith("/helpers.vela") &&
              event.selections[0].active.line === 0 && event.selections[0].active.character === 7) {
            subscription.dispose();
            clearTimeout(timer);
            resolve();
          }
        });
      });
      await vscode.commands.executeCommand("editor.action.revealDefinition");
      await arrived;
      assert.equal(vscode.window.activeTextEditor.document.uri.toString(),
        vscode.Uri.joinPath(workspace, "scripts/helpers.vela").toString());
      assert.equal(vscode.window.activeTextEditor.selection.active.line, 0);
      assert.equal(vscode.window.activeTextEditor.selection.active.character, 7);
    });
    await check("definition observes an unsaved edit and its shifted range", async () => {
      const editor = await vscode.window.showTextDocument(document);
      assert.ok(await editor.edit((edit) => edit.insert(new vscode.Position(0, 0), "// unsaved change\n")));
      assert.ok(document.isDirty);
      await definition(document, "local);", document, "local");
    });
    await check("hover and completion reach the real language client", async () => {
      const cursor = position(document, "increment(local)", 3);
      const hover = await vscode.commands.executeCommand("vscode.executeHoverProvider", document.uri, cursor);
      assert.ok(hover?.some((item) => item.contents.length > 0), "hover must contain content");
      const completion = await vscode.commands.executeCommand("vscode.executeCompletionItemProvider", document.uri, cursor);
      assert.ok(completion?.items.some((item) => (item.label.label ?? item.label) === "helpers::increment"),
        "completion must contain imported function");
    });
    await vscode.commands.executeCommand("workbench.action.revertAndCloseActiveEditor");
  } finally {
    const extension = vscode.extensions.getExtension("vela-lang.vela-vscode");
    const binary = extension && path.join(extension.extensionPath, "server",
      process.platform === "win32" ? "vela_lsp_server.exe" : "vela_lsp_server");
    fs.writeFileSync(path.join(process.env.VELA_TEST_RESULT_DIR, "results.json"), JSON.stringify({
      version: 1,
      vscodeVersion: vscode.version,
      provenance: binary && fs.existsSync(binary) ? provenance(path.resolve(__dirname, "../../.."), binary) : null,
      results
    }, null, 2));
  }
}

module.exports = { run };
