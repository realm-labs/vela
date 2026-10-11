"use strict";
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const entries = ["suite.js", "workspace-symbol-suite.js", "selection-suite.js"];

function editorTests(root) {
  const names = entries.flatMap(entry => [...fs.readFileSync(path.join(root, "editors/vscode/test", entry), "utf8")
    .matchAll(/await check\("([^"]+)"/g)].map(match => match[1]));
  assert.equal(new Set(names).size, names.length, "unique checks across isolated editor suites");
  return names;
}
function mergeEditorResults(first, second) {
  assert.equal(first.version, 1); assert.equal(second.version, 1);
  assert.equal(first.vscodeVersion, second.vscodeVersion, "same pinned editor");
  assert(first.provenance && second.provenance, "installed package provenance present");
  assert.deepEqual(first.provenance, second.provenance, "same source, server and platform");
  assert(Array.isArray(first.results) && Array.isArray(second.results));
  const results = [...first.results, ...second.results];
  assert.equal(new Set(results.map(row => row.name)).size, results.length, "no overwritten/duplicate checks");
  return { ...first, results };
}
module.exports = { editorTests, mergeEditorResults };
