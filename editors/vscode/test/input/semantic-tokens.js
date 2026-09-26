"use strict";
const fs = require("node:fs");
const path = require("node:path");
const assert = require("node:assert/strict");
const {
  tokenRenderModel,
} = require("../../../../scripts/lsp-matrix/token-render-contracts");
const evidence = require("../../../../scripts/lsp-matrix/local-evidence");
const { fileUri } = require("./paths");
const { tokenGeometry } = require("./token-geometry");

async function runTokenRendering({
  page,
  bridge,
  record,
  root,
  workspace,
  contracts,
  until,
  onProof,
}) {
  const model = tokenRenderModel();
  const textbox = page.getByRole("textbox", { name: /^tokens_input\.vela/ });
  const editor = page.locator(".monaco-editor").filter({ has: textbox });
  for (const contract of contracts.filter((item) =>
    item.id.startsWith("ux09-"),
  )) {
    const started = Date.now(),
      actions = [],
      checks = [],
      observations = [];
    const receipt = (kind, id, details) =>
      record(kind, id, { proof: contract.id, ...details });
    const check = (id, observed) => {
      const expected = contract.checks.find((item) => item.id === id);
      assert(expected, `registered assertion ${id}`);
      assert.deepEqual(observed, expected.expected, `${contract.id}/${id}`);
      checks.push({ ...expected, status: "passed", observed });
      receipt("assertion", id, { expected: expected.expected, observed });
    };
    const action = async (id, callback) => {
      const item = contract.actions.find((item) => item.id === id);
      assert(item, `registered input ${id}`);
      if (callback) await callback(item);
      else if (item.text !== undefined)
        await page.keyboard.insertText(item.text);
      else await page.keyboard.press(item.key);
      actions.push(item);
      receipt(
        "input",
        id,
        Object.fromEntries(
          Object.entries(item).filter(([key]) => key !== "id"),
        ),
      );
    };
    const retain = (id, value) => {
      observations.push({ id, value });
      fs.writeFileSync(
        path.join(root, `${contract.id}-observations.json`),
        JSON.stringify(observations, null, 2),
      );
    };
    const source = async (id) => {
      const expected = contract.checks.find((item) => item.id === id).expected;
      const observed = await until(id, async () => {
        const state = await bridge("inspect");
        if (state.active?.uri !== fileUri(path.join(workspace, model.file)))
          return null;
        const value = {
          file: model.file,
          text: state.active.text,
          dirty: state.active.dirty,
          disk: fs.readFileSync(path.join(workspace, model.file), "utf8"),
        };
        retain(id, value);
        return JSON.stringify(value) === JSON.stringify(expected) && value;
      });
      check(id, observed);
    };
    const styles = async (id, document) => {
      const expected = contract.checks.find((item) => item.id === id).expected;
      const result = await until(id, async () => {
        const geometry = await tokenGeometry(editor, document);
        retain(id, geometry);
        const observed = expected.map(({ marker }) => {
          const glyph = geometry.glyphs[marker];
          return {
            marker,
            text: glyph.text,
            line: glyph.line,
            start: glyph.start,
            end: glyph.end,
            visible: glyph.visible,
            aligned: glyph.aligned,
            style: glyph.style,
          };
        });
        return (
          JSON.stringify(observed) === JSON.stringify(expected) && observed
        );
      });
      check(id, result);
    };
    await bridge("setup", {
      file: model.file,
      line: 0,
      character: 0,
      reset: true,
    });
    await textbox.focus();
    await source("origin");
    await styles("initial-styles", model.disk);
    await page.screenshot({
      path: path.join(root, `${contract.id}-before.png`),
    });
    if (contract.id === "ux09-edit-scroll") {
      await action("home");
      await action("type-unicode");
      await source("edited-source");
      await styles("shifted-styles", model.shifted);
      for (const item of contract.actions.filter((item) =>
        item.id.startsWith("scroll-out-"),
      ))
        await action(item.id, async (item) => {
          await editor.hover();
          await page.mouse.wheel(0, item.deltaY);
        });
      const out = await until(
        "target spans genuinely leave the viewport",
        async () => {
          const geometry = await tokenGeometry(editor, model.shifted);
          retain("scroll-out", geometry);
          const observed = {
            headVisible: Object.keys(model.spec.oracle.roles).some(
              (name) => geometry.glyphs[name].visible,
            ),
            tailVisible: geometry.glyphs.tail.visible,
          };
          return !observed.headVisible && observed.tailVisible && observed;
        },
      );
      check("scrolled-out", out);
      await page.screenshot({
        path: path.join(root, `${contract.id}-scrolled-out.png`),
      });
      await source("scroll-source");
      for (const item of contract.actions.filter((item) =>
        item.id.startsWith("scroll-back-"),
      ))
        await action(item.id, async (item) => {
          await editor.hover();
          await page.mouse.wheel(0, item.deltaY);
        });
      await styles("returned-styles", model.shifted);
      await textbox.focus();
      const undoActions = contract.actions.filter((item) =>
        item.id.startsWith("undo-edit-"),
      );
      for (let index = 1; index <= undoActions.length; index++) {
        await action(`undo-edit-${index}`);
        await source(
          index === undoActions.length ? "undo-source" : `undo-step-${index}`,
        );
      }
      await styles("undo-styles", model.disk);
    } else if (contract.id === "ux09-unresolved-token") {
      await action("select-call", async (item) => {
        const glyph = (await tokenGeometry(editor, model.disk)).glyphs[
          item.target
        ];
        assert.equal(glyph.text, "make");
        assert(glyph.visible);
        await page.mouse.click(
          glyph.rect.x + glyph.rect.width / 2,
          glyph.rect.y + glyph.rect.height / 2,
          { clickCount: item.clickCount },
        );
      });
      const selected = (await bridge("inspect")).active.selections[0];
      check("selected-call", selected);
      await action("type-unknown");
      await source("unknown-source");
      await styles("unknown-styles", model.unknown);
      await page.screenshot({
        path: path.join(root, `${contract.id}-unknown.png`),
      });
      check("decoy-unopened", {
        opened: (await bridge("inspect")).documents.some(
          (doc) =>
            doc.uri ===
            fileUri(path.join(workspace, "scripts/token_decoy.vela")),
        ),
      });
      await action("undo-unknown");
      await source("undo-source");
      await styles("undo-styles", model.disk);
    } else {
      const settingRow = () =>
        page
          .locator(".settings-editor .setting-item")
          .filter({ hasText: "Semantic Highlighting: Enabled" });
      const select = () => settingRow().locator(".monaco-select-box");
      await action("open-settings");
      await page.locator(".settings-editor").waitFor({ state: "visible" });
      await until("settings search owns keyboard focus", async () =>
        page.evaluate(
          () =>
            document.activeElement?.getAttribute("aria-label") ===
            "Search settings",
        ),
      );
      await action("search-setting");
      await settingRow().waitFor({ state: "visible" });
      const settingsAria = await page
        .locator(".settings-editor")
        .ariaSnapshot();
      retain("settings-ui", settingsAria);
      fs.writeFileSync(
        path.join(root, `${contract.id}-settings.aria.txt`),
        settingsAria,
      );
      check("visible-setting", {
        key: model.spec.oracle.setting,
        label:
          `${await settingRow().locator(".setting-item-category").innerText()} ${await settingRow().locator(".setting-item-label").innerText()}`
            .replace(/\s+/g, " ")
            .trim(),
        visible: await settingRow().isVisible(),
      });
      await action("open-disabled-values", async (item) =>
        select().click({ clickCount: item.clickCount }),
      );
      await action("first-disabled-value");
      await action("next-disabled-value");
      await action("select-disabled");
      await until(
        "disabled semantic setting",
        async () => (await bridge("token-setting")).value === false,
      );
      check("disabled-setting", await bridge("token-setting"));
      await action("return-disabled");
      await textbox.waitFor({ state: "visible" });
      await source("disabled-source");
      await styles("disabled-styles", model.disk);
      await page.screenshot({
        path: path.join(root, `${contract.id}-disabled.png`),
      });
      await action("open-settings-again");
      await settingRow().waitFor({ state: "visible" });
      await action("open-enabled-values", async (item) =>
        select().click({ clickCount: item.clickCount }),
      );
      await action("first-enabled-value");
      await action("select-enabled");
      await until(
        "enabled semantic setting",
        async () => (await bridge("token-setting")).value === true,
      );
      check("enabled-setting", await bridge("token-setting"));
      await action("return-enabled");
      await textbox.waitFor({ state: "visible" });
      await source("enabled-source");
      await styles("enabled-styles", model.disk);
    }
    await page.screenshot({
      path: path.join(root, `${contract.id}-after.png`),
    });
    const finished = Date.now();
    onProof({
      id: contract.id,
      fixture: contract.fixture,
      contractHash: evidence.jsonHash(contract),
      status: "passed",
      durationMs: finished - started,
      startedAt: new Date(started).toISOString(),
      finishedAt: new Date(finished).toISOString(),
      actions,
      checks,
    });
    console.log(`PASS ${contract.id}`);
  }
}
module.exports = { runTokenRendering };
