"use strict";
const assert = require("node:assert/strict"), fs = require("node:fs"), path = require("node:path");
const vscode = require("vscode");
const { provenance } = require("../../../scripts/lsp-matrix/provenance");
async function startup(root) {
  let timer;
  try {
    await Promise.race([(async () => {
      const uri = vscode.Uri.joinPath(root, "scripts/main.vela"), deadline = Date.now() + 14000;
      await vscode.commands.executeCommand("vscode.open", uri, { preview: false });
      while (Date.now() < deadline) {
        const symbols = await vscode.commands.executeCommand("vscode.executeDocumentSymbolProvider", uri);
        if (symbols?.some(s => s.name === "main")) return;
        await new Promise(resolve => setTimeout(resolve, 100));
      }
      throw Error("installed provider did not become ready");
    })(), new Promise((_, reject) => { timer = setTimeout(() => reject(Error("installed provider startup: timed out after14000ms")), 14000); })]);
  } finally { clearTimeout(timer); }
}
async function run() {
  const kind = process.env.VELA_SELECTION_KIND; assert(["syntax", "lifecycle"].includes(kind));
  const results = [], root = vscode.workspace.workspaceFolders[0].uri;
  const check = async (name, action) => {
    const started = Date.now(); let timer;
    try {
      await Promise.race([action(), new Promise((_, reject) => {
        timer = setTimeout(() => reject(Error(`${name}: timed out after 600000ms`)), 600000);
      })]);
      results.push({ name, passed: true, durationMs: Date.now() - started }); console.log(`PASS ${name}`);
    } catch (error) { results.push({ name, passed: false, error: error.stack }); throw error; }
    finally { clearTimeout(timer); }
  };
  try {
    await startup(root);
    if (kind === "syntax") {
      await check("selection provider preserves complete authored public and wire parent chains through dirty close Unicode LF CRLF states", () =>
        require("./selection-provider").runSelectionSyntax(vscode, root));
    } else {
      await check("selection provider preserves complete public and wire vectors through saved closed and renamed dependency recovery", () =>
        require("./selection-lifecycle-provider").runSelectionLifecycle(vscode, root));
    }
  } finally {
    const extension = vscode.extensions.getExtension("vela-lang.vela-vscode"), binary = extension && path.join(extension.extensionPath, "server",
      process.platform === "win32" ? "vela_lsp_server.exe" : "vela_lsp_server");
    fs.writeFileSync(path.join(process.env.VELA_TEST_RESULT_DIR, "results.json"), JSON.stringify({ version: 1,
      vscodeVersion: vscode.version, provenance: binary && fs.existsSync(binary) ? provenance(path.resolve(__dirname, "../../.."), binary) : null,
      results }, null, 2));
  }
}
module.exports = { run };
