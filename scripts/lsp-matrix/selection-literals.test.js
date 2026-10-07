"use strict";
const assert = require("node:assert/strict"), test = require("node:test");
const spec = require("../../tests/lsp_matrix/fixtures/selection-literals.json");
const oracle = require("./selection-literal-oracle"), cases = spec.oracle.cases;
const flatten = row => {
  const result = [];
  for (; row; row = row.parent) {
    const r = row.range; result.push([r.start.line, r.start.character, r.end.line, r.end.character]);
  }
  return result;
};

test("selection literals own every operator collection interpolation and true point empty partition", () => {
  assert.equal(cases.length, 117); assert.equal(new Set(cases.map(c => c.id)).size, 117);
  const queries = cases.flatMap(c => c.queries);
  assert.equal(queries.length, 244); assert.equal(queries.filter(q => q.chain.length).length, 230);
  assert.equal(queries.filter(q => !q.chain.length).length, 14);
  assert.equal(queries.filter(q => q.token).length, 5);
  assert.equal(cases.filter(c => c.queries.some(q => q.chain.length)).length, 112);
  assert.equal(cases.filter(c => c.queries.length && c.queries.every(q => !q.chain.length)).length, 4);
  assert.equal(cases.filter(c => !c.queries.length).length, 1);
  assert.deepEqual(Object.keys(spec.files), ["scripts/main.vela", "scripts/helper.vela"]);
  assert.equal(spec.files[spec.oracle.file], cases[0].source);
  for (const id of ["unicode-interpolation-index-operator-and-named-call-owners", "nested-unary-operators",
    "arithmetic-precedence", "left-associative-subtraction", "right-associative-assignment", "logical-and-before-or",
    "unary-binary-precedence", "range-additive-precedence", "parenthesized-precedence", "unit-delimiters",
    "parenthesized-expression", "tuple-elements-and-delimiters", "tuple-trailing-comma", "nested-tuple-and-array",
    "empty-array-delimiters", "array-elements-and-punctuation", "nested-array-literals", "array-trailing-comment-and-comma",
    "multiline-array", "array-as-indexed-receiver", "empty-braced-block", "map-key-value-colon-and-trivia",
    "map-logical-bare-key", "nested-map-and-array", "map-trailing-comma-and-comment", "multiline-map-entries",
    "set-from-array-builtin", "record-literal-explicit-and-shorthand", "record-literal-empty", "record-nested-map-value",
    "record-trailing-comma-and-comment", "indexed-binary-expression", "try-indexed-receiver", "unary-method-call",
    "await-try-operators", "path-separators-and-member-dot", "same-spelled-neighbor-literals"])
    assert(cases.some(c => c.id === id), id);
  for (const kind of ["integer-zero", "integer-decimal", "integer-underscores", "integer-hex", "integer-binary",
    "float-decimal", "float-positive-exponent", "float-negative-exponent", "float-underscored-integer",
    "boolean-true", "boolean-false", "char-ascii", "char-unicode", "char-non-bmp", "char-newline", "char-unicode-escape",
    "string-empty", "string-unicode", "string-escapes", "string-unicode-escape", "string-multiline",
    "bytes-empty", "bytes-ascii", "bytes-escapes", "interpolated-no-expressions"])
    assert(cases.some(c => c.id === "literal-" + kind));
  for (const kind of ["not", "negate"]) assert(cases.some(c => c.id === "unary-" + kind));
  for (const suffix of ["i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64"])
    assert(cases.some(c => c.id === "literal-integer-suffix-" + suffix));
  for (const suffix of ["f32", "f64"]) assert(cases.some(c => c.id === "literal-float-suffix-" + suffix));
  for (const kind of ["integer", "float", "character", "boolean", "qualified-path"])
    assert(cases.some(c => c.id === "map-" + kind + "-key"));
  for (const op of ["add", "subtract", "multiply", "divide", "remainder", "less", "less-equal", "greater",
    "greater-equal", "equal", "not-equal", "strict-equal", "strict-not-equal", "and", "or", "range", "range-inclusive"])
    assert(cases.some(c => c.id === "binary-" + op));
  for (const op of ["set", "add", "subtract", "multiply", "divide", "remainder"])
    assert(cases.some(c => c.id === "assignment-" + op));
  for (const kind of ["single-value", "escaped-braces-and-text", "unary-and-binary", "array-and-map", "nested-string",
    "multiline-with-expressions", "inner-comment", "escaped-unicode", "lambda-call", "assignment"])
    assert(cases.some(c => c.id === "interpolation-" + kind));
  assert(oracle.document(cases.find(c => c.id === "set-from-array-builtin").source).text.includes("set::from_array([ seed, true])"));
  for (const c of cases) {
    const doc = oracle.document(c.source);
    for (const q of c.queries) { assert(doc.markers[q.position]); for (const name of q.chain) assert(doc.markers[name]); }
    oracle.expected(doc, c.queries);
  }
  const bracket = cases[0].queries[11], doc = oracle.document(cases[0].source);
  assert.throws(() => oracle.expected(doc, [{ ...bracket, token: "]" }]));
  assert.throws(() => oracle.expected(doc, [{ ...bracket, chain: [] }]));
  assert.throws(() => oracle.expected(doc, [{ ...bracket, position: "member-cursor" }]));
});

