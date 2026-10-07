"use strict";
const assert = require("node:assert/strict"), test = require("node:test");
const spec = require("../../tests/lsp_matrix/fixtures/selection-members.json");
const oracle = require("./selection-oracle"), cases = spec.oracle.cases;
const flatten = row => {
  const result = [];
  for (; row; row = row.parent) {
    const r = row.range; result.push([r.start.line, r.start.character, r.end.line, r.end.character]);
  }
  return result;
};

test("selection members own fields methods constructors variants writes and true point empty policies", () => {
  assert.equal(cases.length, 57); assert.equal(new Set(cases.map(c => c.id)).size, 57);
  const queries = cases.flatMap(c => c.queries);
  assert.equal(queries.length, 177); assert.equal(queries.filter(q => q.chain.length).length, 163);
  assert.equal(queries.filter(q => !q.chain.length).length, 14);
  assert.equal(cases.filter(c => c.queries.some(q => q.chain.length)).length, 52);
  assert.equal(cases.filter(c => c.queries.length && c.queries.every(q => !q.chain.length)).length, 4);
  assert.equal(cases.filter(c => !c.queries.length).length, 1);
  assert.deepEqual(Object.keys(spec.files), ["scripts/main.vela", "scripts/helper.vela"]);
  assert.equal(spec.files[spec.oracle.file], cases[0].source);
  for (const id of ["record-field-and-named-method-ancestry", "source-field", "host-field", "trait-field", "dynamic-field",
    "unknown-field", "source-method", "host-method", "trait-method", "dynamic-method", "unknown-method",
    "nested-member-chain", "indexed-receiver-member", "member-then-index", "returned-receiver-member",
    "returned-receiver-method", "parenthesized-member-receiver", "tuple-index-member", "qualified-receiver-member",
    "method-field-argument", "chained-indexed-member-write", "record-explicit-and-shorthand-fields",
    "qualified-record-constructor", "empty-record-constructor", "nested-record-constructor", "constructed-receiver-field",
    "unit-variant-expression", "tuple-variant-constructor", "record-variant-constructor", "unknown-variant-constructor",
    "static-trait-method-path", "member-comments-and-trivia", "multiline-record-fields", "struct-member-declarations",
    "enum-all-variant-declarations", "required-trait-member", "default-trait-member", "inherent-member",
    "implemented-trait-member", "same-spelled-neighbor-members", "host-record-constructor", "unknown-record-constructor",
    "imported-alias-record-constructor", "record-trailing-comma", "constructed-receiver-method", "indexed-receiver-method"])
    assert(cases.some(c => c.id === id), id);
  for (const op of ["set", "add", "subtract", "multiply", "divide", "remainder"])
    assert(cases.some(c => c.id === "member-write-" + op));
  assert(oracle.document(cases.find(c => c.id === "imported-alias-record-constructor").source).text
    .includes("use helper::Known as Alias;"));
  assert(oracle.document(spec.files["scripts/helper.vela"]).text.includes("pub struct Known { value: i64 }"));
  for (const c of cases) {
    const doc = oracle.document(c.source);
    for (const q of c.queries) { assert(doc.markers[q.position]); for (const name of q.chain) assert(doc.markers[name]); }
  }
});

test("selection member goldens pin complete nested UTF16 byte label value and independent helper chains", () => {
  for (const crlf of [false, true]) for (const shifted of [false, true]) {
    const c = cases[0], doc = oracle.document(c.source, crlf, shifted), line = shifted ? 2 : 0;
    const positions = oracle.positions(doc, c.queries), utf16 = oracle.expected(doc, c.queries), bytes = oracle.expected(doc, c.queries, false);
    assert.deepEqual(positions[0], positions[2]); assert.deepEqual(utf16[0], utf16[2]); assert.equal(utf16.length, 13);
    assert.deepEqual(flatten(utf16[0]), [[line, 59, line, 64], [line, 49, line, 64], [line, 42, line, 64],
      [line, 40, line, 96], [line, 33, line, 96], [line, 22, line, 97], [line, 20, line, 118],
      [line, 8, line, 118], [0, 0, line + 1, 0]]);
    assert.deepEqual(flatten(bytes[0]), [[line, 63, line, 68], [line, 53, line, 68], [line, 46, line, 68],
      [line, 44, line, 100], [line, 37, line, 100], [line, 26, line, 101], [line, 24, line, 122],
      [line, 12, line, 122], [0, 0, line + 1, 0]]);
    assert.deepEqual(flatten(utf16[7]), [[line, 84, line, 87], [line, 84, line, 94], [line, 83, line, 95],
      [line, 73, line, 95], [line, 66, line, 95], [line, 40, line, 96], [line, 33, line, 96],
      [line, 22, line, 97], [line, 20, line, 118], [line, 8, line, 118], [0, 0, line + 1, 0]]);
    assert.deepEqual(flatten(utf16[10]), [[line, 110, line, 115], [line, 105, line, 115],
      [line, 98, line, 116], [line, 20, line, 118], [line, 8, line, 118], [0, 0, line + 1, 0]]);
    assert.equal(doc.markers.file.start.byte, 0); assert.equal(doc.markers.file.end.byte, Buffer.byteLength(doc.text));
    assert.equal(doc.text.includes("\r\n"), crlf);
    const helper = oracle.document(spec.files["scripts/helper.vela"], crlf, shifted);
    assert.deepEqual(oracle.expected(helper, spec.oracle.helperQueries).map(flatten), [
      [[line, 55, line, 60], [line, 51, line, 60], [line, 43, line, 60], [line, 42, line, 61],
        [line, 32, line, 61], [line, 25, line, 62], [line, 23, line, 64], [line, 8, line, 64], [0, 0, line + 1, 0]],
      [[line, 43, line, 48], [line, 43, line, 60], [line, 42, line, 61], [line, 32, line, 61],
        [line, 25, line, 62], [line, 23, line, 64], [line, 8, line, 64], [0, 0, line + 1, 0]]]);
  }
});

test("selection member parents contain whole children without phantom declaration or duplicate spans", () => {
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
