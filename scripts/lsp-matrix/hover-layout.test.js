"use strict";
const test = require("node:test"), assert = require("node:assert/strict");
const { hoverLineReady } = require("../../editors/vscode/test/input/hover-layout");

test("native hover waits for the authored inlay labels that shifted the recorded pointer geometry", () => {
  // The failed capture measured combine at x631; its later inlay render moved
  // combine to x675, and no hover request was emitted at the old coordinate.
  const before = "    /* 中😀 */ let value = combine(1);";
  const after = "    /* 中😀 */ let value: i64 = combine(left:1);";
  assert.equal(hoverLineReady([before]), false);
  assert.equal(hoverLineReady([after]), true);
  assert.equal(hoverLineReady([after.replaceAll(" ", "\u00a0")]), true);
  assert.equal(hoverLineReady([after.replace(": i64", ": String")]), false);
  assert.equal(hoverLineReady([after.replace("left:", "right:")]), false);
  assert.equal(hoverLineReady([after, after]), false);
  assert.equal(hoverLineReady([after.replace("combine", "other")]), false);
  assert.equal(hoverLineReady([]), false);
});
