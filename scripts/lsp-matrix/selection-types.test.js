"use strict";
const assert = require("node:assert/strict"), test = require("node:test");
const spec = require("../../tests/lsp_matrix/fixtures/selection-types.json");
const oracle = require("./selection-oracle");
const cases = spec.oracle.cases;
const flatten = row => {
  const result = [];
  for (; row; row = row.parent) {
    const r = row.range; result.push([r.start.line, r.start.character, r.end.line, r.end.character]);
  }
  return result;
};

test("selection type corpus owns primitive qualified nested hints and true point empty policies", () => {
  assert.equal(cases.length, 47); assert.equal(new Set(cases.map(c => c.id)).size, 47);
  const queries = cases.flatMap(c => c.queries);
  assert.equal(queries.length, 232); assert.equal(queries.filter(q => q.chain.length).length, 219);
  assert.equal(queries.filter(q => !q.chain.length).length, 13);
  assert.equal(cases.filter(c => c.queries.some(q => q.chain.length)).length, 42);
  assert.equal(cases.filter(c => c.queries.length && c.queries.every(q => !q.chain.length)).length, 4);
  assert.equal(cases.filter(c => !c.queries.length).length, 1);
  assert.deepEqual(Object.keys(spec.files), ["scripts/main.vela", "scripts/helper.vela"]);
  assert.equal(spec.files[spec.oracle.file], cases[0].source);
  for (const id of ["nested-parameter-and-return", "all-primitive-and-dynamic-hints", "bare-builtin-container-hints",
    "source-host-trait-unknown-and-qualified-spellings", "unit-parameter", "tuple-parameter", "nested-tuple-parameter",
    "tuple-element-trailing-trivia", "nested-array-hint", "nested-set-hint", "nested-iterator-hint", "nested-option-hint",
    "map-with-tuple-and-result", "result-with-two-nested-containers", "type-argument-trailing-trivia", "multiline-nested-hints",
    "comments-inside-type-arguments", "unit-return", "qualified-return", "nested-return", "async-type-signature",
    "defaulted-parameter-hint", "typed-lambda-hint", "typed-lambda-in-default", "two-typed-lambda-parameters",
    "local-nested-hint", "uninitialized-local-hint", "constant-hint", "state-hint", "extern-state-hint",
    "struct-field-hints", "tuple-variant-hints", "record-variant-hints", "required-trait-signature", "trait-default-signature",
    "inherent-method-signature", "trait-impl-method-signature", "impl-header-type-spellings", "hint-with-internal-comment",
    "same-spelled-type-neighbors", "imported-alias-hint"])
    assert(cases.some(c => c.id === id), id);
  assert.equal(cases.find(c => c.id === "all-primitive-and-dynamic-hints").queries.length, 18);
  assert.equal(cases.find(c => c.id === "bare-builtin-container-hints").queries.length, 12);
  const views = cases.find(c => c.id === "builtin-view-mut-hints");
  assert.equal(views.queries.length, 14);
  for (const name of ["ArrayView", "ArrayMut", "SetView", "SetMut", "MapView", "MapMut"])
    assert(oracle.document(views.source).text.includes(name + "<"));
  assert.equal(cases.find(c => c.id === "source-host-trait-unknown-and-qualified-spellings").queries.length, 7);
  assert.equal(oracle.document(cases.find(c => c.id === "map-with-tuple-and-result").source).text,
    "/*中😀*/ fn typed(value: Map<String, (i64, Result<bool, ()>)>) {}\n");
  const multiline = oracle.document(cases.find(c => c.id === "multiline-nested-hints").source);
  assert.equal(multiline.markers.param.start.line, 0); assert.equal(multiline.markers.param.start.character, 17);
  assert.equal(multiline.markers.param.end.line, 4); assert.equal(multiline.markers.param.end.character, 0);
  for (const c of cases) {
    const doc = oracle.document(c.source);
    for (const q of c.queries) { assert(doc.markers[q.position]); for (const name of q.chain) assert(doc.markers[name]); }
  }
});

test("selection type goldens pin complete nested UTF16 byte and independent helper chains", () => {
  for (const crlf of [false, true]) for (const shifted of [false, true]) {
    const c = cases[0], doc = oracle.document(c.source, crlf, shifted), line = shifted ? 2 : 0;
    const positions = oracle.positions(doc, c.queries), utf16 = oracle.expected(doc, c.queries), bytes = oracle.expected(doc, c.queries, false);
    assert.deepEqual(positions[0], positions[2]); assert.deepEqual(utf16[0], utf16[2]); assert.equal(utf16.length, 12);
    assert.deepEqual(flatten(utf16[0]), [[line, 50, line, 53], [line, 49, line, 60], [line, 43, line, 60],
      [line, 34, line, 61], [line, 31, line, 61], [line, 30, line, 62], [line, 24, line, 62],
      [line, 17, line, 62], [line, 16, line, 63], [line, 8, line, 100], [0, 0, line + 1, 0]]);
    assert.deepEqual(flatten(bytes[0]), [[line, 54, line, 57], [line, 53, line, 64], [line, 47, line, 64],
      [line, 38, line, 65], [line, 35, line, 65], [line, 34, line, 66], [line, 28, line, 66],
      [line, 21, line, 66], [line, 20, line, 67], [line, 12, line, 104], [0, 0, line + 1, 0]]);
    assert.deepEqual(flatten(utf16[8]), [[line, 74, line, 76], [line, 73, line, 77], [line, 68, line, 77],
      [line, 67, line, 82], [line, 8, line, 100], [0, 0, line + 1, 0]]);
    assert.deepEqual(flatten(utf16[9]), [[line, 79, line, 80], [line, 79, line, 81], [line, 67, line, 82],
      [line, 8, line, 100], [0, 0, line + 1, 0]]);
    assert.equal(doc.markers.file.start.byte, 0); assert.equal(doc.markers.file.end.byte, Buffer.byteLength(doc.text));
    assert.equal(doc.text.includes("\r\n"), crlf);
    const helper = oracle.document(spec.files["scripts/helper.vela"], crlf, shifted);
    assert.deepEqual(oracle.expected(helper, spec.oracle.helperQueries).map(flatten), [
      [[line, 29, line, 33], [line, 28, line, 34], [line, 25, line, 34], [line, 18, line, 34],
        [line, 17, line, 35], [line, 8, line, 60], [0, 0, line + 1, 0]],
      [[line, 39, line, 42], [line, 8, line, 60], [0, 0, line + 1, 0]]]);
  }
});

test("selection type parents contain their full authored children without phantom or duplicate spans", () => {
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
