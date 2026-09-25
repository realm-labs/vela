"use strict";

const fs = require("node:fs");
const path = require("node:path");
const { spawnSync } = require("node:child_process");
const { downloadAndUnzipVSCode, runVSCodeCommand, runTests } = require("@vscode/test-electron");

const extensionRoot = path.resolve(__dirname, "..");

async function main() {
  const resultsDir = path.join(extensionRoot, "test-results");
  fs.mkdirSync(resultsDir, { recursive: true });
  const resultRoot = fs.mkdtempSync(path.join(resultsDir, "run-"));
  console.log(`Test artifacts: ${resultRoot}`);
  // Preserve every run, including failures, without touching the user's profile.
  const workspace = path.join(resultRoot, "中文 workspace");
  fs.mkdirSync(path.join(workspace, "scripts"), { recursive: true });
  for (const file of ["vela.toml", "scripts/main.vela", "scripts/helpers.vela"]) {
    fs.writeFileSync(path.join(workspace, file), fs.readFileSync(path.join(__dirname, "fixture", file)));
  }
  const { parseMarkers } = require("../../../scripts/lsp-matrix/fixtures");
  const shared = require("../../../tests/lsp_matrix/fixtures/shared-unicode-lifecycle.json");
  fs.writeFileSync(path.join(workspace, "scripts/helper.vela"), parseMarkers(shared.files["scripts/helper.vela"]).text);
  const navigation = require("../../../tests/lsp_matrix/fixtures/input-navigation.json");
  for (const [file, source] of Object.entries(navigation.files)) {
    const destination = path.join(workspace, file);
    fs.mkdirSync(path.dirname(destination), { recursive: true });
    fs.writeFileSync(destination, parseMarkers(source).text);
  }
  for (const fixture of ["input-references.json", "input-rename.json"]) {
    const spec = require(`../../../tests/lsp_matrix/fixtures/${fixture}`);
    for (const [file, source] of Object.entries(spec.files)) {
      const destination = path.join(workspace, file);
      fs.mkdirSync(path.dirname(destination), { recursive: true });
      fs.writeFileSync(destination, parseMarkers(source).text);
    }
  }
  const diagnosticAction = require("../../../tests/lsp_matrix/fixtures/diagnostic-action-method-typo.json");
  for (const [file, source] of Object.entries(diagnosticAction.files)) {
    const destination = path.join(workspace, file);
    fs.mkdirSync(path.dirname(destination), { recursive: true });
    fs.writeFileSync(destination, parseMarkers(source).text);
  }
  require("./completion-fixture").materializeCompletion(workspace);
  fs.mkdirSync(path.join(workspace, ".vscode"));
  fs.writeFileSync(path.join(workspace, ".vscode", "settings.json"), JSON.stringify({
    "vela.trace.server": "verbose",
    "editor.gotoLocation.multipleDefinitions": "goto",
    "chat.disableAIFeatures": true,
    "workbench.secondarySideBar.defaultVisibility": "hidden",
    "workbench.startupEditor": "none",
    "files.autoSave": "off"
  }));
  const vsix = path.join(resultRoot, "vela.vsix");
  const packaged = spawnSync(process.execPath, [path.join(extensionRoot, "scripts", "package-vsix.js"), "--out", vsix], {
    cwd: extensionRoot, stdio: "inherit", timeout: 300000
  });
  if (packaged.error) throw packaged.error;
  if (packaged.status !== 0) throw new Error(`VSIX packaging failed: ${packaged.status}`);

  const { selectProfile } = require("../../../scripts/lsp-matrix/profiles");
  const { profiles } = require("../../../tests/lsp_matrix/checkpoint.json");
  const version = process.env.VSCODE_TEST_VERSION || selectProfile(profiles, process, false)?.vscodeVersion || "stable";
  const vscodeExecutablePath = await downloadAndUnzipVSCode(version);
  const extensionsDir = path.join(resultRoot, "extensions");
  const userDataDir = path.join(resultRoot, "user-data");
  const isolatedArgs = ["--extensions-dir", extensionsDir, "--user-data-dir", userDataDir];
  const installed = await runVSCodeCommand(["--install-extension", vsix, "--force", ...isolatedArgs], {
    version, spawn: { timeout: 120000, windowsHide: true }
  });
  console.log(installed.stdout);
  let editorFailure;
  try {
    await runTests({
      vscodeExecutablePath,
      // Only this empty driver is loaded as a development extension. Vela must
      // come from the VSIX installed above, including its production dependencies.
      extensionDevelopmentPath: path.join(__dirname, "driver"),
      extensionTestsPath: path.join(__dirname, "suite.js"),
      extensionTestsEnv: {
        ELECTRON_RUN_AS_NODE: undefined,
        VELA_TEST_EXTENSIONS_DIR: extensionsDir,
        VELA_TEST_RESULT_DIR: resultRoot
      },
      launchArgs: [workspace, ...isolatedArgs, "--skip-welcome", "--skip-release-notes",
        "--disable-workspace-trust", "--disable-updates", "--disable-gpu", "--no-sandbox"]
    });
  } catch (error) {
    editorFailure = error;
  }
  const resultFile = path.join(resultRoot, "results.json");
  const audit = spawnSync(process.execPath, [path.join(extensionRoot, "../../scripts/lsp-matrix/run.js"),
    "--run", ...(fs.existsSync(resultFile) ? ["--editor-results", resultFile] : [])], {
    cwd: extensionRoot, stdio: "inherit", timeout: 600000, windowsHide: true
  });
  if (editorFailure) throw editorFailure;
  if (audit.error) throw audit.error;
  if (audit.status !== 0) throw new Error(`LSP matrix audit failed: ${audit.status}`);
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
