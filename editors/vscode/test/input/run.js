"use strict";
const fs = require("node:fs");
const path = require("node:path");
const assert = require("node:assert/strict");
const { spawn, spawnSync } = require("node:child_process");
const {
  downloadAndUnzipVSCode,
  runVSCodeCommand,
} = require("@vscode/test-electron");
const { chromium } = require("playwright-core");
const { FixtureWorkspace } = require("../../../../scripts/lsp-matrix/fixtures");
const profile = require("../../../../scripts/lsp-matrix/profiles").inputProfile(path.resolve(__dirname, "../../../.."));
const { buildNativeMenu, windowsDesktop } = require("./native-menu");
const fixture = require("../../../../tests/lsp_matrix/fixtures/input-driver.json");
const evidence = require("../../../../scripts/lsp-matrix/local-evidence");
const {
  localContracts,
} = require("../../../../scripts/lsp-matrix/local-contracts");
const { loadInventory } = require("../../../../scripts/lsp-matrix/inventory");
const {
  verifyInstalledPackage,
} = require("../../../../scripts/lsp-matrix/archive-evidence");

const extensionRoot = path.resolve(__dirname, "../..");
const repository = path.resolve(extensionRoot, "../..");
const delay = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
async function until(label, read, timeout = 15000) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    const result = await read();
    if (result) return result;
    await delay(50);
  }
  throw Error(`${label}: timed out after ${timeout}ms`);
}

