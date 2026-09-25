"use strict";
const fs = require("node:fs");
const path = require("node:path");
const assert = require("node:assert/strict");
const { referencesModel } = require("../../../../scripts/lsp-matrix/references-contracts");
const evidence = require("../../../../scripts/lsp-matrix/local-evidence");
const { fileUri } = require("./paths");

async function runHighlights({ page, bridge, record, root, workspace, contracts, until, onProof }) {
  const model = referencesModel();
  const file = model.spec.oracle.openFile, document = model.documents[file];
  const markerNames = [...model.spec.oracle.localSites, ...model.spec.oracle.shadowSites];
  const markers = Object.fromEntries(markerNames.map((name) => [name, document.markers[name]]));
  const sourceLines = document.text.split("\n");
  const textbox = page.getByRole("textbox", { name: /^refs_open\.vela/ });
  const editor = page.locator(".monaco-editor").filter({ has: textbox });
  const geometry = async () => editor.evaluate((element, { markers, sourceLines }) => {
    const lines = [...element.querySelectorAll(".view-lines > .view-line")];
    function rangeFor(name) {
      const marker = markers[name], line = lines[marker.start.line];
      if (!line) throw Error(`source line is not visibly rendered for ${name}`);
      const walker = document.createTreeWalker(line, NodeFilter.SHOW_TEXT);
      const range = document.createRange();
      const nodes = []; let node;
      while ((node = walker.nextNode())) {
        if (!node.parentElement?.closest('[class*="dyn-rule-"]')) nodes.push(node);
      }
      const source = nodes.map((item) => item.textContent).join("").replaceAll("\u00a0", " ");
      if (source !== sourceLines[marker.start.line])
        throw Error(`rendered source differs from fixture for ${name}: ${JSON.stringify(source)}`);
      let offset = 0, started = false, ended = false;
      for (const node of nodes) {
        const next = offset + node.textContent.length;
        if (!started && marker.start.character >= offset && marker.start.character < next) {
          range.setStart(node, marker.start.character - offset); started = true;
        }
        if (started && !ended && marker.end.character > offset && marker.end.character <= next) {
          range.setEnd(node, marker.end.character - offset); ended = true; break;
        }
        offset = next;
      }
      if (!started || !ended) throw Error(`rendered marker is incomplete: ${name}`);
      return { rect: range.getBoundingClientRect().toJSON(), text: range.toString() };
    }
    const glyphs = Object.fromEntries(Object.keys(markers).map((name) => [name, rangeFor(name)]));
    const overlays = [...element.querySelectorAll(".view-overlays .cdr")].filter((item) =>
      [...item.classList].some((name) => /^wordHighlight(?:Strong|Text)?$/.test(name)));
    const decorations = overlays.map((overlay) => {
      const rect = overlay.getBoundingClientRect();
      const matches = Object.entries(glyphs).filter(([, glyph]) => {
        const box = glyph.rect;
        return Math.abs(rect.x - box.x) <= 2 && Math.abs(rect.right - (box.x + box.width)) <= 2 &&
          rect.y <= box.y + box.height && rect.bottom >= box.y;
      });
      const style = [...overlay.classList].find((name) => /^wordHighlight(?:Strong|Text)?$/.test(name));
      return { marker: matches.length === 1 ? matches[0][0] : null,
        text: matches.length === 1 ? matches[0][1].text : null, style,
        aligned: matches.length === 1, visible: rect.width > 0 && rect.height > 0 };
    });
    decorations.sort((a, b) => Object.keys(markers).indexOf(a.marker) - Object.keys(markers).indexOf(b.marker));
    return { glyphs, decorations };
  }, { markers, sourceLines });

  for (const contract of contracts.filter((item) => ["ux06-highlights-caret", "ux06-shadow-exclusion"].includes(item.id))) {
    const started = Date.now(), actions = [], checks = [];
    const receipt = (kind, id, details) => record(kind, id, { proof: contract.id, ...details });
    const check = (id, observed) => {
      const required = contract.checks.find((item) => item.id === id);
      assert.ok(required, `unregistered assertion ${id}`);
      assert.deepEqual(observed, required.expected, `${contract.id}/${id}`);
      checks.push({ ...required, status: "passed", observed });
      receipt("assertion", id, { expected: required.expected, observed });
    };
    await bridge("setup", { file, line: 1, character: 0 });
    await textbox.focus();
    const origin = (await bridge("inspect")).active;
    assert.equal(origin.uri, fileUri(path.join(workspace, file)));
    check("origin", { file, text: origin.text, dirty: origin.dirty, selections: origin.selections });
    for (const action of contract.actions) {
      const glyph = (await geometry()).glyphs[action.target];
      assert.equal(glyph.text, "score", `visible ${action.target} must match the fixture`);
      await page.mouse.click(glyph.rect.x + Math.min(2, glyph.rect.width / 8),
        glyph.rect.y + glyph.rect.height / 2, { clickCount: action.clickCount });
      actions.push(action);
      receipt("input", action.id, Object.fromEntries(Object.entries(action).filter(([key]) => key !== "id")));
      const active = (await bridge("inspect")).active;
      assert.equal(active.uri, fileUri(path.join(workspace, file)));
      check(`${action.target}-caret`, { file, position: active.selections[0].active });
      const expected = contract.checks.find((item) => item.id === `${action.target}-decorations`).expected;
      const observed = await until(`${action.target} highlights`, async () => {
        const result = await geometry();
        fs.writeFileSync(path.join(root, `${contract.id}-decorations.json`), JSON.stringify(result, null, 2));
        return JSON.stringify(result.decorations) === JSON.stringify(expected) && result.decorations;
      });
      check(`${action.target}-decorations`, observed);
      await page.screenshot({ path: path.join(root, `${contract.id}-${action.target}.png`) });
    }
    const finished = Date.now();
    onProof({ id: contract.id, fixture: contract.fixture, contractHash: evidence.jsonHash(contract), status: "passed",
      durationMs: finished - started, startedAt: new Date(started).toISOString(), finishedAt: new Date(finished).toISOString(),
      actions, checks });
    console.log(`PASS ${contract.id}`);
  }
}

module.exports = { runHighlights };
