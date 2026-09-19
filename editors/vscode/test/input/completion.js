"use strict";
const fs = require("node:fs");
const path = require("node:path");
const assert = require("node:assert/strict");
const { fileURLToPath } = require("node:url");
const { completionModel } = require("../../../../scripts/lsp-matrix/completion-contracts");
const evidence = require("../../../../scripts/lsp-matrix/local-evidence");
const { relativeFile, fileUri } = require("./paths");

async function runCompletion({ page, bridge, record, root, workspace, contracts, until, onProof }) {
  for (const contract of contracts.filter((item) => item.id.startsWith("ux04-"))) {
    const route = contract.id.slice(5), model = completionModel(route);
    const started = Date.now(), actions = [], checks = [];
    const receipt = (kind, id, details) => record(kind, id, { proof: contract.id, ...details });
    const check = (id, observed) => {
      const required = contract.checks.find((item) => item.id === id);
      assert.ok(required, `unregistered assertion ${id}`);
      assert.deepEqual(observed, required.expected, `${contract.id}/${id}`);
      checks.push({ ...required, status: "passed", observed });
      receipt("assertion", id, { expected: required.expected, observed });
    };
    const state = async (id) => {
      const expected = contract.checks.find((item) => item.id === id).expected;
      check(id, await until(id, async () => {
        const active = (await bridge("inspect")).active;
        if (!active) return false;
        const file = relativeFile(workspace, fileURLToPath(active.uri));
        assert.equal(active.uri, fileUri(path.join(workspace, file)));
        const observed = { file, text: active.text, dirty: active.dirty, selections: active.selections };
        fs.writeFileSync(path.join(root, "completion-state.json"), JSON.stringify({ id, expected, observed }, null, 2));
        return JSON.stringify(observed) === JSON.stringify(expected) && observed;
      }));
    };
    const action = async (id) => {
      const item = contract.actions.find((item) => item.id === id);
      assert.ok(item, `unregistered input ${id}`);
      if (item.text !== undefined) await page.keyboard.type(item.text);
      else await page.keyboard.press(item.key);
      actions.push(item);
      receipt("input", id, Object.fromEntries(Object.entries(item).filter(([key]) => key !== "id")));
    };
    await bridge("setup", { file: model.file, ...model.cursor });
    const name = path.basename(model.file);
    const input = page.getByRole("textbox", { name: new RegExp(`^${name.replaceAll(".", "\\.")}`) });
    await input.focus();
    await state("origin");
    const focus = async (id) => check(id, { focused: await input.evaluate((element) => element === document.activeElement) });
    await focus("editor-focus");
    await action("clear-prior-widget");
    await action("type-prefix");
    await state("typed-source");
    await action("open-suggestions");
    const widget = page.locator(".suggest-widget.visible");
    await widget.waitFor({ state: "visible" });
    const labels = await until("complete suggestion list", async () => {
      const values = await widget.locator(".monaco-list-row .label-name").allTextContents();
      return values.length > 0 && values;
    });
    check("visible-candidates", { labels, visible: await widget.isVisible() });
    if (route !== "dismiss-escape") {
      await action("select-next");
      const selected = widget.locator(".monaco-list-row.focused .label-name");
      await until("selected beta candidate", async () => await selected.textContent() === model.spec.oracle.labels[1]);
      check("selected-candidate", { label: await selected.textContent(), visible: await selected.isVisible() });
      await action("open-details");
      const docs = page.locator(".suggest-details .docs");
      await docs.waitFor({ state: "visible" });
      await until("resolved documentation", async () => (await docs.innerText()).trim() === model.spec.oracle.documentation);
      check("resolved-documentation", { text: (await docs.innerText()).trim(), visible: await docs.isVisible() });
    }
    await page.screenshot({ path: path.join(root, `${contract.id}-open.png`) });
    fs.writeFileSync(path.join(root, `${contract.id}-open.aria.txt`), await page.locator('[id="workbench.parts.editor"]').ariaSnapshot());
    if (route !== "dismiss-escape") await action("close-details");
    await action("finish-completion");
    await widget.waitFor({ state: "hidden" });
    check("widget-hidden", { visible: await widget.isVisible() });
    await state("final-source");
    await focus("restored-focus");
    if (route !== "dismiss-escape") {
      await action("undo-completion");
      await state("undo-source");
    }
    await page.screenshot({ path: path.join(root, `${contract.id}.png`) });
    fs.writeFileSync(path.join(root, `${contract.id}.aria.txt`), await page.locator('[id="workbench.parts.editor"]').ariaSnapshot());
    const finished = Date.now();
    onProof({ id: contract.id, fixture: contract.fixture, contractHash: evidence.jsonHash(contract), status: "passed",
      durationMs: finished - started, startedAt: new Date(started).toISOString(), finishedAt: new Date(finished).toISOString(), actions, checks });
    console.log(`PASS ${contract.id}`);
  }
}
module.exports = { runCompletion };
