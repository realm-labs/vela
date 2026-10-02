"use strict";
const test = require("node:test");
const assert = require("node:assert/strict");
const spec = require("../../tests/lsp_matrix/fixtures/workspace-symbol-ownership.json");
const { parseMarkers } = require("./fixtures");
const { symbols, queries, schema } = spec.oracle;
const ids = query => queries.find(row => row.id === query).symbols;

test("workspace symbol ownership corpus pins complete source and static schema partitions", () => {
  assert.equal(Object.keys(spec.files).filter(file => file.endsWith(".vela")).length, 8);
  assert.equal(symbols.length, 60); assert.equal(new Set(symbols.map(row => row.id)).size, 60);
  assert.equal(symbols.filter(row => row.ownership === "Source").length, 41);
  assert.equal(symbols.filter(row => row.ownership === "Schema").length, 19);
  const counts = {};
  for (const row of symbols) {
    counts[row.kind] = (counts[row.kind] ?? 0) + 1;
    if (row.ownership === "Source") {
      const doc = parseMarkers(spec.files[row.file]), marker = doc.markers[row.range];
      assert(marker && marker.start.byte < marker.end.byte);
      if (["File", "Module"].includes(row.kind)) assert.deepEqual([marker.start.byte, marker.end.byte], [0, Buffer.byteLength(doc.text)]);
    } else {
      assert.equal(row.ownership, "Schema"); assert.equal(row.file, undefined); assert.equal(row.range, undefined);
    }
  }
  assert.deepEqual(counts, { Struct: 6, Function: 9, Module: 8, File: 8, Class: 3, Field: 5, Method: 4, Enum: 3, EnumMember: 3, Interface: 3, Object: 5, Constant: 1, Variable: 2 });
  assert.deepEqual(Object.fromEntries(Object.entries(schema).map(([key, rows]) => [key, rows.length])),
    { types: 4, traits: 1, functions: 2, fields: 5, methods: 3, traitMethods: 1, variants: 3 });
  assert.equal(Object.values(schema).flat().filter(row => row.sourceSpan).length, 10);
});

test("workspace symbol ownership queries pin collisions, member kinds, imports and exact negative sets", () => {
  assert.equal(queries.length, 49); assert.equal(new Set(queries.map(row => row.id)).size, 49);
  assert.deepEqual(ids("all"), symbols.map(row => row.id)); assert.deepEqual(ids("trim-empty"), ids("all"));
  assert.equal(ids("source-root").length, 14);
  assert(!ids("source-root").includes("source-module"), "source has no trailing :: in its module name");
  assert.deepEqual(ids("same-type"), ["schema-source-widget", "source-widget", "schema-source-widget-read", "schema-source-widget-value"]);
  assert.deepEqual(ids("same-function"), ["source-make", "schema-source-make"]);
  assert.equal(ids("all-widget").length, 9);
  assert(!ids("all-widget").includes("main-imported-impl"), "Row keeps its declared name, not its imported target's name");
  assert.deepEqual(ids("imported-alias"), ["main-trait-imported-impl", "main-imported-impl"]);
  assert.deepEqual(ids("imports-only"), ["imports-module", "imports-file"]);
  assert.deepEqual(ids("private-original"), ["source-private"]);
  assert.deepEqual(ids("source-field-schema-collision"), ["schema-source-widget-value"]);
  assert.deepEqual(ids("source-method-schema-collision"), ["schema-source-widget-read"]);
  assert.equal(ids("fields").length, 5); assert.equal(ids("methods").length, 6);
  assert.equal(ids("host").length, 12); assert.equal(ids("metadata").length, 3);
  assert.equal(queries.find(row => row.id === "source-method-body-name").query, "preview");
  assert.deepEqual(ids("unit-variant"), ["schema-host-choice-empty"]);
  assert.deepEqual(ids("tuple-variant"), ["schema-host-choice-pair", "schema-pair-value"]);
  assert.deepEqual(ids("record-variant"), ["schema-host-choice-named", "schema-named-value"]);
  for (const query of queries) {
    assert.equal(new Set(query.symbols).size, query.symbols.length);
    assert(query.symbols.every(id => symbols.some(symbol => symbol.id === id)));
  }
  for (const id of ["source-enum-members-not-global", "source-trait-members-not-global", "private-import-alias", "unresolved-import-alias", "duplicate-import-alias", "missing-type", "missing-member", "unknown-owner", "dynamic-owner", "module-alias", "source-module-alias", "stdlib-module-alias", "stdlib-function-alias", "stdlib-function", "stdlib-module", "constructor-use", "field-label-use", "source-added-method", "scripted-host-method", "source-method-body-name", "detail-not-name", "schema-signature-not-name", "source-call-alias", "schema-call-alias", "variant-alias", "tuple-alias", "unicode-trivia", "no-match"]) assert.deepEqual(ids(id), [], id);
});

test("workspace symbol ownership source ranges and static origins retain independent Unicode goldens", () => {
  for (const crlf of [false, true]) for (const shifted of [false, true]) {
    const docs = {};
    for (const [file, raw] of Object.entries(spec.files).filter(([file]) => file.endsWith(".vela"))) {
      let source = shifted ? raw.replace("[[file:start]]", "[[file:start]]// shifted 中😀\n/* extra 😀 */\n") : raw;
      if (crlf) source = source.replaceAll("\n", "\r\n");
      const doc = docs[file] = parseMarkers(source);
      assert.deepEqual([doc.markers.file.start.byte, doc.markers.file.end.byte], [0, Buffer.byteLength(doc.text)]);
    }
    const doc = docs["scripts/source.vela"], p = doc.markers["widget-range"].start;
    assert.deepEqual([p.line, p.character], [shifted ? 2 : 0, 10]);
    assert.equal(p.byte - Buffer.from(doc.text).lastIndexOf(10, p.byte - 1) - 1, 14);
    for (const entry of Object.values(schema).flat().filter(row => row.sourceSpan)) {
      const { file, marker } = entry.sourceSpan, range = docs[file].markers[marker];
      assert(range && range.start.byte < range.end.byte);
    }
    assert.equal(schema.types.find(row => row.name === "metadata::Box").sourceSpan, undefined);
    assert.equal(symbols.find(row => row.id === "schema-host-box").file, undefined, "known metadata source span never changes workspace location ownership");
  }
});
