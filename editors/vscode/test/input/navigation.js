"use strict";
const assert = require("node:assert/strict");
const path = require("node:path");
const fs = require("node:fs");
const { offsetAt } = require("../../../../scripts/lsp-matrix/fixtures");
const { navigationResponses } = require("../../../../scripts/lsp-matrix/navigation-trace");
const { findLog } = require("./logs");
const { pathToFileURL } = require("node:url");
const evidence = require("../../../../scripts/lsp-matrix/local-evidence");
const { navigationModel } = require("../../../../scripts/lsp-matrix/navigation-contracts");

async function runNavigation({ page, bridge, record, root, workspace, contracts, until, onProof }) {
  const model = navigationModel(), proofs = [];
  const log = findLog(root, (name) => name.endsWith("-Vela LSP Trace.log"));
  const logText = () => fs.readFileSync(log, "utf8");
  const observe = async () => {
    const state = await bridge("inspect"), active = state.active;
    if (!active) return null; // Editor replacement can briefly clear the active editor.
    const file = path.relative(workspace, require("node:url").fileURLToPath(active.uri));
    assert.equal(active.uri, pathToFileURL(path.join(workspace, file)).href,
      "editor URI must match the encoded workspace file URI");
    return { file, text: active.text, dirty: active.dirty, selections: active.selections };
  };
  const setup = async (marker) => {
    const point = model.caller.markers[marker].start;
    await bridge("setup", { file: model.spec.oracle.caller, line: point.line, character: point.character });
    const input = page.getByRole("textbox", { name: /^nav\.vela/ });
    await input.focus();
    await until("origin selection acknowledged", async () => {
      const active = await observe();
      return JSON.stringify(active) === JSON.stringify(model.origin(marker));
    });
    return input;
  };
  for (const contract of contracts.filter((item) => item.fixture === model.spec.id)) {
    const started = Date.now(), checks = [], actions = [];
    const receipt = (kind, id, details) => record(kind, id, { proof: contract.id, ...details });
    const check = (id, observed) => {
      const expected = contract.checks.find((item) => item.id === id);
      assert.ok(expected, `unregistered assertion ${id}`);
      assert.deepEqual(observed, expected.expected, `${contract.id}/${id}`);
      receipt("assertion", id, { expected: expected.expected, observed });
      checks.push({ ...expected, observed, status: "passed" });
    };
    const action = async (id) => {
      const expected = contract.actions.find((item) => item.id === id);
      assert.ok(expected, `unregistered action ${id}`);
      if (expected.device === "command") await bridge("command", { command: expected.command });
      else if (expected.key) await page.keyboard.press(expected.key);
      else await page.keyboard.type(expected.text);
      actions.push(expected);
      receipt(expected.device === "command" ? "command" : "input", id,
        Object.fromEntries(Object.entries(expected).filter(([key]) => key !== "id")));
    };
    const destination = async (id) => {
      const expected = contract.checks.find((item) => item.id === id).expected;
      const observed = await until(id, async () => {
        const state = await observe();
        return JSON.stringify(state) === JSON.stringify(expected) && state;
      });
      check(id, observed);
    };
    const palette = async (prefix) => {
      await action(`${prefix}-open`);
      const widget = page.locator(".quick-input-widget");
      await widget.waitFor({ state: "visible" });
      await action(`${prefix}-query`);
      const title = contract.actions.find((item) => item.id === `${prefix}-query`).text;
      const label = widget.locator(".label-name").filter({ hasText: new RegExp(`^${title}$`) });
      await label.waitFor({ state: "visible" });
      assert.equal(await label.count(), 1, "palette command must be unambiguous");
      receipt("observation", `${prefix}-palette`, { title: await label.innerText(), visible: true });
      await action(`${prefix}-accept`);
      await widget.waitFor({ state: "hidden" });
    };
    let boundary;
    const wire = async (kind) => {
      const expected = contract.checks.find((item) => item.id === `wire-${kind}`).expected;
      const responses = await until(`completed ${kind} request after action`, () => {
        const entries = navigationResponses(logText().slice(boundary)).filter((entry) =>
          entry.method === expected.method &&
          entry.params.textDocument.uri === pathToFileURL(path.join(workspace, expected.request.file)).href &&
          entry.params.position.line === expected.request.position.line &&
          entry.params.position.character === expected.request.position.character);
        return entries.length > 0 && entries;
      });
      assert.equal(responses.length, 1, "the action must have one matching completed request");
      const response = responses[0];
      let result = null;
      if (response.result !== null) {
        const value = Array.isArray(response.result) ? response.result[0] : response.result;
        if (Array.isArray(response.result)) assert.equal(response.result.length, 1);
        const uri = value.targetUri ?? value.uri;
        const range = value.targetSelectionRange ?? value.range;
        const document = (await bridge("inspect")).documents.find((doc) => doc.uri === uri);
        assert.ok(document, "the native action must have opened the returned target");
        result = { file: path.relative(workspace, require("node:url").fileURLToPath(uri)), range,
          text: document.text.slice(offsetAt(document.text, range.start), offsetAt(document.text, range.end)) };
      }
      // Normalize property order only; compare the actual method, request and response.
      check(`wire-${kind}`, { method: response.method,
        request: { file: path.relative(workspace, require("node:url").fileURLToPath(response.params.textDocument.uri)),
          position: response.params.position }, result });
      receipt("observation", `${kind}-request-id`, { requestId: response.id });
    };
    const inputRoute = contract.id.endsWith("-input");
    if (contract.id === "ux02-definition-back-input") {
      await bridge("setup", { file: model.spec.oracle.caller, line: 0, character: 0 });
      await page.getByRole("textbox", { name: /^nav\.vela/ }).focus();
      await action("dirty-home");
      await action("dirty-prefix");
      const state = await bridge("inspect");
      check("unopened-target", { unopened: !state.documents.some((doc) =>
        doc.uri === pathToFileURL(path.join(workspace, model.spec.oracle.definitionFile)).href) });
    }
    const marker = contract.id.includes("unknown-target") ? "unknown" :
      contract.id.includes("type-definition") ? "typed-use" : "call";
    const input = await setup(marker);
    check("origin", await observe());
    check("editor-focus", { focused: await input.evaluate((element) => element === document.activeElement) });
    boundary = logText().length;
    if (contract.id.includes("definition-back")) {
      await action(inputRoute ? "definition-key" : "definition-command");
      await destination("destination");
      await wire("definition");
      await action(inputRoute ? "back-key" : "back-command");
      await destination("restored-origin");
    } else if (contract.id.includes("declaration-palette")) {
      if (inputRoute) await palette("declaration"); else await action("declaration-command");
      await destination("destination");
      await wire("declaration");
    } else if (contract.id.includes("type-definition-palette")) {
      if (inputRoute) await palette("type"); else await action("type-command");
      await destination("destination");
      await wire("type");
    } else {
      for (const kind of ["definition", "declaration", "type"]) {
        if (inputRoute && kind !== "definition") await palette(`unknown-${kind}`);
        else await action(`unknown-${kind}`);
        if (inputRoute) {
          const message = page.locator('[id="workbench.parts.editor"]').getByText(new RegExp(`^No ${kind === "type" ? "type definition" : kind} found for`));
          await message.waitFor({ state: "visible" });
          check(`empty-${kind}`, { text: await message.innerText(), visible: await message.isVisible() });
        }
        await wire(kind);
        await destination(`unchanged-${kind}`);
      }
    }
    await page.screenshot({ path: path.join(root, `${contract.id}.png`) });
    const finished = Date.now();
    proofs.push({ id: contract.id, fixture: contract.fixture, contractHash: evidence.jsonHash(contract),
      status: "passed", durationMs: finished - started,
      startedAt: new Date(started).toISOString(), finishedAt: new Date(finished).toISOString(), actions, checks });
    onProof(proofs.at(-1));
    console.log(`PASS ${contract.id}`);
  }
  return proofs;
}
module.exports = { runNavigation };
