"use strict";
const assert = require("node:assert/strict"), test = require("node:test");
const spec = require("../../tests/lsp_matrix/fixtures/selection-patterns.json");
const oracle = require("./selection-oracle"), cases = spec.oracle.cases;
const flatten = row => {
  const result = [];
  for (; row; row = row.parent) {
    const r = row.range; result.push([r.start.line, r.start.character, r.end.line, r.end.character]);
  }
  return result;
};

test("selection patterns own complete match guard loop control and true point empty partitions", () => {
  assert.equal(cases.length, 79); assert.equal(new Set(cases.map(c => c.id)).size, 79);
  const queries = cases.flatMap(c => c.queries);
  assert.equal(queries.length, 204); assert.equal(queries.filter(q => q.chain.length).length, 191);
  assert.equal(queries.filter(q => !q.chain.length).length, 13);
  assert.equal(cases.filter(c => c.queries.some(q => q.chain.length)).length, 74);
  assert.equal(cases.filter(c => c.queries.length && c.queries.every(q => !q.chain.length)).length, 4);
  assert.equal(cases.filter(c => !c.queries.length).length, 1);
  assert.deepEqual(Object.keys(spec.files), ["scripts/main.vela", "scripts/helper.vela"]);
  assert.equal(spec.files[spec.oracle.file], cases[0].source);
  for (const id of ["nested-record-pattern-guard-for-and-control-owners", "wildcard-pattern", "binding-pattern",
    "unit-variant-pattern", "tuple-pattern", "tuple-pattern-trailing-comma", "nested-tuple-pattern",
    "empty-tuple-variant-pattern", "record-pattern-label-value-and-shorthand", "same-spelled-record-label-and-binding",
    "nested-record-pattern", "host-record-variant-pattern", "unknown-record-variant-pattern", "empty-record-pattern",
    "record-pattern-trailing-trivia", "multiline-record-and-tuple-pattern", "guard-binary-and-keyword", "guard-named-call",
    "guard-lambda-call", "block-arm-return", "empty-match-arms", "standalone-match-expression",
    "same-spelled-neighbor-arm-bindings", "nested-for-if-controls", "let-tuple-destructuring", "let-record-destructuring",
    "if-else-if-expression", "imported-source-enum-pattern", "source-enum-separate-declaration",
    "builtin-option-some-pattern", "builtin-result-ok-pattern", "builtin-result-err-pattern", "builtin-option-none-pattern",
    "imported-enum-record-alias-pattern", "imported-tuple-variant-alias-pattern", "imported-record-variant-alias-pattern",
    "imported-unit-variant-alias-pattern", "match-arm-trailing-comment-and-semicolon", "newline-separated-match-arms",
    "for-multiline-binding-and-comment", "match-inside-for-control-bodies", "let-bare-binding-without-pattern-owner",
    "let-wildcard-binding", "standalone-if-else-controls", "nested-match-arm-expression"])
    assert(cases.some(c => c.id === id), id);
  for (const owner of ["source", "host", "unknown"]) for (const kind of ["qualified-unit", "tuple-variant"])
    assert(cases.some(c => c.id === owner + "-" + kind + "-pattern"));
  for (const kind of ["true", "false", "integer", "float", "string", "character"])
    assert(cases.some(c => c.id === "literal-pattern-" + kind));
  for (const op of ["set", "add", "subtract", "multiply", "divide", "remainder"])
    assert(cases.some(c => c.id === "guard-assignment-" + op));
  for (const kind of ["single", "wildcard", "tuple", "key-value", "nested-key-value", "record"])
    assert(cases.some(c => c.id === "for-" + kind + "-binding"));
  for (const kind of ["range", "call", "method", "index", "parenthesized"])
    assert(cases.some(c => c.id === "for-" + kind + "-iterable"));
  const text = id => oracle.document(cases.find(c => c.id === id).source).text;
  assert(text("imported-source-enum-pattern").includes("use helper::Choice as Alias;"));
  assert(text("imported-record-variant-alias-pattern").includes("use helper::Choice::Record as Alias;"));
  assert(oracle.document(spec.files["scripts/helper.vela"]).text.includes("pub enum Choice { Unit, Tuple(i64, i64), Record"));
  for (const c of cases) {
    const doc = oracle.document(c.source);
    for (const q of c.queries) { assert(doc.markers[q.position]); for (const name of q.chain) assert(doc.markers[name]); }
  }
});

