"use strict";
const test = require("node:test"), assert = require("node:assert/strict");
const spec = require("../../tests/lsp_matrix/fixtures/workspace-symbol-type-positions.json");
const { parseMarkers } = require("./fixtures");
const { cases, symbols, queries, schema } = spec.oracle;
const ids = id => queries.find(query => query.id === id).symbols;

test("workspace symbol type corpus pins every primitive and structured hint with whole source and schema rows", () => {
  assert.equal(cases.length, 47); assert.equal(symbols.length, 78);
  assert.equal(new Set(cases.map(row => row.id)).size, 47); assert.equal(new Set(symbols.map(row => row.id)).size, 78);
  assert.equal(symbols.filter(row => row.ownership === "Source").length, 61);
  assert.equal(symbols.filter(row => row.ownership === "Schema").length, 17);
  const counts = {};
  for (const row of symbols) counts[row.kind] = (counts[row.kind] ?? 0) + 1;
  assert.deepEqual(counts, { File: 3, Module: 3, Struct: 3, Enum: 2, Interface: 2, Constant: 1, Variable: 2, Function: 55, Class: 3, Field: 1, Method: 2, EnumMember: 1 });
  for (const row of cases) {
    const symbol = symbols.find(symbol => symbol.id === row.id);
    assert.equal(symbol.name, "types::accept_" + row.id.replaceAll("-", "_") + "_case");
    assert.equal(symbol.detail, "(value: " + row.display + ") -> " + row.display);
    assert(spec.files["scripts/types.vela"].includes("value: " + row.raw));
  }
  assert.deepEqual(cases.slice(0, 16).map(row => row.display), ["()", "bool", "char", "i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64", "f32", "f64", "String", "Bytes", "Any"]);
  assert.equal(cases.find(row => row.id === "nested").display, "Map<String, Array<Option<Result<models::Row, Bytes>>>>");
  assert.deepEqual(Object.fromEntries(Object.entries(schema).map(([key, rows]) => [key, rows.length])), { types: 6, traits: 1, functions: 6, fields: 1, methods: 1, traitMethods: 1, variants: 1 });
  assert.equal(symbols.find(row => row.id === "schema-function-nested-unknown").detail, "Function() -> Array(Option(unknown))");
});

test("workspace symbol type queries pin exact hint matches and exclude local builtin dynamic and unknown inventions", () => {
  assert.equal(queries.length, 80); assert.equal(new Set(queries.map(row => row.id)).size, 80);
  assert.deepEqual(ids("all"), symbols.map(row => row.id)); assert.deepEqual(ids("trim-empty"), ids("all"));
  for (const row of cases) assert.deepEqual(ids("function-" + row.id), [row.id]);
  assert.deepEqual(ids("source-schema-collision"), ["schema-collision", "model-row"]);
  assert.deepEqual(ids("imports-only"), ["imports-module", "imports-file"]);
  assert.deepEqual(ids("dynamic-hint"), ["any"]); assert.deepEqual(ids("primitive-hint"), ["i64"]);
  assert.deepEqual(ids("parameter"), ["schema-values"]);
  assert.deepEqual(ids("metadata-functions"), ["schema-function-unknown", "schema-function-nested-unknown"]);
  for (const query of queries) {
    assert.equal(new Set(query.symbols).size, query.symbols.length);
    assert(query.symbols.every(id => symbols.some(row => row.id === id)));
  }
  for (const id of ["source-alias", "schema-alias", "trait-alias", "unknown-hint", "builtin-hint", "typed-container-hint", "local", "capture", "callback", "unknown-owner", "builtin-function", "builtin-variant", "constructor-field", "source-trait-member", "builtin-import-alias", "missing-import-alias", "unicode-trivia", "no-match"]) assert.deepEqual(ids(id), [], id);
});

test("workspace symbol type marker ranges remain complete under Unicode prefixes and LF CRLF", () => {
  for (const crlf of [false, true]) for (const shifted of [false, true]) {
    const docs = {};
    for (const [file, raw] of Object.entries(spec.files).filter(([file]) => file.endsWith(".vela"))) {
      let text = shifted ? raw.replace("[[file:start]]", "[[file:start]]// shifted 中😀\n/* extra 😀 */\n") : raw;
      if (crlf) text = text.replaceAll("\n", "\r\n");
      const doc = docs[file] = parseMarkers(text);
      assert.deepEqual([doc.markers.file.start.byte, doc.markers.file.end.byte], [0, Buffer.byteLength(doc.text)]);
    }
    for (const row of symbols.filter(row => row.ownership === "Source")) {
      const range = docs[row.file].markers[row.range]; assert(range && range.start.byte < range.end.byte);
    }
    const doc = docs["scripts/types.vela"], p = doc.markers["unit-range"].start;
    assert.deepEqual([p.line, p.character], [shifted ? 4 : 2, 8]);
    assert.equal(p.byte - Buffer.from(doc.text).lastIndexOf(10, p.byte - 1) - 1, 12);
    assert(symbols.filter(row => row.ownership === "Schema").every(row => row.file === undefined && row.range === undefined));
  }
});
