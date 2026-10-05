"use strict";
const fs = require("node:fs");
const path = require("node:path");
const vscode = require("vscode");
const { provenance } = require("../../../scripts/lsp-matrix/provenance");
const { runWorkspaceSymbols } = require("./workspace-symbol-provider");

async function run() {
  const results = [];
  const check = async (name, action) => {
    const started = Date.now(); let timer;
    try {
      await Promise.race([action(), new Promise((_, reject) => {
        timer = setTimeout(() => reject(Error(`${name}: timed out after 180000ms`)), 180000);
      })]);
      results.push({ name, passed: true, durationMs: Date.now() - started }); console.log(`PASS ${name}`);
    } catch (error) { results.push({ name, passed: false, error: error.stack }); throw error; }
    finally { clearTimeout(timer); }
  };
  try {
    await check("workspace symbol provider preserves complete source and schema sets through Unicode dirty close restoration", () =>
      runWorkspaceSymbols(vscode));
  } finally {
    const extension = vscode.extensions.getExtension("vela-lang.vela-vscode");
    const binary = extension && path.join(extension.extensionPath, "server",
      process.platform === "win32" ? "vela_lsp_server.exe" : "vela_lsp_server");
    fs.writeFileSync(path.join(process.env.VELA_TEST_RESULT_DIR, "results.json"), JSON.stringify({ version: 1,
      vscodeVersion: vscode.version,
      provenance: binary && fs.existsSync(binary) ? provenance(path.resolve(__dirname, "../../.."), binary) : null,
      results }, null, 2));
  }
}
module.exports = { run };