test("selection pattern goldens pin UTF16 byte guards field trivia controls and independent helper chains", () => {
  for (const crlf of [false, true]) for (const shifted of [false, true]) {
    const c = cases[0], doc = oracle.document(c.source, crlf, shifted), line = shifted ? 2 : 0;
    const positions = oracle.positions(doc, c.queries), utf16 = oracle.expected(doc, c.queries), bytes = oracle.expected(doc, c.queries, false);
    const spans = ranges => ranges.map(([start, end]) => [line, start, line, end]).concat([[0, 0, line + 1, 0]]);
    assert.deepEqual(positions[0], positions[2]); assert.deepEqual(utf16[0], utf16[2]); assert.equal(utf16.length, 15);
    assert.deepEqual(flatten(utf16[0]), spans([[74, 78], [73, 85], [66, 85], [49, 92], [49, 192], [47, 209], [36, 209], [23, 210], [21, 227], [8, 227]]));
    assert.deepEqual(flatten(bytes[0]), spans([[78, 82], [77, 89], [70, 89], [53, 96], [53, 196], [51, 213], [40, 213], [27, 214], [25, 231], [12, 231]]));
    assert.deepEqual(flatten(utf16[3]), spans([[87, 90], [87, 91], [49, 92], [49, 192], [47, 209], [36, 209], [23, 210], [21, 227], [8, 227]]));
    assert.deepEqual(flatten(utf16[4]), spans([[96, 100], [96, 108], [49, 192], [47, 209], [36, 209], [23, 210], [21, 227], [8, 227]]));
    assert.deepEqual(flatten(utf16[7]), spans([[158, 166], [158, 167], [156, 169], [139, 169], [137, 178], [114, 178], [112, 192], [49, 192], [47, 209], [36, 209], [23, 210], [21, 227], [8, 227]]));
    assert.deepEqual(flatten(utf16[8]), spans([[170, 175], [170, 176], [137, 178], [114, 178], [112, 192], [49, 192], [47, 209], [36, 209], [23, 210], [21, 227], [8, 227]]));
    assert.deepEqual(flatten(utf16[11]), spans([[192, 193], [47, 209], [36, 209], [23, 210], [21, 227], [8, 227]]));
    assert.deepEqual(flatten(utf16[14]), [[line + 1, 0, line + 1, 0]]);
    assert.equal(doc.markers.file.start.byte, 0); assert.equal(doc.markers.file.end.byte, Buffer.byteLength(doc.text));
    assert.equal(doc.text.includes("\r\n"), crlf);
    const helper = oracle.document(spec.files["scripts/helper.vela"], crlf, shifted);
    assert.deepEqual(oracle.expected(helper, spec.oracle.helperQueries).map(flatten), [
      spans([[62, 67], [48, 71], [48, 97], [46, 100], [34, 100], [27, 101], [25, 103], [8, 103]]),
      spans([[75, 80], [75, 88], [48, 97], [46, 100], [34, 100], [27, 101], [25, 103], [8, 103]])]);
    assert.deepEqual(flatten(oracle.expected(helper, spec.oracle.helperQueries, false)[0]),
      spans([[66, 71], [52, 75], [52, 101], [50, 104], [38, 104], [31, 105], [29, 107], [12, 107]]));
  }
});

test("selection pattern parents contain whole children without phantom owners or duplicate spans", () => {
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
