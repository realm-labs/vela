"use strict";
const test = require("node:test"), assert = require("node:assert/strict");
const spec = require("../../tests/lsp_matrix/fixtures/workspace-symbol-lifecycle.json");
const previous = require("../../tests/lsp_matrix/fixtures/document-symbol-lifecycle.json");
const { parseMarkers } = require("./fixtures");
const phase = id => spec.oracle.phases.find(row => row.id === id);
const ids = (phase, query) => phase.workspace.queries.find(row => row.id === query).symbols;

test("workspace lifecycle pins complete source and schema query sets across nineteen disk overlay dependency and schema phases", () => {
  assert.equal(Object.keys(spec.oracle.variants).length, 8); assert.equal(spec.oracle.phases.length, 19);
  assert.deepEqual(spec.oracle.phases.map(row => row.id), previous.oracle.phases.map(row => row.id));
  assert.equal(phase("disk").workspace.symbols.length, 16);
  for (const current of spec.oracle.phases) {
    const rows = current.workspace.symbols;
    assert.equal(current.workspace.queries.length, 27);
    assert.equal(new Set(rows.map(row => row.id)).size, rows.length);
    assert.deepEqual(ids(current, "all"), rows.map(row => row.id)); assert.deepEqual(ids(current, "trim"), ids(current, "all"));
    const files = Object.values(current.views).filter(Boolean).length;
    assert.equal(rows.filter(row => row.kind === "File").length, files);
    assert.equal(rows.filter(row => row.kind === "Module").length, files);
    const schema = rows.filter(row => row.ownership === "Schema");
    assert.equal(schema.length, current.schema.mode === "valid" ? 2 : 0);
    if (schema.length) {
      assert.equal(schema[0].name, "host::Box"); assert.equal(schema[0].kind, "Class");
      const expected = { a: ["value", "i64"], b: ["replaced", "String"], c: ["new_value", "bool"] }[current.schema.id];
      assert.deepEqual([schema[1].name, schema[1].detail, schema[1].identity], ["host::Box::" + expected[0], expected[1], "host::Box." + expected[0]]);
    }
    for (const query of current.workspace.queries) {
      assert.equal(new Set(query.symbols).size, query.symbols.length);
      assert(query.symbols.every(id => rows.some(row => row.id === id)));
    }
    for (const query of ["source-member", "local-member", "unknown", "no-match", "unicode"]) assert.deepEqual(ids(current, query), [], query);
    assert.deepEqual(ids(current, "alias-impl"), ["main-impl"]);
    const dependent = current.views["scripts/api.vela"];
    assert.deepEqual(ids(current, "api-file"), dependent ? ["api-file"] : []);
    assert.deepEqual(ids(current, "source-type"), dependent ? ["api-widget", "api-api-impl"] : []);
  }
  assert.deepEqual(ids(phase("disk-under-overlay"), "run"), []);
  assert.deepEqual(ids(phase("disk-under-overlay"), "dirty"), ["main-main-fn"]);
  assert.deepEqual(ids(phase("close-restores-disk"), "run"), ["main-main-fn"]);
  assert.deepEqual(ids(phase("close-restores-disk"), "dirty"), []);
  assert.deepEqual(ids(phase("schema-invalid"), "schema"), []);
  assert.deepEqual(ids(phase("schema-missing"), "schema"), []);
  assert.deepEqual(ids(phase("schema-restored"), "schema-new"), ["schema-new"]);
});

test("workspace lifecycle authored variant bytes and whole-file Unicode ranges remain independent under shifts and LF CRLF", () => {
  for (const crlf of [false, true]) for (const shifted of [false, true]) {
    for (const [id, variant] of Object.entries(spec.oracle.variants)) {
      let source = variant.source;
      if (shifted) source = source.replace("[[file:start]]", "[[file:start]]// shifted 中😀\n/* extra 😀 */\n");
      if (crlf) source = source.replaceAll("\n", "\r\n");
      const doc = parseMarkers(source);
      assert.deepEqual([doc.markers.file.start.byte, doc.markers.file.end.byte], [0, Buffer.byteLength(doc.text)]);
      assert.equal(parseMarkers(variant.source).text, parseMarkers(previous.oracle.variants[id].source).text);
      for (const row of variant.symbols) assert(doc.markers[row.range]);
      if (id === "main-base") {
        const name = doc.markers["main-fn-name"];
        assert.deepEqual([name.start.line, name.start.character, name.end.character], [shifted ? 3 : 1, 13, 16]);
        assert.equal(name.start.byte - Buffer.from(doc.text).lastIndexOf(10, name.start.byte - 1) - 1, 17);
      }
    }
  }
});
