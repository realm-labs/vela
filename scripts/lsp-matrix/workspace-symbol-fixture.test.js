"use strict";
const test = require("node:test");
const assert = require("node:assert/strict");
const spec = require("../../tests/lsp_matrix/fixtures/workspace-symbol-declarations.json");
const { parseMarkers } = require("./fixtures");
const { symbols, queries } = spec.oracle;

test("workspace symbol corpus pins 31 owned rows and declaration partitions in four real sources", () => {
  assert.equal(Object.keys(spec.files).length, 4); assert.equal(symbols.length, 31);
  assert.equal(new Set(symbols.map(row => row.id)).size, 31);
  const counts = {};
  for (const row of symbols) {
    counts[row.kind] = (counts[row.kind] ?? 0) + 1;
    const doc = parseMarkers(spec.files[row.file]), marker = doc.markers[row.range];
    assert(marker && marker.start.byte < marker.end.byte);
    if (["File", "Module"].includes(row.kind)) assert.deepEqual([marker.start.byte, marker.end.byte], [0, Buffer.byteLength(doc.text)]);
  }
  assert.deepEqual(counts, { Module: 4, File: 4, Enum: 2, Interface: 2, Constant: 3, Struct: 4, Function: 6, Variable: 4, Object: 2 });
  assert(symbols.every(row => row.file.startsWith("scripts/") && typeof row.protocolKind === "number"));
  assert.equal(symbols.find(row => row.id === "api-async").detail, "async (value: i64) -> i64");
  assert.equal(symbols.find(row => row.id === "api-choose").detail, "(value: i64, amount: i64) -> i64");
});

test("workspace symbol queries independently pin duplicate owners, case and trim policies and exact exclusions", () => {
  assert.equal(queries.length, 32);
  const expected = id => queries.find(row => row.id === id).symbols;
  assert.deepEqual(expected("all"), symbols.map(row => row.id));
  assert.deepEqual(expected("trim-empty"), expected("all"));
  assert.deepEqual(expected("case-fold"), ["api-widget", "api-trait-impl", "api-impl", "helper-widget", "nested-api-widget"]);
  assert.deepEqual(expected("trim-case"), expected("case-fold"));
  assert.deepEqual(expected("duplicate-files"), ["api-file", "nested-api-file"]);
  assert.deepEqual(expected("duplicate-constants"), ["api-limit", "nested-api-limit"]);
  assert.deepEqual(expected("imports-only"), ["imports-module", "imports-file"]);
  assert.deepEqual(expected("private-case"), ["api-private-struct", "api-private"]);
  assert.equal(expected("qualified-substring").length, 21);
  for (const row of queries) {
    assert.equal(new Set(row.symbols).size, row.symbols.length);
    assert(row.symbols.every(id => symbols.some(symbol => symbol.id === id)));
  }
  for (const id of ["detail-is-not-name", "no-wildcards", "interior-space", "alias", "import-alias", "field-local", "method", "tuple-variant", "record-variant", "local", "parameter", "attribute-text", "unicode-trivia", "no-match"]) assert.deepEqual(expected(id), [], id);
});

test("workspace symbol declaration ranges pin UTF-16 versus byte goldens through both line endings and two-line shifts", () => {
  for (const crlf of [false, true]) for (const shifted of [false, true]) for (const [file, raw] of Object.entries(spec.files)) {
    let text = shifted ? raw.replace("[[file:start]]", "[[file:start]]// shifted 中😀\n/* extra 😀 */\n") : raw;
    if (crlf) text = text.replaceAll("\n", "\r\n");
    const doc = parseMarkers(text);
    assert.deepEqual([doc.markers.file.start.byte, doc.markers.file.end.byte], [0, Buffer.byteLength(doc.text)]);
    if (file === "scripts/api.vela") {
      const p = doc.markers["limit-range"].start;
      assert.deepEqual([p.line, p.character], [1 + (shifted ? 2 : 0), 8]);
      assert.equal(p.byte - Buffer.from(doc.text).lastIndexOf(10, p.byte - 1) - 1, 12);
    }
  }
});
