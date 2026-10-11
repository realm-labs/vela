"use strict";
const fs = require("node:fs"), path = require("node:path"), assert = require("node:assert/strict");
const { outlineModel } = require("../../../../scripts/lsp-matrix/outline-contracts");
const evidence = require("../../../../scripts/lsp-matrix/local-evidence");
const { fileUri, canonicalUri } = require("./paths");
const { readTrace, providerWatermark, completedWorkerProviderRequest } = require("./readiness");

async function runOutline({ page, bridge, record, root, workspace, contracts, until, onProof }) {
  for (const contract of contracts.filter(c => c.id === "ux11-outline")) {
    const m = outlineModel(), started = Date.now(), actions = [], checks = [], observations = [];
    const outline = page.locator(".outline-tree");
    const focused = outline.locator('[role="treeitem"].focused');
    const observe = (id, value) => {
      observations.push({ id, value });
      fs.writeFileSync(path.join(root, `${contract.id}-observations.json`), JSON.stringify(observations, null, 2));
    };
    const check = (id, observed) => {
      const definition = contract.checks.find(c => c.id === id);
      assert(definition, `registered Outline assertion ${id}`);
      observe(id, observed);
      assert.deepEqual(observed, definition.expected, `${contract.id}/${id}`);
      checks.push({ ...definition, status: "passed", observed });
      record("assertion", id, { proof: contract.id, expected: definition.expected, observed });
    };
    const action = async (id, pointer) => {
      assert(Date.now() - started < contract.deadlineMs, "finite Outline route budget");
      const item = contract.actions.find(a => a.id === id);
      assert(item, `registered Outline input ${id}`);
      if (pointer) await focused.locator(".label-name").click({ clickCount: item.clickCount });
      else if (item.text !== undefined) await page.keyboard.type(item.text);
      else await page.keyboard.press(item.key);
      actions.push(item);
      record("input", id, { proof: contract.id, ...Object.fromEntries(Object.entries(item).filter(([key]) => key !== "id")) });
    };
    const focus = async (prefix, populated = true) => {
      await action(`${prefix}-palette`);
      const widget = page.locator(".quick-input-widget");
      await widget.waitFor({ state: "visible" });
      await action(`${prefix}-query`);
      const candidate = widget.locator(".label-name").filter({ hasText: /^Explorer: Focus on Outline View$/ });
      await candidate.waitFor({ state: "visible" });
      assert.equal(await candidate.count(), 1, "unambiguous Outline focus command");
      observe(`${prefix}-command`, { title: await candidate.innerText(), visible: true });
      await action(`${prefix}-invoke`);
      await widget.waitFor({ state: "hidden" });
      if (populated) await until("Outline DOM focus", () => outline.evaluate(e => e.contains(document.activeElement)));
    };
    const active = async (id, selections = false) => {
      const expected = contract.checks.find(c => c.id === id).expected;
      check(id, await until(id, async () => {
        const a = (await bridge("inspect")).active;
        if (a?.uri !== fileUri(path.join(workspace, expected.file))) return false;
        const value = selections ? { file: expected.file, selections: a.selections } : { file: expected.file, text: a.text, dirty: a.dirty };
        return JSON.stringify(value) === JSON.stringify(expected) && value;
      }));
    };
    for (const c of m.cases) {
      const afterSeq = providerWatermark(readTrace(workspace));
      await bridge("setup", { file: c.file, line: 0, character: 0, reset: true });
      await active(`${c.id}-source`);
      await focus(`${c.id}-focus`, c.nodes.length !== 0);
      const response = await until(`${c.id} symbol response`, async () => completedWorkerProviderRequest(readTrace(workspace), {
        afterSeq, method: "textDocument/documentSymbol", matchesDocument: uri => canonicalUri(uri) === fileUri(path.join(workspace, c.file)),
      }));
      observe(`${c.id}-response`, response);
      if (!c.nodes.length) {
        check(`${c.id}-empty-request`, { method: "textDocument/documentSymbol", completed: true });
        await until("Outline empty message", () => page.locator(".outline-message").isVisible());
        check(`${c.id}-empty`, { text: await page.locator(".outline-message").innerText(), rows: await outline.locator('[role="treeitem"]:visible').allTextContents() });
        await active(`${c.id}-final-source`);
        await page.screenshot({ path: path.join(root, `${contract.id}-${c.id}.png`) });
        fs.writeFileSync(path.join(root, `${contract.id}-${c.id}.aria.txt`), await page.locator("body").ariaSnapshot());
        continue;
      }
      await until("Outline populated tree", () => outline.locator('[role="treeitem"]').count());
      await action(`${c.id}-home`);
      const parents = [];
      const row = async (id, update = true) => {
        const raw = await until(id, async () => {
          if (await focused.count() !== 1) return false;
          return focused.evaluate(e => ({ name: e.querySelector(".label-name")?.textContent ?? "",
            detail: e.querySelector(".label-description")?.textContent ?? "",
            icon: Array.from(e.querySelector(".outline-element-icon")?.classList ?? []).find(c => c.startsWith("codicon-symbol-"))?.replace("codicon-", "") ?? "",
            level: Number(e.getAttribute("aria-level")), expanded: e.hasAttribute("aria-expanded") ? e.getAttribute("aria-expanded") === "true" : null,
          }));
        });
        const value = { name: raw.name, detail: raw.detail, icon: raw.icon, level: raw.level,
          parent: raw.level === 1 ? null : parents[raw.level - 2], expanded: raw.expanded };
        if (update) { parents.length = raw.level; parents[raw.level - 1] = raw.name; }
        check(id, value);
      };
      for (const [index, node] of c.nodes.entries()) {
        const id = `${c.id}-${index}`;
        if (index) await action(`${id}-next`);
        if (node.expandable) {
          await row(`${id}-initial-parent`);
          await action(`${id}-collapse`);
          await row(`${id}-collapsed-parent`);
          await action(`${id}-expand`);
        }
        await row(`${id}-row`);
        await action(`${id}-click`, true);
        await active(`${id}-selection`, true);
      }
      await action(`${c.id}-end`);
      await row(`${c.id}-tail`, false);
      await action(`${c.id}-enter`);
      check(`${c.id}-keyboard-focus`, { focused: await until("Outline Enter focuses editor", () =>
        page.evaluate(() => Boolean(document.activeElement?.closest(".monaco-editor")))) });
      await active(`${c.id}-keyboard-selection`, true);
      await focus(`${c.id}-refocus`);
      await action(`${c.id}-double-click`, true);
      await active(`${c.id}-whole-range`, true);
      await active(`${c.id}-final-source`);
      await page.screenshot({ path: path.join(root, `${contract.id}-${c.id}.png`) });
      fs.writeFileSync(path.join(root, `${contract.id}-${c.id}.aria.txt`), await page.locator("body").ariaSnapshot());
    }
    check("disk-inputs", Object.fromEntries(Object.keys(m.files).map(f => [f, fs.readFileSync(path.join(workspace, f), "utf8")])));
    await page.screenshot({ path: path.join(root, `${contract.id}-final.png`) });
    fs.writeFileSync(path.join(root, `${contract.id}-final.aria.txt`), await page.locator("body").ariaSnapshot());
    const finished = Date.now();
    onProof({ id: contract.id, fixture: contract.fixture, contractHash: evidence.jsonHash(contract), status: "passed",
      durationMs: finished - started, startedAt: new Date(started).toISOString(), finishedAt: new Date(finished).toISOString(), actions, checks });
    console.log(`PASS ${contract.id}`);
  }
}
module.exports = { runOutline };
