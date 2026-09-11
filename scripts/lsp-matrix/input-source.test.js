"use strict";
const test = require("node:test");
const assert = require("node:assert/strict");
const { withInputSource } = require("../../editors/vscode/test/input/keyboard-layout");

test("pinned keyboard setup restores the original source after success and failure", async () => {
  for (const failAt of [null, "selection", "run"]) {
    let source = "pinyin", ran = false;
    const invoke = (operation, selected) => {
      if (operation === "get") return source;
      source = selected;
      if (selected === "ABC" && failAt === "selection") throw Error("selection failure");
    };
    const promise = withInputSource("ABC", async () => {
      ran = true;
      assert.equal(source, "ABC");
      if (failAt === "run") throw Error("run failure");
      return 42;
    }, invoke, () => {});
    if (failAt) await assert.rejects(promise, new RegExp(`${failAt} failure`));
    else assert.equal(await promise, 42);
    assert.equal(source, "pinyin");
    assert.equal(ran, failAt !== "selection");
  }
});

test("matching input source is untouched and restoration failure is not hidden", async () => {
  await withInputSource("ABC", async () => {}, (operation) => {
    assert.equal(operation, "get"); return "ABC";
  });
  await assert.rejects(withInputSource("ABC", async () => {}, (operation, source) => {
    if (operation === "get") return "pinyin";
    if (source === "pinyin") throw Error("restore failure");
  }, () => {}), /restore failure/);
});
