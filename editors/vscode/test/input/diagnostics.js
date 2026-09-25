"use strict";

const fs = require("node:fs");
const path = require("node:path");
const assert = require("node:assert/strict");
const { fileURLToPath } = require("node:url");
const { diagnosticModel } = require("../../../../scripts/lsp-matrix/diagnostic-contracts");
const evidence = require("../../../../scripts/lsp-matrix/local-evidence");
const { relativeFile, fileUri } = require("./paths");

async function runDiagnostics({ page, bridge, record, root, workspace, contracts, until, onProof }) {
  const model = diagnosticModel();
  const textbox = page.getByRole("textbox", { name: /^diagnostics\.vela/ });
  const editor = page.locator(".monaco-editor").filter({ has: textbox });
  const panel = page.locator(".markers-panel");
  const rows = panel.locator(".monaco-list-row");
  const observe = async () => {
    const active = (await bridge("inspect")).active;
    if (!active) return null;
    const file = relativeFile(workspace, fileURLToPath(active.uri));
    assert.equal(active.uri, fileUri(path.join(workspace, file)));
    return {
      file, text: active.text, dirty: active.dirty,
      disk: fs.readFileSync(path.join(workspace, file), "utf8"),
      selections: active.selections,
    };
  };
  const glyph = async (marker, source) => editor.evaluate((element, { range, source }) => {
    const line = [...element.querySelectorAll(".view-lines > .view-line")][range.start.line];
    if (!line) throw Error("marked source line is not rendered");
    const textNodes = [];
    const walker = document.createTreeWalker(line, NodeFilter.SHOW_TEXT);
    let node;
    while ((node = walker.nextNode())) textNodes.push(node);
    const rendered = textNodes.map((item) => item.textContent).join("").replaceAll("\u00a0", " ");
    if (rendered !== source.split("\n")[range.start.line])
      throw Error(`rendered source differs from fixture: ${JSON.stringify(rendered)}`);
    let offset = 0;
    const selection = document.createRange();
    let start = false, end = false;
    for (const item of textNodes) {
      const next = offset + item.textContent.length;
      if (!start && range.start.character >= offset && range.start.character < next) {
        selection.setStart(item, range.start.character - offset); start = true;
      }
      if (start && !end && range.end.character > offset && range.end.character <= next) {
        selection.setEnd(item, range.end.character - offset); end = true; break;
      }
      offset = next;
    }
    if (!start || !end) throw Error("marked source range is not rendered");
    return { text: selection.toString(), rect: selection.getBoundingClientRect().toJSON() };
  }, { range: marker, source });
  const decorations = async (range, source) => {
    const target = await glyph(range, source);
    const overlays = await editor.locator(".view-overlays .squiggly-error").evaluateAll((items) => items.map((item) => ({
      className: item.className, rect: item.getBoundingClientRect().toJSON(),
    })));
    const aligned = overlays.some(({ rect }) =>
      Math.abs(rect.x - target.rect.x) < 3 && Math.abs(rect.right - target.rect.right) < 3 &&
      rect.y <= target.rect.y + target.rect.height && rect.bottom >= target.rect.y);
    return { target, overlays, aligned };
  };
  const panelText = () => rows.allTextContents();
  const panelItem = async (diagnostic) => {
    const row = rows.filter({ hasText: diagnostic.message });
    assert.equal(await row.count(), 1, `one rendered row for ${diagnostic.code}`);
    const text = await row.innerText();
    const label = await row.getAttribute("aria-label");
    return {
      code: text.includes(diagnostic.code) ? diagnostic.code : null,
      message: text.includes(diagnostic.message) ? diagnostic.message : null,
      severity: label?.startsWith("Error:") ? "error" : null,
      location: text.match(/\[Ln \d+, Col \d+\]/)?.[0].slice(1, -1) ?? null,
    };
  };
  for (const contract of contracts.filter((item) => item.id.startsWith("ux07-"))) {
    const started = Date.now(), actions = [], checks = [];
    const receipt = (kind, id, details) => record(kind, id, { proof: contract.id, ...details });
    const check = (id, observed) => {
      const expected = contract.checks.find((item) => item.id === id);
      assert.ok(expected, `unregistered assertion ${id}`);
      assert.deepEqual(observed, expected.expected, `${contract.id}/${id}`);
      checks.push({ ...expected, status: "passed", observed });
      receipt("assertion", id, { expected: expected.expected, observed });
    };
    const state = async (id, fields) => {
      const expected = contract.checks.find((item) => item.id === id).expected;
      const actual = await until(id, async () => {
        const observed = await observe();
        return observed && Object.entries(expected).every(([name, value]) =>
          JSON.stringify(observed[name]) === JSON.stringify(value)) && observed;
      });
      check(id, Object.fromEntries(fields.map((name) => [name, actual[name]])));
    };
    const action = async (id) => {
      const item = contract.actions.find((candidate) => candidate.id === id);
      assert.ok(item, `unregistered input ${id}`);
      if (item.text !== undefined) await page.keyboard.type(item.text);
      else if (item.key) await page.keyboard.press(item.key);
      else if (item.id === "select-problem") {
        const row = rows.filter({ hasText: item.target });
        assert.equal(await row.count(), 1, "one target problem row");
        await row.dblclick();
      } else if (item.id === "select-valid") {
        const target = await glyph(model.valid, model.typed.text);
        assert.equal(target.text, "first");
        await page.mouse.click(target.rect.x + Math.min(3, target.rect.width / 8),
          target.rect.y + target.rect.height / 2, { clickCount: item.clickCount });
      } else throw Error(`unsupported input ${id}`);
      actions.push(item);
      receipt("input", id, Object.fromEntries(Object.entries(item).filter(([key]) => key !== "id")));
    };
    await bridge("setup", { file: model.file, ...model.cursor, reset: true });
    await textbox.focus();
    await state("origin", Object.keys(contract.checks.find((item) => item.id === "origin").expected));
    await action("type-error");
    await state("typed-source", Object.keys(contract.checks.find((item) => item.id === "typed-source").expected));
    const expectedDiagnostics = contract.checks.find((item) => item.id === "typed-diagnostics").expected;
    check("typed-diagnostics", await until("typed diagnostics", async () => {
      const values = await bridge("diagnostics", { file: model.file });
      return JSON.stringify(values) === JSON.stringify(expectedDiagnostics) && values;
    }));
    await action("open-problems");
    await panel.waitFor({ state: "visible" });
    const rendered = await until("Problems entries", async () => {
      const values = await panelText();
      fs.writeFileSync(path.join(root, `${contract.id}-observations.json`), JSON.stringify({ values }, null, 2));
      return values.some((value) => value.includes(model.target.message)) &&
        values.some((value) => value.includes(model.unrelated.message)) && values;
    });
    check("problems-panel", {
      visible: await panel.isVisible(),
      target: await panelItem(model.target),
      unrelated: await panelItem(model.unrelated),
    });
    receipt("observation", "visible-problems", { rows: rendered });
    if (contract.id !== "ux07-valid-location") {
      await action("select-problem");
      await state("selected-problem", Object.keys(contract.checks.find((item) => item.id === "selected-problem").expected));
      const marked = await decorations(model.target.range, model.typed.text);
      fs.writeFileSync(path.join(root, `${contract.id}-observations.json`), JSON.stringify({ rows: rendered, marked }, null, 2));
      check("target-decoration", { marker: "target", text: marked.target.text,
        aligned: marked.aligned, visible: marked.aligned && marked.target.rect.width > 0 });
      if (contract.id === "ux07-repair-unsaved") {
        await action("focus-editor");
        check("repair-focus", { focused: await textbox.evaluate((element) => element === document.activeElement) });
        await action("repair-error");
        await state("repaired-source", Object.keys(contract.checks.find((item) => item.id === "repaired-source").expected));
        const remaining = contract.checks.find((item) => item.id === "remaining-diagnostics").expected;
        check("remaining-diagnostics", await until("remaining diagnostics", async () => {
          const values = await bridge("diagnostics", { file: model.file });
          return JSON.stringify(values) === JSON.stringify(remaining) && values;
        }));
        const values = await until("remaining Problems entry", async () => {
          const entries = await panelText();
          return entries.some((item) => item.includes(model.unrelated.message)) &&
            !entries.some((item) => item.includes(model.target.message)) && entries;
        });
        check("remaining-panel", { target: values.some((item) => item.includes(model.target.message)),
          unrelated: values.some((item) => item.includes(model.unrelated.message)) });
        const visible = await editor.evaluate((element, lines) => {
          const rendered = [...element.querySelectorAll(".view-lines > .view-line")];
          const overlays = [...element.querySelectorAll(".view-overlays .squiggly-error")]
            .map((item) => item.getBoundingClientRect());
          const onLine = (number) => {
            const row = rendered[number]?.getBoundingClientRect();
            if (!row) throw Error(`source line ${number} is not rendered`);
            return overlays.some((rect) => rect.y < row.bottom && rect.bottom > row.y);
          };
          return { target: onLine(lines.target), unrelated: onLine(lines.unrelated) };
        }, { target: model.target.range.start.line, unrelated: model.unrelated.range.start.line });
        check("cleared-target-decoration", { marker: "target", visible: visible.target,
          unrelatedVisible: visible.unrelated });
      }
    } else {
      await action("select-valid");
      const active = (await bridge("inspect")).active;
      check("valid-caret", { file: model.file, position: active.selections[0].active });
      await state("valid-source", Object.keys(contract.checks.find((item) => item.id === "valid-source").expected));
      const marked = await decorations(model.valid, model.typed.text);
      check("valid-decoration", { marker: "valid", visible: marked.aligned });
      check("unchanged-diagnostics", await bridge("diagnostics", { file: model.file }));
    }
    await page.screenshot({ path: path.join(root, `${contract.id}.png`) });
    fs.writeFileSync(path.join(root, `${contract.id}.aria.txt`), await panel.ariaSnapshot());
    const finished = Date.now();
    onProof({ id: contract.id, fixture: contract.fixture, contractHash: evidence.jsonHash(contract), status: "passed",
      durationMs: finished - started, startedAt: new Date(started).toISOString(), finishedAt: new Date(finished).toISOString(),
      actions, checks });
    console.log(`PASS ${contract.id}`);
  }
}

module.exports = { runDiagnostics };
