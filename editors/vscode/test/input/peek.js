"use strict";
const fs = require("node:fs");
const path = require("node:path");
const assert = require("node:assert/strict");
const { fileURLToPath, pathToFileURL } = require("node:url");
const { navigationModel } = require("../../../../scripts/lsp-matrix/navigation-contracts");
const { navigationResponses } = require("../../../../scripts/lsp-matrix/navigation-trace");
const { offsetAt } = require("../../../../scripts/lsp-matrix/fixtures");
const evidence = require("../../../../scripts/lsp-matrix/local-evidence");
const { findLog } = require("./logs");

async function runPeek({ page, bridge, record, root, workspace, contracts, until, onProof, pid }) {
  const model = navigationModel();
  const log = findLog(root, (name) => name.endsWith("-Vela LSP Trace.log"));
  const logText = () => fs.readFileSync(log, "utf8");
  const peek = page.locator(".peekview-widget");
  const input = page.getByRole("textbox", { name: /^nav\.vela/ });
  const editor = page.locator(".monaco-editor").filter({ has: input });
  const observe = async () => {
    const active = (await bridge("inspect")).active;
    if (!active) return null;
    const file = path.relative(workspace, fileURLToPath(active.uri));
    assert.equal(active.uri, pathToFileURL(path.join(workspace, file)).href);
    return { file, text: active.text, dirty: active.dirty, selections: active.selections };
  };
  const line = async (scope, text) => {
    fs.writeFileSync(path.join(root, "rendered-lines.json"), JSON.stringify(await scope.locator(".view-lines > .view-line").evaluateAll((items) => items.map((item) => ({ text: item.textContent, html: item.innerHTML }))), null, 2));
    const locator = scope.locator(".view-lines > .view-line").filter({ hasText: text.trim() });
    await locator.waitFor({ state: "visible" });
    assert.equal(await locator.count(), 1, "one rendered source line must identify the pointer target");
    const observed = (await locator.textContent()).replaceAll("\u00a0", " ");
    assert.equal(observed, text, "rendered source must match the independent fixture");
    return locator;
  };
  for (const contract of contracts.filter((item) => item.id.startsWith("ux03-"))) {
    const started = Date.now(), actions = [], checks = [];
    const unknown = contract.id === "ux03-unknown-target";
    const origin = model.origin(unknown ? "unknown" : "call");
    const point = origin.selections[0].active;
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
        const value = await observe();
        fs.writeFileSync(path.join(root, "peek-state.json"), JSON.stringify({ proof: contract.id, id, expected, observed: value }, null, 2));
        return JSON.stringify(value) === JSON.stringify(expected) && value;
      }));
    };
    if (contract.id !== "ux03-modifier-click")
      require("node:child_process").execFileSync(path.join(root, "native-menu"), [String(pid), "activate", "setup"], { timeout: 5000 });
    await bridge("setup", { file: origin.file, ...point });
    await input.focus();
    await state("origin");
    check("editor-focus", { focused: await input.evaluate((element) => element === document.activeElement) });
    const clear = contract.actions.find((item) => item.id === "clear-prior-message");
    await page.keyboard.press(clear.key);
    actions.push(clear);
    receipt("input", clear.id, { device: clear.device, key: clear.key });
    await page.locator(".monaco-editor-overlaymessage:visible").waitFor({ state: "hidden" });
    const renderedSource = contract.checks.find((item) => item.id === "source-line").expected.text;
    const sourceLine = await line(editor, renderedSource);
    check("source-line", { text: (await sourceLine.textContent()).replaceAll("\u00a0", " "), visible: await sourceLine.isVisible() });
    // Read geometry from the rendered first character. Never set a DOM selection,
    // dispatch synthetic editor events, or call a navigation command for this route.
    const pointer = await sourceLine.evaluate((element, column) => {
      const walker = document.createTreeWalker(element, NodeFilter.SHOW_TEXT);
      let node, offset = column;
      while ((node = walker.nextNode())) {
        if (offset < node.textContent.length) {
          const range = document.createRange();
          range.setStart(node, offset); range.setEnd(node, offset + 1);
          const rect = range.getBoundingClientRect();
          return { x: rect.x + rect.width / 4, y: rect.y + rect.height / 2 };
        }
        offset -= node.textContent.length;
      }
      throw Error("source column was not rendered");
    }, renderedSource.indexOf(unknown ? "missing" : "make"));
    receipt("observation", "pointer-geometry", { pointer, hit: await page.evaluate(({ x, y }) => {
      const element = document.elementFromPoint(x, y);
      return { html: element?.outerHTML, line: element?.closest(".view-line")?.textContent };
    }, pointer) });
    let boundary = logText().length;
    const action = async (id) => {
      const item = contract.actions.find((item) => item.id === id);
      assert.ok(item, `unregistered input ${id}`);
      if (item.device === "keyboard") {
        if (item.event === "down") await page.keyboard.down(item.key);
        else if (item.event === "up") await page.keyboard.up(item.key);
        else await page.keyboard.press(item.key);
      } else if (item.target === "source-identifier") {
        if (item.event === "hover") await page.mouse.move(pointer.x, pointer.y);
        else if (item.button === "right") {
          const geometry = { ...pointer, ...await page.evaluate(() => ({ width: innerWidth, height: innerHeight })) };
          const nativePointer = JSON.parse(require("node:child_process").execFileSync(path.join(root, "native-menu"),
            [String(pid), "context-click", JSON.stringify(geometry)], { encoding: "utf8", timeout: 5000 }));
          receipt("observation", `${id}-native-pointer`, nativePointer);
        } else await page.mouse.click(pointer.x, pointer.y, { button: item.button, clickCount: item.clickCount });
      } else if (item.target === "peek-target-title") {
        await peek.locator(".head .peekview-title").click();
      } else {
        const { execFileSync } = require("node:child_process");
        const native = (operation) => JSON.parse(execFileSync(path.join(root, "native-menu"),
          [String(pid), operation, item.target], { encoding: "utf8", timeout: 5000, stdio: ["ignore", "pipe", "pipe"] }));
        const visible = await until(`native ${item.target} menu`, () => {
          try { return native("inspect"); }
          catch (error) {
            if (!String(error.stderr).includes("expected one visible native menu item")) throw error;
            fs.writeFileSync(path.join(root, "native-menu-inspection.txt"), String(error.stderr));
            return false;
          }
        });
        receipt("observation", `${id}-native`, visible);
        if (item.target === "Peek Definition") {
          check("native-submenu", { title: visible.title, role: visible.role, enabled: visible.enabled });
          const b = visible.menuBounds;
          execFileSync("screencapture", ["-x", "-R", [Math.floor(b.x), Math.floor(b.y), Math.ceil(b.width), Math.ceil(b.height)].join(","),
            path.join(root, `${contract.id}-menu.png`)], { timeout: 5000 });
        }
        native(item.event === "hover" ? "hover" : "click");
      }
      /* The native helper verifies test-process focus before physical menu input. */

      actions.push(item);
      receipt("input", id, Object.fromEntries(Object.entries(item).filter(([key]) => key !== "id")));
    };
    const wire = async (id) => {
      const expected = contract.checks.find((item) => item.id === id).expected;
      const responses = await until(`${id} completed`, () => {
        const found = navigationResponses(logText().slice(boundary)).filter((item) => item.method === expected.method);
        return found.length > 0 && found;
      });
      for (const response of responses) {
        assert.equal(response.params.textDocument.uri, pathToFileURL(path.join(workspace, origin.file)).href);
        let result = null;
        if (response.result !== null) {
          const value = Array.isArray(response.result) ? response.result[0] : response.result;
          if (Array.isArray(response.result)) assert.equal(response.result.length, 1);
          const uri = value.targetUri ?? value.uri, range = value.targetSelectionRange ?? value.range;
          const doc = (await bridge("inspect")).documents.find((doc) => doc.uri === uri);
          assert.ok(doc, "peek or navigation must resolve the actual target document");
          result = { file: path.relative(workspace, fileURLToPath(uri)), range,
            text: doc.text.slice(offsetAt(doc.text, range.start), offsetAt(doc.text, range.end)) };
        }
        const observed = { method: response.method, request: { file: origin.file, position: response.params.position }, result };
        assert.deepEqual(observed, expected, "modifier hover/click must agree on exact target ownership");
        if (response === responses[0]) check(id, observed);
      }
      receipt("observation", `${id}-requests`, { ids: responses.map((item) => item.id) });
    };
    const modifier = async () => {
      try {
        await action("modifier-down"); await action("modifier-hover");
        if (unknown) {
          await until("unknown modifier hover completes", () => navigationResponses(logText().slice(boundary)).some((item) => item.method === "textDocument/definition"));
        } else {
          const link = sourceLine.locator(".goto-definition-link");
          const rendered = await until("definition link is rendered", async () => {
            const parts = await link.all();
            if (!parts.length) return false;
            const text = (await link.allTextContents()).join("");
            const visible = (await Promise.all(parts.map((part) => part.isVisible()))).every(Boolean);
            return text === "make" && visible && { text, visible };
          });
          check("source-link", rendered);
        }
        await action("modifier-click");
      }
      finally { await action("modifier-up"); }
    };
    const contextPeek = async () => {
      await action("context-menu"); await action("peek-submenu"); await action("peek-definition");
    };
    if (contract.id === "ux03-modifier-click" || unknown) {
      await modifier();
      await state(unknown ? "modifier-origin" : "destination");
      await wire(unknown ? "modifier-wire" : "wire-definition");
    }
    if (contract.id !== "ux03-modifier-click") {
      boundary = logText().length;
      await contextPeek();
      if (unknown) {
        const message = page.locator('[id="workbench.parts.editor"]').getByText("No definition found for 'missing'", { exact: true });
        await message.waitFor({ state: "visible" });
        check("empty-message", { text: await message.innerText(), visible: await message.isVisible() });
        await wire("peek-wire");
        check("peek-hidden", { visible: await peek.isVisible() });
        await state("final-origin");
      } else {
        await peek.waitFor({ state: "visible" });
        check("peek-visible", { visible: await peek.isVisible() });
        const preview = await line(peek, model.definition.text.split("\n")[1]);
        check("preview-line", { text: (await preview.textContent()).replaceAll("\u00a0", " "), visible: await preview.isVisible() });
        const highlight = peek.locator(".preview .reference-decoration");
        await highlight.waitFor({ state: "visible" });
        const glyphs = await preview.evaluate((element) => {
          const walker = document.createTreeWalker(element, NodeFilter.SHOW_TEXT);
          const nodes = []; let node;
          while ((node = walker.nextNode())) nodes.push(node);
          const text = element.textContent, start = text.indexOf("make"), end = start + 4;
          const range = document.createRange(); let offset = 0;
          for (const current of nodes) {
            const next = offset + current.textContent.length;
            if (start >= offset && start < next) range.setStart(current, start - offset);
            if (end > offset && end <= next) { range.setEnd(current, end - offset); break; }
            offset = next;
          }
          const r = range.getBoundingClientRect();
          return { text: range.toString(), x: r.x, y: r.y, width: r.width, height: r.height };
        });
        const box = await highlight.boundingBox();
        const aligned = box !== null && Math.abs(box.x - glyphs.x) <= 2 &&
          Math.abs(box.x + box.width - glyphs.x - glyphs.width) <= 2 &&
          box.y <= glyphs.y + glyphs.height && box.y + box.height >= glyphs.y;
        receipt("observation", "highlight-geometry", { box, glyphs });
        check("preview-highlight", { text: glyphs.text, visible: await highlight.isVisible(), aligned });
        await state("peek-source");
        await wire("wire-definition");
        await page.screenshot({ path: path.join(root, `${contract.id}-open.png`) });
        fs.writeFileSync(path.join(root, `${contract.id}-open.aria.txt`), await peek.ariaSnapshot());
        await action(contract.id === "ux03-peek-follow" ? "follow-target" : "dismiss-peek");
        await peek.waitFor({ state: "hidden" });
        if (contract.id === "ux03-peek-dismiss") {
          check("peek-hidden", { visible: await peek.isVisible() });
          check("restored-focus", { focused: await input.evaluate((element) => element === document.activeElement) });
          await state("final-origin");
        } else await state("destination");
      }
    }
    if (["ux03-modifier-click", "ux03-peek-follow"].includes(contract.id)) {
      const destinationInput = page.getByRole("textbox", { name: /^definitions\.vela/ });
      const targetEditor = page.locator(".monaco-editor").filter({ has: destinationInput });
      const targetLine = await line(targetEditor, model.definition.text.split("\n")[1]);
      check("destination-line", { text: (await targetLine.textContent()).replaceAll("\u00a0", " "), visible: await targetLine.isVisible() });
    }
    await page.screenshot({ path: path.join(root, `${contract.id}.png`) });
    fs.writeFileSync(path.join(root, `${contract.id}.aria.txt`), await page.locator('[id="workbench.parts.editor"]').ariaSnapshot());
    const finished = Date.now();
    onProof({ id: contract.id, fixture: contract.fixture, contractHash: evidence.jsonHash(contract), status: "passed",
      durationMs: finished - started, startedAt: new Date(started).toISOString(), finishedAt: new Date(finished).toISOString(), actions, checks });
    console.log(`PASS ${contract.id}`);
  }
}
module.exports = { runPeek };
