"use strict";
const assert = require("node:assert/strict"), test = require("node:test");
const spec = require("../../tests/lsp_matrix/fixtures/selection-declarations.json");
const oracle = require("./selection-oracle");
const cases = spec.oracle.cases;
const flatten = row => {
  const result = [];
  for (; row; row = row.parent) {
    const r = row.range; result.push([r.start.line, r.start.character, r.end.line, r.end.character]);
  }
  return result;
};

test("selection declaration corpus owns all item forms defaults methods siblings and true point/empty vectors", () => {
  assert.equal(cases.length, 38); assert.equal(new Set(cases.map(c => c.id)).size, 38);
  const queries = cases.flatMap(c => c.queries);
  assert.equal(queries.length, 132); assert.equal(queries.filter(q => q.chain.length).length, 122);
  assert.equal(queries.filter(q => !q.chain.length).length, 10);
  assert.equal(cases.filter(c => c.queries.some(q => q.chain.length)).length, 34);
  assert.equal(cases.filter(c => c.queries.length && c.queries.every(q => !q.chain.length)).length, 3);
  assert.equal(cases.filter(c => !c.queries.length).length, 1);
  assert.deepEqual(Object.keys(spec.files), ["scripts/main.vela", "scripts/helper.vela"]);
  assert.equal(spec.files[spec.oracle.file], cases[0].source);
  for (const id of ["function-default-signature", "private-empty-function", "async-function", "attributed-function",
    "qualified-parameter-return", "tuple-parameter-return", "builtin-parameter-hint", "function-neighbors",
    "constant", "public-constant", "attributed-constant", "state", "public-state", "extern-state", "public-extern-state",
    "attributed-state", "empty-struct", "struct-field-defaults", "struct-attributed-field", "struct-qualified-field",
    "enum-unit-variants", "enum-tuple-defaults", "enum-record-defaults", "enum-attributed-variant",
    "trait-required", "trait-default", "inherent-impl", "trait-impl", "trait-method-neighbors", "impl-self-parameter",
    "public-use-declaration", "all-declaration-neighbors"]) assert(cases.some(c => c.id === id), id);
  assert.deepEqual(cases.find(c => c.id === "enum-unit-variants").queries[0].chain, ["second", "variants", "item", "file"]);
  assert.deepEqual(cases.find(c => c.id === "impl-self-parameter").queries[0].chain, ["self", "params", "method", "item", "file"]);
  assert.deepEqual(cases.find(c => c.id === "enum-tuple-defaults").queries[2].chain,
    ["first-value", "first", "tuple", "variant", "variants", "item", "file"]);
  for (const c of cases) {
    const doc = oracle.document(c.source);
    for (const q of c.queries) { assert(doc.markers[q.position]); for (const name of q.chain) assert(doc.markers[name]); }
  }
});

test("selection declaration complete goldens pin UTF16 bytes duplicate input order and independent helper ancestry", () => {
  for (const crlf of [false, true]) for (const shifted of [false, true]) {
    const c = cases[0], doc = oracle.document(c.source, crlf, shifted), line = shifted ? 2 : 0;
    const positions = oracle.positions(doc, c.queries), utf16 = oracle.expected(doc, c.queries), bytes = oracle.expected(doc, c.queries, false);
    assert.deepEqual(positions[0], positions[2]); assert.deepEqual(utf16[0], utf16[2]); assert.equal(utf16.length, 13);
    assert.deepEqual(flatten(utf16[0]), [[line, 43, line, 49], [line, 42, line, 62], [line, 23, line, 63], [line, 8, line, 89], [0, 0, line + 1, 0]]);
    assert.deepEqual(flatten(bytes[0]), [[line, 47, line, 53], [line, 46, line, 66], [line, 27, line, 67], [line, 12, line, 93], [0, 0, line + 1, 0]]);
    assert.deepEqual(flatten(utf16[1]), [[line, 15, line, 23], [line, 8, line, 89], [0, 0, line + 1, 0]]);
    assert.deepEqual(flatten(bytes[1]), [[line, 19, line, 27], [line, 12, line, 93], [0, 0, line + 1, 0]]);
    assert.deepEqual(flatten(utf16[11]), [[line, 80, line, 86], [line, 73, line, 87], [line, 71, line, 89], [line, 8, line, 89], [0, 0, line + 1, 0]]);
    assert.equal(doc.markers.file.start.byte, 0); assert.equal(doc.markers.file.end.byte, Buffer.byteLength(doc.text));
    assert.equal(doc.text.includes("\r\n"), crlf);
    const helper = oracle.document(spec.files["scripts/helper.vela"], crlf, shifted);
    assert.deepEqual(flatten(oracle.expected(helper, spec.oracle.helperQueries)[0]), [[line, 14, line, 20], [line, 8, line, 26], [0, 0, line + 1, 0]]);
    assert.deepEqual(flatten(oracle.expected(helper, spec.oracle.helperQueries, false)[0]), [[line, 18, line, 24], [line, 12, line, 30], [0, 0, line + 1, 0]]);
  }
});

test("selection declaration parents strictly contain owned children and never invent comment or EOF ancestry", () => {
  const before = (a, b) => a.line < b.line || (a.line === b.line && a.character <= b.character);
  for (const crlf of [false, true]) for (const shifted of [false, true]) for (const c of cases) {
    const doc = oracle.document(c.source, crlf, shifted), positions = oracle.positions(doc, c.queries);
    const actual = oracle.expected(doc, c.queries); assert.equal(actual.length, c.queries.length);
    for (const [index, result] of actual.entries()) {
      if (!c.queries[index].chain.length) {
        assert.deepEqual(result, { range: { start: positions[index], end: positions[index] } }); continue;
      }
      let child;
      for (let row = result; row; row = row.parent) {
        assert(before(row.range.start, positions[index]) && before(positions[index], row.range.end));
        assert.notDeepEqual(row.range.start, row.range.end);
        if (child) { assert(before(row.range.start, child.start) && before(child.end, row.range.end)); assert.notDeepEqual(row.range, child); }
        child = row.range;
      }
    }
  }
});
