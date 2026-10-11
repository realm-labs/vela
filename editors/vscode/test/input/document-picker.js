"use strict";
const fs = require("node:fs"), path = require("node:path"), assert = require("node:assert/strict");
const { documentPickerModel } = require("../../../../scripts/lsp-matrix/document-picker-contracts");
const evidence = require("../../../../scripts/lsp-matrix/local-evidence");
const { fileUri, canonicalUri } = require("./paths");
const { readTrace, providerWatermark, completedWorkerProviderRequest } = require("./readiness");

async function runDocumentPicker({ page, bridge, record, root, workspace, contracts, until, onProof }) {
  for (const contract of contracts.filter(c => c.id === "ux11-document-picker")) {
    const m = documentPickerModel(), started = Date.now(), actions = [], checks = [], observations = [];
    const widget = page.locator(".quick-input-widget");
    const focused = widget.locator(".quick-input-list .monaco-list-row.focused");
    const observe = (id, value) => {
      observations.push({ id, value });
      fs.writeFileSync(path.join(root, `${contract.id}-observations.json`), JSON.stringify(observations, null, 2));
    };
    const check = (id, observed) => {
      const definition = contract.checks.find(c => c.id === id);
      assert(definition, `registered picker assertion ${id}`);
      observe(id, observed);
      assert.deepEqual(observed, definition.expected, `${contract.id}/${id}`);
      checks.push({ ...definition, status: "passed", observed });
      record("assertion", id, { proof: contract.id, expected: definition.expected, observed });
    };
    const action = async id => {
      assert(Date.now()-started < contract.deadlineMs, "finite document picker route budget");
      const item = contract.actions.find(a => a.id === id);
      assert(item, `registered picker input ${id}`);
      if (item.device === "pointer") await focused.locator(".label-name").click();
      else if (item.text !== undefined) await page.keyboard.type(item.text);
      else await page.keyboard.press(item.key);
      actions.push(item);
      record("input", id, { proof: contract.id, ...Object.fromEntries(Object.entries(item).filter(([k]) => k !== "id")) });
    };
    const active = async (id, selection = false) => {
      const expected = contract.checks.find(c => c.id === id).expected;
      check(id, await until(id, async () => {
        const a = (await bridge("inspect")).active;
        if (a?.uri !== fileUri(path.join(workspace, expected.file))) return false;
        const value = { file: expected.file, text: a.text, dirty: a.dirty, ...(selection ? { selections: a.selections } : {}),
          ...(Object.hasOwn(expected, "pickerVisible") ? { pickerVisible: await widget.isVisible() } : {}) };
        return JSON.stringify(value) === JSON.stringify(expected) && value;
      }));
    };
    const row = async (id, empty = false) => {
      const locator = empty ? widget.locator(".quick-input-list .monaco-list-row") : focused;
      await until(`${id} row`, async () => await locator.count() === 1 && await locator.isVisible());
      check(id, await locator.evaluate((e, empty) => ({
        name: (e.querySelector(".label-name")?.textContent ?? "").trim(),
        description: (e.querySelector(".label-description")?.textContent ?? "").trim(),
        icon: Array.from(e.querySelector(".codicon[class*='codicon-symbol-']")?.classList ?? [])
          .find(c => c.startsWith("codicon-symbol-"))?.replace("codicon-", "") ?? "",
        ...(empty ? { disabled: e.getAttribute("aria-disabled") === "true" } : {}),
      }), empty));
    };
    const snapshot = async c => {
      await page.screenshot({ path: path.join(root, `${contract.id}-${c.id}.png`) });
      fs.writeFileSync(path.join(root, `${contract.id}-${c.id}.aria.txt`), await page.locator("body").ariaSnapshot());
    };
    for (const c of m.cases) {
      const afterSeq = providerWatermark(readTrace(workspace));
      await bridge("setup", { file: c.file, line: 0, character: 0, reset: true });
      await active(`${c.id}-source`);
      await action(`${c.id}-open`);
      await widget.waitFor({ state: "visible" });
      const response = await until(`${c.id} fresh symbols`, () => completedWorkerProviderRequest(readTrace(workspace), {
        afterSeq, method: "textDocument/documentSymbol", matchesDocument: uri => canonicalUri(uri) === fileUri(path.join(workspace,c.file)),
      }));
      observe(`${c.id}-response`, response);
      check(`${c.id}-request`, { method: "textDocument/documentSymbol", completed: true });
      check(`${c.id}-query`, { value: await widget.locator("input").inputValue() });
      if (!c.nodes.length) {
        await row(`${c.id}-empty`, true);
        await snapshot(c);
        await action(`${c.id}-empty-accept`);
        await active(`${c.id}-empty-retained`, true);
      } else {
        const separator = widget.locator(".quick-input-list .quick-input-list-separator:visible");
        await until("document symbol count separator", () => separator.count());
        check(`${c.id}-separator`, { text: (await separator.innerText()).trim() });
        await action(`${c.id}-home`);
        for (const [index] of c.nodes.entries()) {
          const id = `${c.id}-${index}`;
          if (index) await action(`${id}-next`);
          await row(`${id}-row`);
          if (!index) await snapshot(c);
          await action(`${id}-accept`);
          await widget.waitFor({ state: "hidden" });
          await active(`${id}-destination`, true);
          await action(`${id}-reopen`);
          await widget.waitFor({ state: "visible" });
          await row(`${id}-restored-row`);
        }
        await action(`${c.id}-end`);
        await row(`${c.id}-tail`);
        await action(`${c.id}-past-tail`);
        await row(`${c.id}-wrapped-head`);
        await action(`${c.id}-select-query`);
        await action(`${c.id}-no-match-query`);
        await until("document no-match placeholder", () => widget.locator(".label-name").filter({ hasText: m.spec.oracle.noMatchText }).count());
        await row(`${c.id}-no-match`, true);
        await snapshot({ id: `${c.id}-no-match` });
        await action(`${c.id}-no-match-accept`);
        await active(`${c.id}-no-match-retained`, true);
      }
      await action(`${c.id}-dismiss`);
      await widget.waitFor({ state: "hidden" });
      await active(`${c.id}-final-source`);
    }
    check("disk-inputs", Object.fromEntries(Object.keys(m.files).map(f => [f,fs.readFileSync(path.join(workspace,f),"utf8")])));
    await page.screenshot({ path: path.join(root,`${contract.id}-final.png`) });
    fs.writeFileSync(path.join(root,`${contract.id}-final.aria.txt`), await page.locator("body").ariaSnapshot());
    const finished = Date.now();
    onProof({ id: contract.id, fixture: contract.fixture, contractHash: evidence.jsonHash(contract), status: "passed",
      durationMs: finished-started, startedAt: new Date(started).toISOString(), finishedAt: new Date(finished).toISOString(), actions, checks });
    console.log(`PASS ${contract.id}`);
  }
}
module.exports = { runDocumentPicker };
