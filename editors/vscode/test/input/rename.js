"use strict";
const fs = require("node:fs");
const path = require("node:path");
const assert = require("node:assert/strict");
const { fileURLToPath } = require("node:url");
const { renameModel } = require("../../../../scripts/lsp-matrix/rename-contracts");
const evidence = require("../../../../scripts/lsp-matrix/local-evidence");
const { relativeFile, fileUri } = require("./paths");

async function runRename({ page, bridge, record, root, workspace, contracts, until, onProof }) {
  const model = renameModel();
  for (const contract of contracts.filter((item) => item.id.startsWith("ux05-"))) {
    const started = Date.now(), actions = [], checks = [];
    const receipt = (kind, id, details) => record(kind, id, { proof: contract.id, ...details });
    const check = (id, observed) => {
      const required = contract.checks.find((item) => item.id === id);
      assert.ok(required, `unregistered assertion ${id}`);
      assert.deepEqual(observed, required.expected, `${contract.id}/${id}`);
      checks.push({ ...required, status: "passed", observed });
      receipt("assertion", id, { expected: required.expected, observed });
    };
    const action = async (id) => {
      const item = contract.actions.find((item) => item.id === id);
      assert.ok(item, `unregistered input ${id}`);
      if (item.text !== undefined) await page.keyboard.type(item.text);
      else await page.keyboard.press(item.key);
      actions.push(item);
      receipt("input", id, Object.fromEntries(Object.entries(item).filter(([key]) => key !== "id")));
    };
    const observe = async () => {
      const inspection = await bridge("inspect");
      const active = inspection.active;
      if (!active) return null;
      const file = relativeFile(workspace, fileURLToPath(active.uri));
      assert.equal(file, model.spec.oracle.openFile);
      assert.equal(active.uri, fileUri(path.join(workspace, file)));
      const document = (file) => {
        const disk = fs.readFileSync(path.join(workspace, file), "utf8");
        const open = inspection.documents.find((item) => item.uri === fileUri(path.join(workspace, file)));
        return { text: open?.text ?? disk, dirty: open?.dirty ?? false, disk };
      };
      const origin = document("scripts/rename_origin.vela");
      const closed = document(model.spec.oracle.closedFile);
      return {
        openText: active.text,
        openDirty: active.dirty,
        openDisk: fs.readFileSync(path.join(workspace, file), "utf8"),
        originText: origin.text,
        originDirty: origin.dirty,
        originDisk: origin.disk,
        closedText: closed.text,
        closedDirty: closed.dirty,
        closedDisk: closed.disk,
      };
    };
    const state = async (id) => {
      const expected = contract.checks.find((item) => item.id === id).expected;
      let last;
      const observed = await until(id, async () => {
        const actual = await observe();
        last = actual;
        fs.writeFileSync(path.join(root, `${contract.id}-${id}.json`), JSON.stringify({ expected, actual }, null, 2));
        return actual && JSON.stringify(actual) === JSON.stringify(expected) && actual;
      }).catch((error) => { throw Error(`${error.message}: ${JSON.stringify(last)}`); });
      check(id, observed);
    };
    await bridge("setup", { file: model.spec.oracle.openFile, ...model.cursor });
    const editor = page.getByRole("textbox", { name: /^rename_open\.vela/ });
    await editor.focus();
    await state("origin");
    const documents = (await bridge("inspect")).documents;
    check("unopened-targets", {
      origin: !documents.some((doc) => doc.uri === fileUri(path.join(workspace, "scripts/rename_origin.vela"))),
      closed: !documents.some((doc) => doc.uri === fileUri(path.join(workspace, model.spec.oracle.closedFile))),
    });
    check("editor-focus", { focused: await editor.evaluate((element) => element === document.activeElement) });
    await action("open-rename");
    const widget = page.locator(".monaco-editor.rename-box");
    await widget.waitFor({ state: "visible" });
    const input = widget.locator("input");
    check("rename-widget", { visible: await widget.isVisible(), value: await input.inputValue() });
    await until("rename input focus", () => input.evaluate((element) => element === document.activeElement));
    check("rename-focus", { focused: await input.evaluate((element) => element === document.activeElement) });
    await action("select-name");
    await action("type-name");
    check("typed-name", { visible: await widget.isVisible(), value: await input.inputValue() });
    await page.screenshot({ path: path.join(root, `${contract.id}-open.png`) });
    fs.writeFileSync(path.join(root, `${contract.id}-open.aria.txt`), await page.locator('[id="workbench.parts.editor"]').ariaSnapshot());
    if (contract.id === "ux05-rename-confirm") {
      await action("confirm-rename");
      await widget.waitFor({ state: "hidden" });
      check("widget-hidden", { visible: await widget.isVisible() });
      await state("renamed-workspace");
      const confirmation = page.getByRole("alert").filter({ hasText: "Successfully renamed 'grant' to 'award'" });
      await confirmation.waitFor({ state: "visible" });
      check("rename-confirmation", { text: await confirmation.innerText() });
      await action("undo-rename");
      await state("undo-workspace");
      await action("redo-rename");
      await state("redo-workspace");
    } else {
      const cancel = contract.id === "ux05-rename-cancel";
      await action(cancel ? "cancel-rename" : "submit-name");
      if (!cancel) {
        const newName = contract.actions.find((item) => item.id === "type-name").text;
        const message = await until("visible rename rejection", async () => {
          const alerts = await page.getByRole("alert").allTextContents();
          const widgetText = await widget.isVisible() ? await widget.innerText() : "";
          const candidates = [...alerts, widgetText].map((text) => text.trim()).filter(Boolean);
          fs.writeFileSync(path.join(root, `${contract.id}-rejection.json`), JSON.stringify(candidates, null, 2));
          return candidates.find((text) => text.includes(newName) && /invalid|identifier|conflict|collid|rename|failed/i.test(text));
        });
        receipt("observation", "rejection-text", { text: message });
        check("rejection", {
          visible: true, kind: contract.id.slice(5), text: message,
        });
        await action("dismiss-rejection");
      }
      await widget.waitFor({ state: "hidden" });
      check("widget-hidden", { visible: await widget.isVisible() });
      await state("unchanged-workspace");
    }
    const diagnostics = await until("rename diagnostics clear", async () => {
      const result = {
        open: await bridge("diagnostics", { file: model.spec.oracle.openFile }),
        origin: await bridge("diagnostics", { file: "scripts/rename_origin.vela" }),
        closed: await bridge("diagnostics", { file: model.spec.oracle.closedFile }),
      };
      fs.writeFileSync(path.join(root, `${contract.id}-diagnostics.json`), JSON.stringify(result, null, 2));
      return Object.values(result).every((items) => items.length === 0) && result;
    });
    check("final-diagnostics", diagnostics);
    await page.screenshot({ path: path.join(root, `${contract.id}.png`) });
    fs.writeFileSync(path.join(root, `${contract.id}.aria.txt`), await page.locator('[id="workbench.parts.editor"]').ariaSnapshot());
    const finished = Date.now();
    onProof({ id: contract.id, fixture: contract.fixture, contractHash: evidence.jsonHash(contract), status: "passed",
      durationMs: finished - started, startedAt: new Date(started).toISOString(), finishedAt: new Date(finished).toISOString(), actions, checks });
    console.log(`PASS ${contract.id}`);
  }
}
module.exports = { runRename };
