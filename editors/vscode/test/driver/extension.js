"use strict";
const path = require("node:path");
const fs = require("node:fs");
// Test-only host identity. Opening Vela and sending accepted native input stays
// under the external driver; this extension only reports its own log directory.
function activate(context) {
  if (process.env.VELA_TEST_INPUT_DRIVER === "1") {
    // A real workbench reload restarts this extension host. Keep the native
    // driver independent of VS Code's one-shot extension-test exit callback.
    setImmediate(() => require(process.env.VELA_TEST_INPUT_BRIDGE).run().then(
      () => require("vscode").commands.executeCommand("workbench.action.quit"),
      error => {
        fs.writeFileSync(path.join(process.env.VELA_TEST_RESULT_DIR,"bridge-failure.log"),error.stack);
        void require("vscode").commands.executeCommand("workbench.action.quit");
      }
    ));
  }
  return { pid: process.pid, logDirectory: path.dirname(context.logUri.fsPath), mode: context.extensionMode,
    extensionPath: context.extensionPath };
}
module.exports = { activate };
