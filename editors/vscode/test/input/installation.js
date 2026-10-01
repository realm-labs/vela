"use strict";
const assert = require("node:assert/strict"), fs = require("node:fs"), path = require("node:path");
const { fileURLToPath } = require("node:url");
const { fileUri, canonicalUri, relativeFile } = require("./paths");
const { navigationResponses } = require("../../../../scripts/lsp-matrix/navigation-trace");
const { installationModel } = require("../../../../scripts/lsp-matrix/installation-contracts");
const evidence = require("../../../../scripts/lsp-matrix/local-evidence");
const { findLog } = require("./logs");

async function runInstallation({ page, bridge, record, root, workspace, contracts, until, onProof }) {
  const m = installationModel();
  const state = async () => {
    const active = (await bridge("inspect")).active;
    if (!active) return null;
    return { file: relativeFile(workspace, fileURLToPath(active.uri)), text: active.text,
      languageId: active.languageId, dirty: active.dirty, selections: active.selections };
  };
  const status = async () => {
    const observed = await bridge("inspect");
    let started = false;
    if (observed.extensionActive) started = fs.readFileSync(findLog(root, name => /-Vela\.log$/.test(name)), "utf8").includes("Language server started.");
    return { active: observed.extensionActive, bundled: observed.serverPath === "", started };
  };
  for (const contract of contracts.filter(c => c.id.startsWith("ux01-"))) {
    const started = Date.now(), actions = [], checks = [];
    const receipt = (kind, id, details) => record(kind, id, { proof: contract.id, ...details });
    const check = (id, observed) => {
      const expected = contract.checks.find(c => c.id === id);
      assert(expected, `unknown installation check ${id}`);
      assert.deepEqual(observed, expected.expected, `${contract.id}/${id}`);
      receipt("assertion", id, { expected: expected.expected, observed });
      checks.push({ ...expected, observed, status: "passed" });
    };
    const destination = async id => {
      const expected = contract.checks.find(c => c.id === id).expected;
      check(id, await until(id, async () => { const s = await state(); return JSON.stringify(s) === JSON.stringify(expected) && s; }));
    };
    const action = async id => {
      const a = contract.actions.find(a => a.id === id);
      assert(a, `unregistered installation action ${id}`);
      let value;
      if (a.device === "command") {
        value = a.command === "editor.action.revealDefinition" ? await bridge("command", { command: a.command }) :
          await bridge("installation-command", { command: a.command, file: a.file });
      } else if (a.key) await page.keyboard.press(a.key);
      else await page.keyboard.type(a.text);
      actions.push(a);
      receipt(a.device === "command" ? "command" : "input", id, Object.fromEntries(Object.entries(a).filter(([key]) => key !== "id")));
      return value;
    };
    await action("open-picker");
    const picker = page.locator(".quick-input-widget");
    await picker.waitFor({ state: "visible" });
    await action("file-name");
    const file = contract.actions.find(a => a.id === "file-name").text;
    const candidate = picker.locator(".label-name").filter({ hasText: new RegExp(`^${path.basename(file).replaceAll(".", "\\.")}$`) });
    await candidate.waitFor({ state: "visible" });
    assert.equal(await candidate.count(), 1, "native open must select the authored file unambiguously");
    await action("open-file");
    await picker.waitFor({ state: "hidden" });
    await until("native file open", async () => (await state())?.file === file);
    await action("goto-line");
    await picker.waitFor({ state: "visible" });
    await action("call-position");
    await action("place-cursor");
    await picker.waitFor({ state: "hidden" });
    await destination("native-source");
    const positive = contract.id === "ux01-install-open";
    if (positive) check("activated", await until("first native activation", async () => { const s = await status(); return s.active && s.started && s; }));
    let boundary;
    const trace = () => fs.readFileSync(findLog(root, name => name.endsWith("-Vela LSP Trace.log")), "utf8");
    const wire = async id => {
      const responses = await until(id, () => {
        const r = navigationResponses(trace().slice(boundary)).filter(r => canonicalUri(r.params.textDocument.uri) === fileUri(path.join(workspace, file)));
        return r.length && r;
      });
      assert.equal(responses.length, 1, "each action must issue exactly one completed definition request");
      const r = responses[0];
      // LSP permits a single Location as well as a one-element location list.
      const targets = Array.isArray(r.result) ? r.result : [r.result];
      assert.equal(targets.length, 1);
      assert(targets[0], "native definition must return an owned target");
      const target = targets[0], uri = target.targetUri ?? target.uri, range = target.targetSelectionRange ?? target.range;
      const text = (await bridge("inspect")).documents.find(d => d.uri === canonicalUri(uri)).text;
      const { offsetAt } = require("../../../../scripts/lsp-matrix/fixtures");
      check(id, { method: r.method, request: { file: relativeFile(workspace, fileURLToPath(r.params.textDocument.uri)), position: r.params.position },
        result: { file: relativeFile(workspace, fileURLToPath(uri)), range, text: text.slice(offsetAt(text, range.start), offsetAt(text, range.end)) } });
      receipt("observation", `${id}-request-id`, { requestId: r.id });
    };
    if (positive) boundary = trace().length;
    await action("native-query");
    if (positive) { await destination("native-destination"); await wire("native-wire"); }
    else {
      // VS Code guards F12 with editorHasDefinitionProvider. Plaintext has no
      // provider, so the required negative behavior is unchanged source and
      // inactive Vela, rather than a navigation-provider message.
      await destination("native-unchanged");
      check("native-inactive", await status());
    }
    await action("command-open");
    await destination("command-source");
    if (positive) boundary = trace().length;
    const result = await action("command-query");
    if (positive) { await destination("command-destination"); await wire("command-wire"); }
    else { check("command-empty", result); check("command-inactive", await status()); }
    check("disk-inputs", Object.fromEntries(Object.keys(m.disk).map(f => [f, fs.readFileSync(path.join(workspace, f), "utf8")])));
    await page.screenshot({ path: path.join(root, `${contract.id}.png`) });
    const finished = Date.now();
    onProof({ id: contract.id, fixture: contract.fixture, contractHash: evidence.jsonHash(contract), status: "passed",
      durationMs: finished - started, startedAt: new Date(started).toISOString(), finishedAt: new Date(finished).toISOString(), actions, checks });
    console.log(`PASS ${contract.id}`);
  }
}
module.exports = { runInstallation };