async function run() {
  assert.equal(
    process.platform,
    profile.platform,
    "use the pinned local profile",
  );
  assert.equal(process.arch, profile.arch);
  if (profile.platform === "darwin") {
    const keyboard = spawnSync(
      "defaults",
      ["read", "com.apple.HIToolbox", "AppleCurrentKeyboardLayoutInputSourceID"],
      { encoding: "utf8", timeout: 5000 },
    );
    if (keyboard.error) throw keyboard.error;
    assert.equal(keyboard.status, 0);
    assert.equal(keyboard.stdout.trim(), profile.keyboardLayout);
  }
  const resultsDir = path.join(extensionRoot, "test-results");
  fs.mkdirSync(resultsDir, { recursive: true });
  const root = fs.mkdtempSync(path.join(resultsDir, "input-"));
  console.log(`Input artifacts: ${root}`);
  buildNativeMenu(root, profile.platform);
  const trace = [];
  const record = (kind, id, details) => {
    trace.push({
      kind,
      id,
      proof: fixture.id,
      at: new Date().toISOString(),
      ...details,
    });
    fs.writeFileSync(
      path.join(root, "trace.json"),
      JSON.stringify(trace, null, 2),
    );
  };
  const workspace = path.join(root, "中文 % workspace");
  new FixtureWorkspace(fixture).materialize(workspace);
  const navigation = require("../../../../tests/lsp_matrix/fixtures/input-navigation.json");
  for (const [file, document] of new FixtureWorkspace(navigation).disk) {
    const target = path.join(workspace, file);
    assert.ok(!fs.existsSync(target), "navigation fixture must not overwrite driver files");
    fs.mkdirSync(path.dirname(target), { recursive: true });
    fs.writeFileSync(target, document.text);
  }
  require("../completion-fixture").materializeCompletion(workspace);
  const rename = require("../../../../tests/lsp_matrix/fixtures/input-rename.json");
  for (const [file, document] of new FixtureWorkspace(rename).disk) {
    const target = path.join(workspace, file);
    assert.ok(!fs.existsSync(target), "rename fixture must not overwrite driver files");
    fs.mkdirSync(path.dirname(target), { recursive: true });
    fs.writeFileSync(target, document.text);
  }
  const references = require("../../../../tests/lsp_matrix/fixtures/input-references.json");
  for (const [file, document] of new FixtureWorkspace(references).disk) {
    const target = path.join(workspace, file);
    assert.ok(!fs.existsSync(target), "references fixture must not overwrite driver files");
    fs.mkdirSync(path.dirname(target), { recursive: true });
    fs.writeFileSync(target, document.text);
  }
  fs.mkdirSync(path.join(workspace, ".vscode"));
  fs.writeFileSync(
    path.join(workspace, ".vscode/settings.json"),
    JSON.stringify(profile.settings, null, 2),
  );
  const vsix = path.join(root, "vela.vsix");
  const packaged = spawnSync(
    process.execPath,
    [path.join(extensionRoot, "scripts/package-vsix.js"), "--out", vsix],
    { cwd: extensionRoot, stdio: "inherit", timeout: 300000 },
  );
  if (packaged.error) throw packaged.error;
  if (packaged.status !== 0) throw Error(`packaging exited ${packaged.status}`);
  const cachePath = path.join(extensionRoot, ".vscode-test");
  const executable = await downloadAndUnzipVSCode({
    version: profile.vscodeVersion,
    cachePath,
  });
  const extensions = path.join(root, "extensions"),
    userData = path.join(root, "user-data");
  const isolated = [
    "--extensions-dir",
    extensions,
    "--user-data-dir",
    userData,
  ];
  const install = await runVSCodeCommand(
    ["--install-extension", vsix, "--force", ...isolated],
    { version: profile.vscodeVersion, cachePath, spawn: { timeout: 120000, windowsHide: true } },
  );
  if (install.exitCode !== undefined && install.exitCode !== 0)
    throw Error("VSIX install failed");
  const log = fs.createWriteStream(path.join(root, "workbench.log"));
  const env = {
    ...process.env,
    VELA_TEST_RESULT_DIR: root,
    VELA_TEST_EXTENSIONS_DIR: extensions,
  };
  delete env.ELECTRON_RUN_AS_NODE;
  const args = [
    workspace,
    ...isolated,
    "--extensionDevelopmentPath=" + path.join(extensionRoot, "test/driver"),
    "--extensionTestsPath=" + path.join(__dirname, "bridge.js"),
    "--remote-debugging-port=0",
    "--remote-debugging-address=127.0.0.1",
    "--locale=en",
    "--skip-welcome",
    "--skip-release-notes",
    "--disable-workspace-trust",
    "--disable-updates",
    "--disable-gpu",
    "--no-sandbox",
  ];
  const child = spawn(executable, args, {
    env,
    stdio: ["ignore", "pipe", "pipe"],
  });
  let cdp, exit, childError;
  for (const stream of [child.stdout, child.stderr]) {
    let pending = "";
    stream.on("data", (chunk) => {
      log.write(chunk);
      pending = (pending + chunk.toString()).slice(-16384);
      const match = pending.match(
        /DevTools listening on (ws:\/\/127\.0\.0\.1:\d+\/[^\r\n]+)[\r\n]/,
      );
      if (match) cdp = match[1];
    });
  }
  child.on("error", (error) => {
    childError = error;
  });
  const exited = new Promise((resolve) =>
    child.once("exit", (code, signal) => {
      exit = { code, signal };
      resolve(exit);
    }),
  );
  let closed = false;
  child.once("close", () => {
    closed = true;
  });
  const alive = () => {
    if (childError) throw childError;
    if (exit) throw Error(`workbench exited ${JSON.stringify(exit)}`);
  };
  let browser,
    page,
    bridge,
    error,
    inputs,
    installedServer,
    proof,
    observedDisplay;
  const proofs = [];
  let contracts = [];
  let keyboardState;
  const restoreKeyboard = () => {
    if (keyboardState) {
      windowsDesktop("restore", child.pid, keyboardState.previous, keyboardState.window);
      keyboardState = undefined;
    }
  };
  const timer = setTimeout(() => child.kill("SIGTERM"), 180000);
  try {
    await until(
      "debug endpoint",
      () => {
        alive();
        return cdp;
      },
      30000,
    );
    browser = await chromium.connectOverCDP(cdp, { timeout: 15000 });
    const context = browser.contexts()[0];
    page = await until("workbench window", () => {
      alive();
      return context.pages().find((p) => p.url().includes("workbench"));
    });
    page.setDefaultTimeout(15000);
    await until("installed extension bridge", () => {
      alive();
      const file = path.join(root, "bridge.json");
      if (fs.existsSync(file)) return true;
    });
    const address = JSON.parse(
      fs.readFileSync(path.join(root, "bridge.json"), "utf8"),
    );
    bridge = async (op, params = {}) => {
      const response = await fetch(`http://127.0.0.1:${address.port}`, {
        method: "POST",
        headers: { authorization: `Bearer ${address.token}` },
        body: JSON.stringify({ op, ...params }),
        signal: AbortSignal.timeout(10000),
      });
      const body = await response.json();
      if (!response.ok) throw Error(body.error);
      return body.value;
    };
    await page.bringToFront();
    if (profile.platform === "win32") keyboardState = windowsDesktop("select", child.pid, profile.keyboardLayout);
    const before = await bridge("inspect");
    assert.equal(before.vscodeVersion, profile.vscodeVersion);
    assert.deepEqual(before.settings, profile.settings);
    assert.equal(before.locale, profile.locale);
    assert.equal(before.platform, profile.platform);
    assert.equal(before.arch, profile.arch);
    observedDisplay = await page.evaluate(() => ({
      width: innerWidth,
      height: innerHeight,
      deviceScaleFactor: devicePixelRatio,
      screenWidth: screen.width,
      screenHeight: screen.height,
    }));
    fs.writeFileSync(path.join(root, "observed-display.json"), JSON.stringify(observedDisplay, null, 2));
    assert.deepEqual(
      observedDisplay,
      profile.display,
      "recorded display profile",
    );
    installedServer = path.relative(
      root,
      path.join(before.extensionPath, "server", profile.platform === "win32" ? "vela_lsp_server.exe" : "vela_lsp_server"),
    ).split(path.sep).join("/");
    inputs = evidence.currentInputs(
      repository,
      path.join(root, installedServer),
      profile,
    );
    await verifyInstalledPackage(
      vsix,
      before.extensionPath,
      extensionRoot,
      inputs.serverSha256,
    );
    const requirements = loadInventory(repository).executionRequirements;
    contracts = localContracts(requirements, fixture, profile.platform);
    const contract = contracts[0];
    const proofStarted = Date.now();
    assert.equal(
      before.active.text,
      new FixtureWorkspace(fixture).disk.get("scripts/main.vela").text,
    );
    record("assertion", "installed-profile", { observed: before });
    const input = page.getByRole("textbox", { name: /^main\.vela/ });
    await input.focus();
    const focus = {
      focused: await input.evaluate(
        (element) => element === document.activeElement,
      ),
    };
    assert.deepEqual(
      focus,
      contract.checks[0].expected,
      "editor must own focus",
    );
    record("assertion", "editor-focus", {
      expected: contract.checks[0].expected,
      observed: focus,
    });
    await page.keyboard.type(fixture.oracle.typedText);
    record("input", "type-prefix", {
      device: "keyboard",
      text: fixture.oracle.typedText,
    });
    await page.keyboard.press("Control+Space");
    record("input", "open-suggestions", {
      device: "keyboard",
      key: "Control+Space",
    });
    const widget = page.locator(".suggest-widget.visible");
    await widget.waitFor({ state: "visible" });
    const candidate = widget
      .getByRole(contract.checks[1].expected.role)
      .filter({ hasText: fixture.oracle.candidate });
    await candidate.waitFor({ state: "visible" });
    assert.equal(await candidate.count(), 1);
    const rendered = await widget.innerText();
    const visible = {
      label: await candidate.locator(".label-name").innerText(),
      role: await candidate.getAttribute("role"),
      visible: await candidate.isVisible(),
    };
    assert.deepEqual(visible, contract.checks[1].expected);
    record("assertion", "visible-candidate", {
      expected: contract.checks[1].expected,
      observed: visible,
      rendered,
    });
    await page.screenshot({ path: path.join(root, "suggestions.png") });
    fs.writeFileSync(
      path.join(root, "suggestions.aria.txt"),
      await widget.ariaSnapshot(),
    );
    await candidate.click();
    record("input", "accept-candidate", {
      device: "pointer",
      selector: contract.actions.find((action) => action.id === "accept-candidate").selector,
      label: fixture.oracle.candidate,
      clickCount: 1,
    });
    await widget.waitFor({ state: "hidden" });
    const after = await until("final document", async () => {
      const state = await bridge("inspect");
      return state.active.text === fixture.oracle.finalText && state;
    });
    assert.deepEqual(after.active.selections, [
      {
        anchor: fixture.oracle.finalSelection,
        active: fixture.oracle.finalSelection,
      },
    ]);
    assert.ok(after.active.dirty);
    record("assertion", "final-document", {
      expected: contract.checks[2].expected,
      observed: {
        text: after.active.text,
        dirty: after.active.dirty,
        selections: after.active.selections,
      },
    });
    const proofFinished = Date.now();
    proof = {
      id: contract.id,
      fixture: contract.fixture,
      contractHash: evidence.jsonHash(contract),
      status: "passed",
      durationMs: proofFinished - proofStarted,
      startedAt: new Date(proofStarted).toISOString(),
      finishedAt: new Date(proofFinished).toISOString(),
      actions: trace
        .filter((item) => item.kind === "input")
        .map(({ kind, at, proof: owner, ...action }) => action),
      checks: [
        { ...contract.checks[0], status: "passed", observed: focus },
        { ...contract.checks[1], status: "passed", observed: visible },
        {
          ...contract.checks[2],
          status: "passed",
          observed: {
            text: after.active.text,
            dirty: after.active.dirty,
            selections: after.active.selections,
          },
        },
      ],
    };
    await page.screenshot({ path: path.join(root, "final.png") });
    proofs.push(proof);
    await require("./navigation").runNavigation({
      page, bridge, record, root, workspace, contracts, until, onProof: (proof) => proofs.push(proof),
    });
    // A child may collect selected new proofs while preserving the shared
    // driver/dirty-navigation prerequisites. Strict gates still require every
    // owned route; absent proofs are never treated as passed or N/A.
    const requestedProofs = [];
    for (let index = 2; index < process.argv.length; index += 2) {
      if (process.argv[index] !== "--proof" || !process.argv[index + 1]) throw Error("use --proof <ux03-to-ux06-proof-id>");
      const id = process.argv[index + 1];
      if (!/^ux0[3456]-/.test(id) || !contracts.some((item) => item.id === id) || requestedProofs.includes(id))
        throw Error(`unknown or duplicate proof: ${id}`);
      requestedProofs.push(id);
    }
    record("observation", "requested-proofs", { ids: requestedProofs.length ? requestedProofs : contracts.map((item) => item.id) });
    await require("./peek").runPeek({
      page, bridge, record, root, workspace, contracts: requestedProofs.length ? contracts.filter((item) => requestedProofs.includes(item.id)) : contracts, until, pid: child.pid, platform: profile.platform, onProof: (proof) => proofs.push(proof),
    });
    await require("./completion").runCompletion({
      page, bridge, record, root, workspace, contracts: requestedProofs.length ? contracts.filter((item) => requestedProofs.includes(item.id)) : contracts,
      until, onProof: (proof) => proofs.push(proof),
    });
    await require("./rename").runRename({
      page, bridge, record, root, workspace, contracts: requestedProofs.length ? contracts.filter((item) => requestedProofs.includes(item.id)) : contracts,
      until, onProof: (proof) => proofs.push(proof),
    });
    await require("./references").runReferences({
      page, bridge, record, root, workspace, contracts: requestedProofs.length ? contracts.filter((item) => requestedProofs.includes(item.id)) : contracts,
      until, onProof: (proof) => proofs.push(proof),
    });
    restoreKeyboard();
    await bridge("finish");
    const completed = await until("workbench exit", () => exit, 15000);
    assert.equal(completed.code, 0);
    assert.equal(completed.signal, null);
    record("assertion", "clean-exit", { observed: completed });
    assert.deepEqual(
      evidence.currentInputs(
        repository,
        path.join(root, installedServer),
        profile,
      ),
      inputs,
      "source inputs changed during input run",
    );
    console.log(
      "PASS installed VSIX keyboard, pointer, visible suggestion and final document",
    );
  } catch (failure) {
    error = failure;
    record("failure", "driver", { error: failure.stack });
    if (page) {
      await page
        .screenshot({ path: path.join(root, "failure.png") })
        .catch(() => {});
      fs.writeFileSync(
        path.join(root, "failure.aria.txt"),
        await page
          .locator("body")
          .ariaSnapshot()
          .catch(() => "unavailable"),
      );
    }
  } finally {
    clearTimeout(timer);
    try { restoreKeyboard(); } catch (failure) { error ??= failure; }
    if (!exit) {
      child.kill("SIGTERM");
      await Promise.race([exited, delay(3000)]);
      if (!exit) child.kill("SIGKILL");
    }
    if (browser) await browser.close().catch(() => {});
    try {
      await until("process streams closed", () => closed, 5000);
    } catch (closeFailure) {
      error ??= closeFailure;
      child.stdout.destroy();
      child.stderr.destroy();
    }
    await new Promise((resolve) => log.end(resolve));
    if (bridge) {
      try { require("./logs").retainLogs(root, workspace); }
      catch (failure) { error ??= failure; }
    }
    const artifacts =
      fs.existsSync(path.join(root, "final.png")) && installedServer
        ? [
            "trace.json",
            "suggestions.png",
            "suggestions.aria.txt",
            "final.png",
            "workbench.log",
            "vela.vsix",
            installedServer,
            ...new Set(proofs.flatMap((proof) => contracts.find((item) => item.id === proof.id).artifacts)
              .filter((file) => !["trace.json", "suggestions.png", "suggestions.aria.txt", "final.png", "workbench.log"].includes(file))),
          ].map((file) => evidence.artifact(root, file))
        : [];
    const result = {
      version: 2,
      status: error ? "failed" : "passed",
      profile,
      observedDisplay,
      inputs,
      proofs,
      exit,
      vsix: "vela.vsix",
      vsixSha256: evidence.fileHash(vsix),
      installedServer,
      artifacts,
    };
    fs.writeFileSync(
      path.join(root, "results.json"),
      JSON.stringify(result, null, 2),
    );
  }
  if (error) throw error;
}
async function main() {
  assert.equal(process.platform, profile.platform);
  assert.equal(process.arch, profile.arch);
  if (profile.platform === "darwin") await require("./keyboard-layout").withKeyboardLayout(profile.keyboardLayout, run);
  else await run();
}
main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
