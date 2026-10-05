"use strict";
const fs = require("node:fs"), path = require("node:path"), assert = require("node:assert/strict");
const { spawn } = require("node:child_process"), { chromium } = require("playwright-core");
const { FixtureWorkspace } = require("../../../../scripts/lsp-matrix/fixtures");
const evidence = require("../../../../scripts/lsp-matrix/local-evidence");
const { readSession } = require("./session");
const { windowsDesktop } = require("./native-menu");
async function runWorkspaceTrustSession({ root, profile, executable, extensions, installedServer, contracts, record, until, onProof }) {
  if (!contracts.some(c => c.id.startsWith("ux21-"))) return;
  // Separate owned profile, enabled Workspace Trust, sequential normal window.
  // Existing trusted450s + this finite210s lane; neither lane changes its budget.
  const owned = path.join(root, "trust"), workspace = path.join(owned, "中文 % trust workspace");
  fs.mkdirSync(owned); new FixtureWorkspace(require("../../../../tests/lsp_matrix/fixtures/input-workspace-trust.json")).materialize(workspace);
  const m = require("../../../../scripts/lsp-matrix/workspace-trust-contracts").workspaceTrustModel(profile.platform);
  const binary = path.join(workspace, m.configured); fs.mkdirSync(path.dirname(binary)); fs.copyFileSync(path.join(root, installedServer), binary);
  if (profile.platform === "darwin") fs.chmodSync(binary, 0o755);
  assert.equal(evidence.fileHash(binary), evidence.fileHash(path.join(root, installedServer)));
  const userData = path.join(owned, "user-data"); require("./profile-settings").writeProfileSettings(workspace, userData, profile);
  const settingsFile = path.join(workspace, ".vscode/settings.json"), settings = JSON.parse(fs.readFileSync(settingsFile, "utf8"));
  settings["vela.server.path"] = m.configured; fs.writeFileSync(settingsFile, JSON.stringify(settings, null, 2));
  const userFile = path.join(userData, "User/settings.json"), user = JSON.parse(fs.readFileSync(userFile, "utf8"));
  user["security.workspace.trust.enabled"] = true; user["security.workspace.trust.startupPrompt"] = "always";
  fs.writeFileSync(userFile, JSON.stringify(user, null, 2));
  const args = [workspace, "--extensions-dir", extensions, "--user-data-dir", userData, "--shared-data-dir", path.join(owned, "shared-data"),
    "--remote-debugging-port=0", "--remote-debugging-address=127.0.0.1", "--locale=en", "--skip-welcome", "--skip-release-notes", "--disable-updates", "--disable-gpu", "--no-sandbox"];
  assert(!args.includes("--disable-workspace-trust"));
  fs.writeFileSync(path.join(owned, "setup.json"), JSON.stringify({ args, settings, userSettings: user, configured: m.configured,
    binarySha256: evidence.fileHash(binary), installedBinarySha256: evidence.fileHash(path.join(root, installedServer)) }, null, 2));
  const env = { ...process.env, VELA_TEST_RESULT_DIR: owned, VELA_TEST_EXTENSIONS_DIR: extensions, VELA_TEST_INPUT_DRIVER: "1",
    VELA_TEST_INPUT_BRIDGE: path.join(__dirname, "trust-bridge.js"), VELA_TEST_WORKSPACE_BASE: workspace,
    VELA_TEST_VELA_EXTENSION_DIR: path.dirname(path.dirname(path.join(root, installedServer))) }; delete env.ELECTRON_RUN_AS_NODE;
  const log = fs.createWriteStream(path.join(owned, "workbench.log")), child = spawn(executable, args, { env, stdio: ["ignore", "pipe", "pipe"] });
  let cdp, exit, closed = false, childError, browser, page, keyboardState, failure;
  for (const stream of [child.stdout, child.stderr]) {
    let pending = ""; stream.on("data", chunk => { log.write(chunk); pending = (pending + chunk).slice(-16384);
      const match = pending.match(/DevTools listening on (ws:\/\/127\.0\.0\.1:\d+\/[^\r\n]+)[\r\n]/); if (match) cdp = match[1]; });
  }
  child.on("error", error => { childError = error; });
  const exited = new Promise(resolve => child.once("exit", (code, signal) => { exit = { code, signal }; resolve(exit); }));
  child.once("close", () => { closed = true; });
  const alive = () => { if (childError) throw childError; if (exit) throw Error("trust workbench exited " + JSON.stringify(exit)); };
  const timer = setTimeout(() => child.kill("SIGTERM"), 210000);
  try {
    await until("trust workbench debug endpoint", () => { alive(); return cdp; }, 30000);
    browser = await chromium.connectOverCDP(cdp, { timeout: 15000 });
    page = await until("trust workbench page", () => { alive(); return browser.contexts()[0].pages().find(p => p.url().includes("workbench")); });
    page.setDefaultTimeout(15000); await page.bringToFront();
    if (profile.platform === "win32") keyboardState = windowsDesktop("select", child.pid, profile.keyboardLayout);
    const observedDisplay = await page.evaluate(() => ({ width: innerWidth, height: innerHeight, deviceScaleFactor: devicePixelRatio, screenWidth: screen.width, screenHeight: screen.height }));
    assert.deepEqual(observedDisplay, profile.display); record("observation", "trust-display", { observed: observedDisplay });
    const bridge = async (op) => {
      const deadline = Date.now() + 15000;
      while (Date.now() < deadline) {
        alive();
        try {
          const session = readSession(owned), response = await fetch(`http://127.0.0.1:${session.port}`, { method: "POST", headers: { authorization: `Bearer ${session.token}` },
            body: JSON.stringify({ op }), signal: AbortSignal.timeout(Math.min(5000, deadline - Date.now())) });
          const body = await response.json(); if (!response.ok) throw Error(body.error); return body.value;
        } catch (error) {
          // Only transient observation transport during a host transition may
          // retry. Finish, native input and server-side observer errors do not.
          if (op === "finish" || !["ECONNRESET", "ECONNREFUSED", "ENOENT"].includes(error.cause?.code ?? error.code)) throw error;
          record("observation", "trust-observer-transition", { code: error.cause?.code ?? error.code });
          await new Promise(resolve => setTimeout(resolve, 50));
        }
      }
      throw Error("bounded current trust observer unavailable");
    };
    await require("./workspace-trust").runWorkspaceTrust({ page, bridge, record, root, owned, workspace, profile, contracts, until, onProof });
    if (keyboardState) { windowsDesktop("restore", child.pid, keyboardState.previous, keyboardState.window); keyboardState = undefined; }
    await bridge("finish"); const result = await until("trust workbench clean exit", () => { require("./observer-health").assertObserverHealthy(owned); return exit; }, 15000);
    require("./observer-health").assertObserverHealthy(owned); assert.deepEqual(result, { code: 0, signal: null });
    record("assertion", "trust-clean-exit", { observed: result });
  } catch (error) {
    failure = error;
    if (page) { await page.screenshot({ path: path.join(owned, "failure.png") }).catch(() => {}); fs.writeFileSync(path.join(owned, "failure.aria.txt"), await page.locator("body").ariaSnapshot().catch(() => "unavailable")); }
  } finally {
    clearTimeout(timer);
    if (keyboardState) { try { windowsDesktop("restore", child.pid, keyboardState.previous, keyboardState.window); } catch (error) { failure ??= error; } }
    if (!exit) { child.kill("SIGTERM"); await Promise.race([exited, new Promise(resolve => setTimeout(resolve, 3000))]); if (!exit) child.kill("SIGKILL"); }
    if (browser) await browser.close().catch(() => {});
    try { await until("trust process streams closed", () => closed, 5000); } catch (error) { failure ??= error; child.stdout.destroy(); child.stderr.destroy(); }
    await new Promise(resolve => log.end(resolve));
  }
  if (failure) throw failure;
}
module.exports = { runWorkspaceTrustSession };
