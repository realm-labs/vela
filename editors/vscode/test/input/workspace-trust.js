"use strict";
const fs = require("node:fs"), path = require("node:path"), assert = require("node:assert/strict");
const { fileURLToPath } = require("node:url"), { isDeepStrictEqual } = require("node:util");
const { workspaceTrustModel } = require("../../../../scripts/lsp-matrix/workspace-trust-contracts");
const { navigationResponses } = require("../../../../scripts/lsp-matrix/navigation-trace");
const { offsetAt } = require("../../../../scripts/lsp-matrix/fixtures");
const evidence = require("../../../../scripts/lsp-matrix/local-evidence");
const { relativeFile, canonicalUri, fileUri } = require("./paths");
const { readSession, sessionLog } = require("./session");
async function runWorkspaceTrust({ page, bridge, record, root, owned, workspace, profile, contracts, until, onProof }) {
  const m = workspaceTrustModel(profile.platform), o = m.o, sessions = [];
  const uriFile = uri => relativeFile(workspace, fileURLToPath(canonicalUri(uri)));
  const disk = file => fs.readFileSync(path.join(workspace, file), "utf8");
  const traceFile = path.join(workspace, ".vela-lsp-trace.jsonl");
  const activeState = async () => {
    const editor = (await bridge("inspect")).activeEditor; if (!editor) return null;
    const relative = path.relative(workspace, fileURLToPath(canonicalUri(editor.uri)));
    if (!relative || relative.startsWith("..") || path.isAbsolute(relative)) return null;
    const file = uriFile(editor.uri); return { file, text: editor.text, disk: disk(file), dirty: editor.dirty, selections: editor.selections };
  };
  const trace = () => fs.readFileSync(sessionLog(readSession(owned), name => name.endsWith("-Vela LSP Trace.log")), "utf8");
  const output = () => { const file = sessionLog(readSession(owned), name => /-Vela\.log$/.test(name), true); return file ? fs.readFileSync(file, "utf8") : ""; };
  const state = async () => { const s = await bridge("inspect"); assert.equal(s.configured, m.configured);
    assert.deepEqual(s.folders, [fileUri(workspace)]); assert.equal(s.vscodeVersion, profile.vscodeVersion); assert.equal(s.platform, profile.platform); assert.equal(s.arch, profile.arch); assert.equal(s.locale, profile.locale);
    assert.deepEqual(s.settings, profile.settings);
    return { trusted: s.trusted, active: s.active, serverStarted: output().includes("Language server started."), serverTraceExists: fs.existsSync(traceFile) }; };
  const location = r => { const file = uriFile(r.uri), text = disk(file); return { file, range: r.range, text: text.slice(offsetAt(text, r.range.start), offsetAt(text, r.range.end)) }; };
  const saveSession = () => {
    const s = readSession(owned); sessions.push({ pid: s.pid, logDirectory: path.relative(owned, s.logDirectory).replaceAll("\\", "/") });
    fs.writeFileSync(path.join(owned, "sessions.json"), JSON.stringify(sessions, null, 2));
  };
  for (const contract of contracts.filter(c => c.id.startsWith("ux21-"))) {
    const started = Date.now(), actions = [], checks = [];
    const receipt = (kind, id, details) => record(kind, id, { proof: contract.id, ...details });
    const expected = id => { const c = contract.checks.find(c => c.id === id); assert(c, "known trust check " + id); return c; };
    const check = (id, value) => { const c = expected(id); assert.deepEqual(value, c.expected, contract.id + "/" + id); checks.push({ ...c, observed: value, status: "passed" }); receipt("assertion", id, { expected: c.expected, observed: value }); };
    const awaitCheck = async (id, read) => check(id, await until(id, async () => { const value = await read(); receipt("observation", id, { observed: value }); return isDeepStrictEqual(value, expected(id).expected) && value; }));
    const folderDialog = () => page.locator(".quick-input-widget:visible"), input = () => folderDialog().locator("input").filter({ visible: true });
    const pointer = async a => {
      let target;
      if (a.selector === "Do not trust startup folder") target = page.getByRole("button", { name: "No, I don't trust the authors", exact: true });
      else if (a.selector === "Workspace Trust grant button") target = page.getByRole("button", { name: "Trust", exact: true });
      // NativeEditContext's textbox is covered by Monaco's painted line. Click
      // that actual rendered search surface, then type into its native focus.
      else if (a.selector === "Extensions search") target = page.locator('[id="workbench.view.extensions"] .suggest-input-container .monaco-editor .view-lines');
      else if (a.selector === "Installed Vela extension") target = page.locator('[id="workbench.view.extensions"]').getByText("Vela", { exact: true });
      else if (a.selector.startsWith("Quick Open file ") || a.selector.startsWith("Command palette ")) {
        const label = a.selector.startsWith("Quick Open file ") ? path.posix.basename(a.selector.slice(16)) : a.selector.slice(16);
        target = folderDialog().locator(".label-name").filter({ hasText: new RegExp("^" + label.replace(/[.*+?^${}()|[\]\\]/g, "\\$&") + "$") });
      } else throw Error("unknown trust pointer " + a.selector);
      await target.waitFor({ state: "visible" }); assert.equal(await target.count(), 1); receipt("observation", a.id + "-target", { aria: await target.ariaSnapshot() }); await target.click();
      if (a.selector.startsWith("Quick Open file ")) await until("owned trust file open", async () => {
        const editor = (await bridge("inspect")).activeEditor; return editor && canonicalUri(editor.uri) === fileUri(path.join(workspace, a.selector.slice(16)));
      });
    };
    let boundary;
    for (const a of contract.actions) {
      if (a.id === "untrusted-native") {
        await awaitCheck("untrusted-source", activeState); await awaitCheck("untrusted-state", state); check("policy", (await bridge("inspect")).policy);
        saveSession();
      }
      if (a.id === "permitted-native") {
        await awaitCheck("trusted-state", state); await awaitCheck("trusted-source", activeState);
        check("trusted-provider", (await bridge("query")).map(location)); boundary = trace().length; saveSession();
      }
      if (a.device === "pointer") await pointer(a);
      else if (a.path !== undefined) {
        const physical = path.join(workspace, a.path); receipt("observation", a.id + "-resolved", { base: workspace, path: a.path, text: physical.replaceAll("\\", "/") }); await page.keyboard.insertText(physical.replaceAll("\\", "/"));
      } else if (a.text !== undefined) await page.keyboard.insertText(a.text);
      else await page.keyboard.press(a.key);
      actions.push(a); receipt("input", a.id, Object.fromEntries(Object.entries(a).filter(([k]) => k !== "id")));
      if (a.id.endsWith("-open") || a.id.endsWith("-goto")) { await input().waitFor({ state: "visible" }); assert(await input().evaluate(el => document.activeElement === el)); }
      if (a.id.endsWith("-place")) await folderDialog().waitFor({ state: "hidden" });
      if (a.id === "untrusted-native") { check("untrusted-provider", (await bridge("query")).map(location)); await awaitCheck("untrusted-caret", activeState); }
      if (a.id === "permitted-native") {
        const rows = await until("one complete trusted native definition", () => { const rows = navigationResponses(trace().slice(boundary)); return rows.length && rows; });
        assert.equal(rows.length, 1); const r = rows[0]; assert(r.result && !Array.isArray(r.result));
        check("trusted-wire", { method: r.method, request: { file: uriFile(r.params.textDocument.uri), position: r.params.position }, result: location(r.result) });
        await awaitCheck("trusted-destination", activeState);
        const source = (await bridge("inspect")).documents.find(d => canonicalUri(d.uri) === fileUri(path.join(workspace, o.caller))); assert(source);
        check("trusted-dirty-caller", { file: o.caller, text: source.text, disk: disk(o.caller), dirty: source.dirty });
      }
      if (a.id === "installed-vela") {
        const label = page.locator('[id="workbench.parts.editor"]').getByText(o.policyDescription, { exact: true });
        await label.waitFor({ state: "visible" }); assert.equal(await label.count(), 1);
        const raw = await label.innerText(); receipt("observation", "extension-status-text", { text: raw });
        // The status icon contributes a leading layout NBSP. Compare the whole
        // semantic description, as the existing hover paragraph contracts do.
        check("untrusted-extension", { visible: await label.isVisible(), description: raw.replaceAll("\u00a0", " ").trim() });
        await page.screenshot({ path: path.join(root, contract.id + "-extension.png") });
        fs.writeFileSync(path.join(root, contract.id + "-extension.aria.txt"), await page.locator('[id="workbench.parts.editor"]').ariaSnapshot());
      }
    }
    const untrusted = contract.id === "ux21-untrusted-open";
    if (untrusted) {
      const target = page.getByText(o.banner, { exact: true }); await target.waitFor({ state: "visible" }); assert.equal(await target.count(), 1);
      check("untrusted-banner", { visible: await target.isVisible(), text: await target.innerText() });
      await page.getByText(o.untrustedHeading, { exact: true }).waitFor({ state: "visible" });
      check("untrusted-trust-editor", { heading: await page.getByText(o.untrustedHeading, { exact: true }).innerText(), trustButton: await page.getByRole("button", { name: "Trust", exact: true }).innerText(), visible: true });
    } else {
      check("trusted-banner", { visible: await page.getByText(o.banner, { exact: true }).isVisible() });
      await page.getByText(o.trustedHeading, { exact: true }).waitFor({ state: "visible" });
      check("trusted-trust-editor", { heading: await page.getByText(o.trustedHeading, { exact: true }).innerText(), visible: true });
      await awaitCheck("trusted-diagnostics", () => {
        const pubs = trace().split("\n\n\n").slice(0, -1).flatMap(block => { const match = block.match(/Received notification 'textDocument\/publishDiagnostics'\.\nParams: ([\s\S]+)$/); if (!match) return [];
          const p = JSON.parse(match[1]); assert.equal(Object.hasOwn(p, "version"), false); return [{ ...p, uri: uriFile(p.uri) }]; });
        return [o.caller, o.target].map(file => pubs.filter(p => p.uri === file).at(-1));
      });
      const setup = JSON.parse(fs.readFileSync(path.join(owned, "setup.json"), "utf8"));
      const events = fs.readFileSync(traceFile, "utf8").split("\n").filter(Boolean).map(line => JSON.parse(line));
      check("trusted-owned-server", { sessions: events.filter(e => e.event === "session_start").length,
        configuredBinaryMatchesInstalled: evidence.fileHash(path.join(workspace, m.configured)) === setup.installedBinarySha256,
        launchedConfiguredCommand: output().includes("Starting native language server: " + m.configured + " --stdio ") });
      fs.copyFileSync(sessionLog(readSession(owned), n => n.endsWith("-Vela LSP Trace.log")), path.join(owned, "lsp-trace.log"));
      fs.copyFileSync(sessionLog(readSession(owned), n => /-Vela\.log$/.test(n)), path.join(owned, "vela-output.log")); fs.copyFileSync(traceFile, path.join(owned, "server-trace.jsonl"));
    }
    saveSession(); assert.deepEqual(checks.map(c => c.id).sort(), contract.checks.map(c => c.id).sort());
    await page.screenshot({ path: path.join(root, contract.id + ".png") }); fs.writeFileSync(path.join(root, contract.id + ".aria.txt"), await page.locator("body").ariaSnapshot());
    const finished = Date.now(); assert(finished - started <= contract.deadlineMs);
    onProof({ id: contract.id, fixture: contract.fixture, contractHash: evidence.jsonHash(contract), status: "passed", startedAt: new Date(started).toISOString(), finishedAt: new Date(finished).toISOString(), durationMs: finished - started, actions, checks });
    console.log("PASS " + contract.id);
  }
}
module.exports = { runWorkspaceTrust };