test("selection literal goldens pin UTF16 byte interpolation chunks ASCII bracket and independent helper chains", () => {
  for (const crlf of [false, true]) for (const shifted of [false, true]) {
    const c = cases[0], doc = oracle.document(c.source, crlf, shifted), line = shifted ? 2 : 0;
    const positions = oracle.positions(doc, c.queries), utf16 = oracle.expected(doc, c.queries), bytes = oracle.expected(doc, c.queries, false);
    const spans = ranges => ranges.map(([start, end]) => [line, start, line, end]).concat([[0, 0, line + 1, 0]]);
    assert.deepEqual(positions[0], positions[2]); assert.deepEqual(utf16[0], utf16[2]); assert.equal(utf16.length, 14);
    assert.deepEqual(flatten(utf16[0]), spans([[46, 52], [42, 52], [42, 59], [42, 66], [41, 67], [35, 99], [22, 100], [20, 117], [8, 117]]));
    assert.deepEqual(flatten(bytes[0]), spans([[54, 60], [50, 60], [50, 67], [50, 74], [49, 75], [39, 107], [26, 108], [24, 125], [12, 125]]));
    assert.deepEqual(flatten(utf16[1]), spans([[85, 89], [85, 96], [84, 97], [71, 97], [70, 98], [35, 99], [22, 100], [20, 117], [8, 117]]));
    assert.deepEqual(flatten(utf16[4]), spans([[35, 41], [35, 99], [22, 100], [20, 117], [8, 117]]));
    assert.deepEqual(flatten(bytes[4]), spans([[39, 49], [39, 107], [26, 108], [24, 125], [12, 125]]));
    assert.deepEqual(flatten(utf16[5]), spans([[67, 70], [35, 99], [22, 100], [20, 117], [8, 117]]));
    assert.deepEqual(flatten(utf16[7]), spans([[41, 42], [41, 67], [35, 99], [22, 100], [20, 117], [8, 117]]));
    assert.deepEqual(flatten(utf16[8]), spans([[66, 67], [41, 67], [35, 99], [22, 100], [20, 117], [8, 117]]));
    assert.deepEqual(flatten(utf16[11]), spans([[52, 53], [42, 59], [42, 66], [41, 67], [35, 99], [22, 100], [20, 117], [8, 117]]));
    assert.deepEqual(flatten(bytes[11]), spans([[60, 61], [50, 67], [50, 74], [49, 75], [39, 107], [26, 108], [24, 125], [12, 125]]));
    assert.deepEqual(flatten(utf16[12]), spans([[108, 114], [101, 115], [20, 117], [8, 117]]));
    assert.deepEqual(flatten(utf16[13]), [[line + 1, 0, line + 1, 0]]);
    assert.equal(doc.markers.file.start.byte, 0); assert.equal(doc.markers.file.end.byte, Buffer.byteLength(doc.text));
    assert.equal(doc.text.includes("\r\n"), crlf);
    const helper = oracle.document(spec.files["scripts/helper.vela"], crlf, shifted);
    assert.deepEqual(oracle.expected(helper, spec.oracle.helperQueries).map(flatten), [
      spans([[43, 48], [39, 48], [39, 52], [38, 53], [32, 54], [25, 55], [23, 57], [8, 57]]),
      spans([[32, 38], [32, 54], [25, 55], [23, 57], [8, 57]])]);
    assert.deepEqual(flatten(oracle.expected(helper, spec.oracle.helperQueries, false)[0]),
      spans([[51, 56], [47, 56], [47, 60], [46, 61], [36, 62], [29, 63], [27, 65], [12, 65]]));
  }
});

test("selection literal parents contain whole children without phantom owners or duplicate spans", () => {
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
