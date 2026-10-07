"use strict";
const assert = require("node:assert/strict"), test = require("node:test");
const spec = require("../../tests/lsp_matrix/fixtures/selection-calls.json");
const oracle = require("./selection-oracle"), cases = spec.oracle.cases;
const flatten = row => {
  const result = [];
  for (; row; row = row.parent) {
    const r = row.range; result.push([r.start.line, r.start.character, r.end.line, r.end.character]);
  }
  return result;
};

test("selection calls own arguments callbacks callable forms and true point empty policies", () => {
  assert.equal(cases.length, 83); assert.equal(new Set(cases.map(c => c.id)).size, 83);
  const queries = cases.flatMap(c => c.queries);
  assert.equal(queries.length, 248); assert.equal(queries.filter(q => q.chain.length).length, 235);
  assert.equal(queries.filter(q => !q.chain.length).length, 13);
  assert.equal(cases.filter(c => c.queries.some(q => q.chain.length)).length, 78);
  assert.equal(cases.filter(c => c.queries.length && c.queries.every(q => !q.chain.length)).length, 4);
  assert.equal(cases.filter(c => !c.queries.length).length, 1);
  assert.deepEqual(Object.keys(spec.files), ["scripts/main.vela", "scripts/helper.vela"]);
  assert.equal(spec.files[spec.oracle.file], cases[0].source);
  for (const id of ["qualified-named-callback-and-positional-owners", "empty-call-list", "one-positional-argument",
    "multiple-positional-arguments", "named-label-and-value", "reordered-named-arguments", "mixed-positional-and-named",
    "argument-trailing-trivia", "trailing-comma-and-comment", "multiline-argument-ownership", "named-label-with-comments",
    "nested-positional-call", "nested-named-call", "consecutive-returned-call", "parenthesized-callee", "indexed-callee",
    "call-then-index", "source-method-call", "host-method-call", "trait-method-call", "dynamic-method-call",
    "unknown-method-call", "stdlib-method-call", "qualified-source-function", "host-function-call", "stdlib-function-call",
    "missing-function-call", "service-base-call", "service-pinned-call", "scoped-task-call", "unknown-bare-function",
    "scoped-task-continuation-call", "method-chain-calls", "returned-receiver-method-call", "indexed-receiver-method-call",
    "parenthesized-assignment-positional", "field-assignment-positional", "positional-assignment-callback",
    "named-assignment-callback", "typed-callback-parameter", "block-callback-method", "lambda-as-callee", "awaited-call",
    "awaited-try-call", "awaited-receiver-call", "record-argument", "tuple-argument", "array-argument", "map-argument",
    "function-default-call", "required-trait-default-call", "trait-default-call", "inherent-default-call",
    "trait-impl-default-call", "source-callable-separate-declaration", "imported-callable-alias",
    "same-spelled-label-and-value", "same-spelled-neighbor-calls", "stdlib-iterator-pipeline", "unit-argument",
    "binary-positional-argument", "block-positional-argument", "conditional-positional-argument"])
    assert(cases.some(c => c.id === id), id);
  for (const op of ["add", "subtract", "multiply", "divide", "remainder"])
    assert(cases.some(c => c.id === "compound-assignment-positional-" + op));
  for (const op of ["set", "subtract", "multiply", "divide", "remainder"]) for (const kind of ["positional", "named"])
    assert(cases.some(c => c.id === kind + "-" + op + "-assignment-callback"));
  const text = id => oracle.document(cases.find(c => c.id === id).source).text;
  assert(text("imported-callable-alias").includes("use helper::apply as Alias;"));
  assert(oracle.document(spec.files["scripts/helper.vela"]).text.includes("pub fn apply(first, callback = fallback)"));
  assert(text("scoped-task-call").includes("task::spawn_scoped(worker(seed))"));
  assert(text("scoped-task-continuation-call").includes("task::spawn_scoped_then(worker(seed), continuation)"));
  assert(text("stdlib-iterator-pipeline").includes("arr.iter().map(|value| value).collect_array()"));
  for (const c of cases) {
    const doc = oracle.document(c.source);
    for (const q of c.queries) { assert(doc.markers[q.position]); for (const name of q.chain) assert(doc.markers[name]); }
  }
});

test("selection call goldens pin complete UTF16 byte label callback and independent helper chains", () => {
  for (const crlf of [false, true]) for (const shifted of [false, true]) {
    const c = cases[0], doc = oracle.document(c.source, crlf, shifted), line = shifted ? 2 : 0;
    const positions = oracle.positions(doc, c.queries), utf16 = oracle.expected(doc, c.queries), bytes = oracle.expected(doc, c.queries, false);
    const spans = ranges => ranges.map(([start, end]) => [line, start, line, end]).concat([[0, 0, line + 1, 0]]);
    assert.deepEqual(positions[0], positions[2]); assert.deepEqual(utf16[0], utf16[2]); assert.equal(utf16.length, 16);
    assert.deepEqual(flatten(utf16[0]), spans([[61, 66], [57, 66], [49, 66], [48, 101], [35, 101], [22, 102], [20, 119], [8, 119]]));
    assert.deepEqual(flatten(bytes[0]), spans([[65, 70], [61, 70], [53, 70], [52, 105], [39, 105], [26, 106], [24, 123], [12, 123]]));
    assert.deepEqual(flatten(utf16[1]), spans([[49, 54], [49, 66], [48, 101], [35, 101], [22, 102], [20, 119], [8, 119]]));
    assert.deepEqual(flatten(utf16[6]), spans([[80, 85], [79, 86], [79, 100], [68, 100], [48, 101], [35, 101], [22, 102], [20, 119], [8, 119]]));
    assert.deepEqual(flatten(utf16[7]), spans([[87, 92], [87, 100], [79, 100], [68, 100], [48, 101], [35, 101], [22, 102], [20, 119], [8, 119]]));
    assert.deepEqual(flatten(utf16[11]), spans([[66, 67], [48, 101], [35, 101], [22, 102], [20, 119], [8, 119]]));
    assert.deepEqual(flatten(utf16[14]), spans([[110, 116], [103, 117], [20, 119], [8, 119]]));
    assert.deepEqual(flatten(utf16[15]), [[line + 1, 0, line + 1, 0]]);
    assert.equal(doc.markers.file.start.byte, 0); assert.equal(doc.markers.file.end.byte, Buffer.byteLength(doc.text));
    assert.equal(doc.text.includes("\r\n"), crlf);
    const helper = oracle.document(spec.files["scripts/helper.vela"], crlf, shifted);
    assert.deepEqual(oracle.expected(helper, spec.oracle.helperQueries).map(flatten), [
      spans([[45, 49], [37, 49], [36, 50], [29, 50], [22, 51], [20, 53], [8, 53]]),
      spans([[37, 42], [37, 49], [36, 50], [29, 50], [22, 51], [20, 53], [8, 53]])]);
    assert.deepEqual(flatten(oracle.expected(helper, spec.oracle.helperQueries, false)[0]),
      spans([[49, 53], [41, 53], [40, 54], [33, 54], [26, 55], [24, 57], [12, 57]]));
  }
});

test("selection call parents contain whole children without semantic fallback or duplicate spans", () => {
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
