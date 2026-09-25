"use strict";
const fs = require("node:fs");
const path = require("node:path");
const assert = require("node:assert/strict");
const { fileURLToPath } = require("node:url");
const { referencesModel } = require("../../../../scripts/lsp-matrix/references-contracts");
const evidence = require("../../../../scripts/lsp-matrix/local-evidence");
const { relativeFile, fileUri } = require("./paths");

async function runReferences({ page, bridge, record, root, workspace, contracts, until, onProof }) {
  const contract = contracts.find((item) => item.id === "ux06-references-select");
  if (!contract) return;
  const model = referencesModel(), started = Date.now(), actions = [], checks = [];
  const receipt = (kind, id, details) => record(kind, id, { proof: contract.id, ...details });
  const check = (id, observed) => {
    const required = contract.checks.find((item) => item.id === id);
    assert.ok(required, `unregistered assertion ${id}`);
    assert.deepEqual(observed, required.expected, `${contract.id}/${id}`);
    checks.push({ ...required, status: "passed", observed });
    receipt("assertion", id, { expected: required.expected, observed });
  };
  const action = async (id, target) => {
    const item = contract.actions.find((entry) => entry.id === id);
    assert.ok(item, `unregistered input ${id}`);
    if (item.device === "keyboard") await page.keyboard.press(item.key);
    else await target.click({ clickCount: item.clickCount });
    actions.push(item);
    receipt("input", id, Object.fromEntries(Object.entries(item).filter(([key]) => key !== "id")));
  };
  const observe = async () => {
    const active = (await bridge("inspect")).active;
    if (!active) return null;
    const file = relativeFile(workspace, fileURLToPath(active.uri));
    assert.equal(active.uri, fileUri(path.join(workspace, file)));
    return { file, text: active.text, dirty: active.dirty, selections: active.selections };
  };
  const panel = page.locator(".peekview-widget");
  for (const [index, site] of model.sites.entries()) {
    await bridge("setup", { file: model.spec.oracle.openFile, ...model.cursor });
    const editor = page.getByRole("textbox", { name: /^refs_open\.vela/ });
    await editor.focus();
    if (index === 0) {
      check("origin", await observe());
      const documents = (await bridge("inspect")).documents;
      check("unopened-targets", {
        origin: !documents.some((item) => item.uri === fileUri(path.join(workspace, "scripts/refs_origin.vela"))),
        closed: !documents.some((item) => item.uri === fileUri(path.join(workspace, "scripts/refs_closed.vela"))),
      });
    }
    await action(`open-references-${index}`);
    await panel.waitFor({ state: "visible" });
    const tree = panel.getByRole("tree", { name: "References" });
    const groups = index === 0 ? [...new Set(model.sites.map((item) => item.file))] : [site.file];
    for (const file of groups) {
      const count = model.sites.filter((item) => item.file === file).length;
      const name = `${count} symbol${count === 1 ? "" : "s"} in ${path.basename(file)}`;
      const group = tree.getByRole("treeitem", { name: new RegExp(`^${name.replaceAll(".", "\\.")},`) });
      await action(`select-group-${index}-${file}`, group);
      await action(`expand-group-${index}-${file}`);
    }
    if (index === 0) {
      const aria = await panel.ariaSnapshot();
      fs.writeFileSync(path.join(root, `${contract.id}.aria.txt`), aria);
      await page.screenshot({ path: path.join(root, `${contract.id}.png`) });
      const count = Number(aria.match(/References \((\d+)\)/)?.[1]);
      check("panel", { visible: await panel.isVisible(), count });
      const rows = await tree.getByRole("treeitem").evaluateAll((items) => items.map((element) => ({
        level: Number(element.getAttribute("aria-level")), label: element.getAttribute("aria-label"),
        highlight: element.querySelector(".referenceMatch .highlight")?.textContent ?? null,
      })));
      fs.writeFileSync(path.join(root, `${contract.id}-rows.json`), JSON.stringify(rows, null, 2));
      const fileCounts = Object.fromEntries(rows.filter((row) => row.level === 1).map((row) => {
        const match = row.label.match(/^(\d+) symbols? in (refs_(?:closed|open|origin)\.vela),/);
        assert.ok(match, `unexpected reference file row: ${row.label}`);
        return [`scripts/${match[2]}`, Number(match[1])];
      }));
      check("file-counts", fileCounts);
      const references = rows.filter((row) => row.level === 2).map((row) => {
        const match = row.label.match(/ in (refs_(?:closed|open|origin)\.vela) on line (\d+) at column (\d+)$/);
        assert.ok(match, `unexpected reference row: ${row.label}`);
        const start = { line: Number(match[2]) - 1, character: Number(match[3]) - 1 };
        return { file: `scripts/${match[1]}`, range: { start,
          end: { line: start.line, character: start.character + row.highlight.length } }, text: row.highlight };
      });
      check("reference-set", references);
    }
    const leaf = tree.getByRole("treeitem", { name: new RegExp(
      ` in ${path.basename(site.file).replaceAll(".", "\\.")} on line ${site.range.start.line + 1} at column ${site.range.start.character + 1}$`,
    ) });
    assert.equal(await leaf.count(), 1, "each reference location must have one visible row");
    await action(`select-reference-${index}`, leaf);
    await panel.waitFor({ state: "hidden" });
    const expected = contract.checks.find((item) => item.id === `destination-${index}`).expected;
    const destination = await until(`reference destination ${index}`, async () => {
      const state = await observe();
      return JSON.stringify(state) === JSON.stringify(expected) && state;
    });
    check(`destination-${index}`, destination);
  }
  const finished = Date.now();
  onProof({ id: contract.id, fixture: contract.fixture, contractHash: evidence.jsonHash(contract), status: "passed",
    durationMs: finished - started, startedAt: new Date(started).toISOString(), finishedAt: new Date(finished).toISOString(),
    actions, checks });
  console.log(`PASS ${contract.id}`);
}

module.exports = { runReferences };
