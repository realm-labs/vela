"use strict";
const assert = require("node:assert/strict"), fs = require("node:fs"), path = require("node:path");
const { runTests } = require("@vscode/test-electron");
const { materializeSelection } = require("./selection-provider");
async function runSelectionEditor({ directory, kind, extensionsDir, vscodeExecutablePath }) {
  assert(["syntax", "lifecycle"].includes(kind)); assert(!fs.existsSync(directory)); fs.mkdirSync(directory);
  materializeSelection(directory, [kind]);
  const workspace = path.join(directory, "中文 % startup"), settings = {
    "vela.trace.server": "verbose", "files.autoSave": "off", "chat.disableAIFeatures": true,
    "workbench.secondarySideBar.defaultVisibility": "hidden", "workbench.startupEditor": "none"
  };
  fs.mkdirSync(path.join(workspace, "scripts"), { recursive: true });
  fs.writeFileSync(path.join(workspace, "scripts/main.vela"), "pub fn main() {}\n");
  const workspaceFile = path.join(directory, "selection.code-workspace");
  fs.writeFileSync(workspaceFile, JSON.stringify({ folders: [{ path: workspace }], settings }));
  await runTests({ vscodeExecutablePath,
    extensionDevelopmentPath: path.join(__dirname, "driver"),
    extensionTestsPath: path.join(__dirname, "selection-suite.js"),
    extensionTestsEnv: { ELECTRON_RUN_AS_NODE: undefined, VELA_TEST_EXTENSIONS_DIR: extensionsDir,
      VELA_TEST_RESULT_DIR: directory, VELA_SELECTION_KIND: kind },
    launchArgs: [workspaceFile, "--extensions-dir", extensionsDir, "--user-data-dir", path.join(directory, "user-data"),
      "--skip-welcome", "--skip-release-notes", "--disable-workspace-trust", "--disable-updates", "--disable-gpu", "--no-sandbox"] });
}
module.exports = { runSelectionEditor };
