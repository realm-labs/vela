"use strict";
const fs = require("node:fs"), path = require("node:path"), assert = require("node:assert/strict");
const { fileURLToPath } = require("node:url");
const { hoverSignatureModel } = require("../../../../scripts/lsp-matrix/hover-signature-contracts");
const evidence = require("../../../../scripts/lsp-matrix/local-evidence");
const { fileUri } = require("./paths");
const { tokenGeometry } = require("./token-geometry");

async function runHoverSignature({ page, bridge, record, root, workspace, contracts, until, onProof }) {
  const m = hoverSignatureModel();
  const textbox = page.getByRole("textbox", { name: /^hover_input\.vela/ });
  const editor = page.locator(".monaco-editor").filter({ has: textbox });
  const hover = page.locator(".monaco-hover:visible");
  const hints = page.locator(".parameter-hints-widget:visible");
  for (const contract of contracts.filter(c => c.id.startsWith("ux10-"))) {
    const started = Date.now(), actions = [], checks = [], observations = [];
    const check = (id, observed) => {
      const expected = contract.checks.find(c => c.id === id);
      assert(expected, `registered assertion ${id}`);
      assert.deepEqual(observed, expected.expected, `${contract.id}/${id}`);
      checks.push({ ...expected, status: "passed", observed });
      record("assertion", id, { proof: contract.id, expected: expected.expected, observed });
    };
    const action = async (id, callback) => {
      const item = contract.actions.find(a => a.id === id);
      assert(item, `registered input ${id}`);
      if (callback) await callback(item);
      else if (item.text !== undefined) await page.keyboard.type(item.text);
      else for (let i = 0; i < (item.count ?? 1); i++) await page.keyboard.press(item.key);
      actions.push(item);
      record("input", id, { proof: contract.id, ...Object.fromEntries(Object.entries(item).filter(([key]) => key !== "id")) });
    };
    const observe = (id, value) => {
      observations.push({ id, value });
      fs.writeFileSync(path.join(root, `${contract.id}-observations.json`), JSON.stringify(observations, null, 2));
    };
    const state = async id => {
      const expected = contract.checks.find(c => c.id === id).expected;
      check(id, await until(`${contract.id}/${id}`, async () => {
        const active = (await bridge("inspect")).active;
        if (active?.uri !== fileUri(path.join(workspace, m.file))) return false;
        const observed = { file: m.file, text: active.text, dirty: active.dirty,
          disk: fs.readFileSync(path.join(workspace, m.file), "utf8"), selections: active.selections };
        observe(id, observed);
        return JSON.stringify(observed) === JSON.stringify(expected) && observed;
      }));
    };
    const focus = async id => check(id, { focused: await textbox.evaluate(e => e === document.activeElement) });
    const visibleHover = async id => {
      const expected = contract.checks.find(c => c.id === id).expected;
      check(id, await until(`${contract.id}/${id}`, async () => {
        if (await hover.count() !== 1) return false;
        const observed = { visible: await hover.isVisible(),
          label: (await hover.locator(".monaco-tokenized-source").allTextContents()).join("").trim(),
          paragraphs: (await hover.locator(".markdown-hover p").allTextContents()).map(s => s.replaceAll("\u00a0", " ").trim()) };
        observe(id, observed);
        return JSON.stringify(observed) === JSON.stringify(expected) && observed;
      }));
    };
    const signature = async id => {
      const expected = contract.checks.find(c => c.id === id).expected;
      check(id, await until(`${contract.id}/${id}`, async () => {
        if (await hints.count() !== 1) return false;
        const observed = { visible: await hints.isVisible(),
          label: (await hints.locator(".signature").innerText()).replaceAll("\u00a0", " ").trim(),
          active: (await hints.locator(".parameter.active").innerText()).replaceAll("\u00a0", " ").trim() };
        observe(id, observed);
        return JSON.stringify(observed) === JSON.stringify(expected) && observed;
      }));
    };
    const hidden = async (id, widget = hover) => {
      await until(`${contract.id}/${id}`, async () => await widget.count() === 0);
      check(id, { visible: await widget.count() !== 0 });
    };
    const snapshot = async suffix => {
      await page.screenshot({ path: path.join(root, `${contract.id}-${suffix}.png`) });
      if (suffix === "open") fs.writeFileSync(path.join(root, `${contract.id}-open.aria.txt`), await page.locator("body").ariaSnapshot());
    };
    const show = async () => { await action("show-chord"); await action("show-invoke"); };
    const pointTarget = async () => {
      const geometry = await tokenGeometry(editor, m.disk), glyph = geometry.glyphs.target;
      assert(glyph.visible && glyph.aligned && glyph.text === "combine", "actual authored target glyph");
      observe("target-geometry", geometry);
      await action("point-target", async () => page.mouse.move(glyph.rect.x + glyph.rect.width / 2, glyph.rect.y + glyph.rect.height / 2));
      return geometry;
    };
    const request = async (id, method, since) => {
      await until(`${contract.id}/${method} response`, () => {
        const file = path.join(workspace, ".vela-lsp-trace.jsonl");
        if (!fs.existsSync(file)) return false;
        const rows = fs.readFileSync(file,"utf8").split(/\r?\n/).filter(Boolean).flatMap(line => { try { return [JSON.parse(line)]; } catch { return []; } });
        const sameFile = uri => {
          if (!uri) return false;
          const actual = path.resolve(fileURLToPath(uri)), expected = path.resolve(workspace,m.file);
          return process.platform === "win32" ? actual.toLowerCase() === expected.toLowerCase() : actual === expected;
        };
        const response = rows.find(row => row.event === "response_sent" && row.method === method && row.timestampMs >= since &&
          row.status === "completed" && sameFile(row.documentUri));
        if (response) observe(id, response);
        return response;
      });
      check(id, { method, completed: true });
    };
    const marker = contract.id === "ux10-signature-arguments" ? "cursor" : contract.id === "ux10-unknown-receiver" ? "unknown" : "target";
    const cursor = m.disk.markers[marker].start;
    await bridge("setup", { file: m.file, line: cursor.line, character: cursor.character, reset: true });
    await textbox.focus();
    await state("origin"); await focus("editor-focus");
    if (contract.id === "ux10-signature-arguments") {
      for (const [actionId, stage] of [["type-open","open"],["type-right","right"],["type-left","both"]]) {
        await action(actionId); await state(`${stage}-source`); await signature(`${stage}-signature`);
      }
      await action("first-home"); await action("first-argument"); await action("show-first");
      await state("first-source"); await signature("first-signature");
      await action("last-end"); await action("last-argument"); await action("show-last");
      await state("last-source"); await signature("last-signature");
      await snapshot("open"); await action("dismiss"); await hidden("dismissed", hints);
    } else if (contract.id === "ux10-unknown-receiver") {
      const hoverAt = Date.now(); await show(); await request("hover-request", "textDocument/hover", hoverAt);
      await page.waitForTimeout(350); check("unknown-hover", { visible: await hover.count() !== 0 });
      await snapshot("open"); await action("enter-call"); await state("argument-source");
      const signatureAt = Date.now(); await action("show-signature"); await request("signature-request", "textDocument/signatureHelp", signatureAt);
      await page.waitForTimeout(350); check("unknown-signature", { visible: await hints.count() !== 0 });
    } else {
      let geometry;
      if (contract.id === "ux10-keyboard-hover") await show(); else geometry = await pointTarget();
      await visibleHover(contract.id === "ux10-dismiss-hover" ? "pointer-hover" : "visible-hover");
      if (contract.id === "ux10-pointer-hover") {
        const glyph = geometry.glyphs.target;
        const aligned = await until("hover range highlight", async () => {
          const rects = await editor.locator(".hoverHighlight").evaluateAll(nodes => nodes.map(n => n.getBoundingClientRect().toJSON()));
          observe("hover-highlight",rects);
          return rects.some(r => Math.abs(r.x-glyph.rect.x)<2 && Math.abs(r.width-glyph.rect.width)<2 && Math.abs(r.y-glyph.rect.y)<2);
        });
        check("target-range",{marker:"target",text:glyph.text,aligned});
      }
      await snapshot("open");
      if (contract.id === "ux10-dismiss-hover") {
        await action("leave-target", async () => page.mouse.move(geometry.viewport.right-8,geometry.viewport.bottom-8));
        await hidden("left-target"); await show(); await visibleHover("keyboard-hover");
      }
      await action("dismiss"); await hidden("dismissed");
    }
    await state("final-source"); await focus("final-focus");
    check("disk-inputs", Object.fromEntries(Object.keys(m.spec.files).map(file => [file,fs.readFileSync(path.join(workspace,file),"utf8")])));
    await snapshot("final");
    const finished = Date.now();
    onProof({ id:contract.id, fixture:contract.fixture, contractHash:evidence.jsonHash(contract), status:"passed",
      durationMs:finished-started, startedAt:new Date(started).toISOString(), finishedAt:new Date(finished).toISOString(), actions, checks });
    console.log(`PASS ${contract.id}`);
  }
}
module.exports = { runHoverSignature };
