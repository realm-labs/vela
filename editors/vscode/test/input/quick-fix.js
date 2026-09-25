"use strict";

const fs = require("node:fs");
const path = require("node:path");
const assert = require("node:assert/strict");
const { fileURLToPath } = require("node:url");
const { quickFixModel } = require("../../../../scripts/lsp-matrix/quick-fix-contracts");
const evidence = require("../../../../scripts/lsp-matrix/local-evidence");
const { relativeFile, fileUri } = require("./paths");

async function runQuickFix({ page, bridge, record, root, workspace, contracts, until, onProof }) {
  const model = quickFixModel();
  const editor = page.getByRole("textbox", { name: /^main\.vela/ });
  for (const contract of contracts.filter((item) => item.id.startsWith("ux08-"))) {
    const started = Date.now(), actions = [], checks = [];
    const receipt = (kind, id, details) => record(kind, id, { proof: contract.id, ...details });
    const check = (id, observed) => {
      const expected = contract.checks.find((item) => item.id === id);
      assert.ok(expected, `unregistered assertion ${id}`);
      assert.deepEqual(observed, expected.expected, `${contract.id}/${id}`);
      checks.push({ ...expected, status: "passed", observed });
      receipt("assertion", id, { expected: expected.expected, observed });
    };
    const action = async (id, callback) => {
      const item = contract.actions.find((candidate) => candidate.id === id);
      assert.ok(item, `unregistered input ${id}`);
      if (callback) await callback(item);
      else await page.keyboard.press(item.key);
      actions.push(item);
      receipt("input", id, Object.fromEntries(Object.entries(item).filter(([key]) => key !== "id")));
    };
    const source = async (id) => {
      const expected = contract.checks.find((item) => item.id === id).expected;
      let last;
      const observed = await until(id, async () => {
        const active = (await bridge("inspect")).active;
        if (!active) return null;
        const file = relativeFile(workspace, fileURLToPath(active.uri));
        if (file !== model.file) return null;
        assert.equal(active.uri, fileUri(path.join(workspace, file)));
        const value = { file, text: active.text, dirty: active.dirty,
          disk: fs.readFileSync(path.join(workspace, file), "utf8") };
        last = value;
        return JSON.stringify(value) === JSON.stringify(expected) && value;
      }).catch((error) => { throw Error(`${error.message}: ${JSON.stringify(last)}`); });
      check(id, observed);
    };
    const diagnostics = async (id) => {
      const expected = contract.checks.find((item) => item.id === id).expected;
      const observed = await until(id, async () => {
        const values = await bridge("diagnostics", { file: model.file });
        return JSON.stringify(values) === JSON.stringify(expected) && values;
      });
      check(id, observed);
    };
    await bridge("setup", { file: model.file,
      ...(contract.id === "ux08-no-fix" ? model.noFixCursor : model.cursor), reset: true });
    await editor.focus();
    await source("origin");
    await diagnostics("initial-diagnostics");
    if (contract.id === "ux08-no-fix") {
      const active = (await bridge("inspect")).active;
      check("no-fix-cursor", { position: active.selections[0].active });
    }
    const widget = page.locator(".action-widget:not(.action-list-submenu-panel)");
    const rows = widget.locator(".monaco-list-row");
    if (contract.id === "ux08-lightbulb-fix") {
      const lightbulb = page.locator(".lightBulbWidget");
      await lightbulb.waitFor({ state: "visible" });
      check("visible-lightbulb", { visible: await lightbulb.isVisible() });
      await action("open-lightbulb", async (item) => lightbulb.click({ clickCount: item.clickCount }));
    } else await action("open-quick-fix");
    if (contract.id === "ux08-no-fix") {
      await page.waitForTimeout(500);
      check("no-fix-menu", { visible: await widget.isVisible(), unsafeActions: await rows.allTextContents() });
      await page.screenshot({ path: path.join(root, `${contract.id}-open.png`) });
      fs.writeFileSync(path.join(root, `${contract.id}-open.aria.txt`), await page.locator("body").ariaSnapshot());
      await source("unchanged-source");
      await diagnostics("unchanged-diagnostics");
    } else {
      await widget.waitFor({ state: "visible" });
      const titles = await until("quick-fix menu entries", async () => {
        const observed = await rows.allTextContents();
        return observed.length >= model.spec.oracle.actionTitles.length && observed;
      });
      fs.writeFileSync(path.join(root, `${contract.id}-menu.json`), JSON.stringify({ titles }, null, 2));
      check("visible-fixes", { visible: await widget.isVisible(), rows: titles.map((item) => item.trim()) });
      const candidate = rows.filter({ hasText: model.spec.oracle.action.title });
      assert.equal(await candidate.count(), 1);
      check("selected-fix", { label: await candidate.innerText(), focused: await candidate.evaluate((element) =>
        element.classList.contains("focused")) });
      await page.screenshot({ path: path.join(root, `${contract.id}-open.png`) });
      fs.writeFileSync(path.join(root, `${contract.id}-open.aria.txt`), await widget.ariaSnapshot());
      if (contract.id === "ux08-dismiss") {
        await action("dismiss-fixes");
        await widget.waitFor({ state: "hidden" });
        check("dismissed-menu", { visible: await widget.isVisible() });
        await source("unchanged-source");
        await diagnostics("unchanged-diagnostics");
      } else {
        await action("choose-fix");
        await source("applied-source");
        await diagnostics("after-fix-diagnostics");
        await action("focus-editor");
        check("undo-focus", { focused: await editor.evaluate((element) => element === document.activeElement) });
        await action("undo-fix");
        await source("undo-source");
        await diagnostics("undo-diagnostics");
      }
    }
    await page.screenshot({ path: path.join(root, `${contract.id}-final.png`) });
    const finished = Date.now();
    onProof({ id: contract.id, fixture: contract.fixture, contractHash: evidence.jsonHash(contract), status: "passed",
      durationMs: finished - started, startedAt: new Date(started).toISOString(), finishedAt: new Date(finished).toISOString(),
      actions, checks });
    console.log(`PASS ${contract.id}`);
  }
}

module.exports = { runQuickFix };
