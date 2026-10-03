"use strict";
const test = require("node:test"), assert = require("node:assert/strict");
const spec = require("../../tests/lsp_matrix/fixtures/workspace-symbol-recovery.json");
const previous = require("../../tests/lsp_matrix/fixtures/document-symbol-recovery.json");
const { parseMarkers } = require("./fixtures");

test("workspace recovery reuses authored damage partitions with complete global ownership and schema collisions", () => {
  assert.equal(spec.oracle.cases.length, 56);
  assert.deepEqual(spec.oracle.cases.map(row => row.id), previous.oracle.cases.map(row => row.id));
  assert.equal(spec.oracle.symbols.length, 9);
  for (const current of [spec.oracle, ...spec.oracle.cases]) {
    assert.equal(current.queries.length, 19);
    assert.equal(new Set(current.symbols.map(row => row.id)).size, current.symbols.length);
    const schema = current.symbols.filter(row => row.ownership === "Schema");
    assert.deepEqual(schema.map(row => [row.name, row.kind, row.detail, row.container]), [["main::Broken", "Function", "Function() -> i64", null]]);
    assert.equal(current.symbols.filter(row => row.kind === "File").length, 2);
    assert.equal(current.symbols.filter(row => row.kind === "Module").length, 2);
    assert.deepEqual(current.queries.find(row => row.id === "all").symbols, current.symbols.map(row => row.id));
    assert.deepEqual(current.queries.find(row => row.id === "trim").symbols, current.queries[0].symbols);
    for (const query of current.queries) {
      assert.equal(new Set(query.symbols).size, query.symbols.length);
      assert(query.symbols.every(id => current.symbols.some(row => row.id === id)));
    }
    for (const id of ["missing-name", "local", "member", "constructor", "builtin", "unicode", "no-match"]) {
      assert.deepEqual(current.queries.find(query => query.id === id).symbols, [], id);
    }
  }
  for (const id of ["impl-no-target", "trait-impl-missing-target"]) {
    const current = spec.oracle.cases.find(row => row.id === id);
    assert(current.parseError);
    assert.equal(current.symbols.filter(row => row.kind === "Object").length, 0);
    assert.deepEqual(current.queries.find(row => row.id === "missing-impl").symbols, []);
  }
  for (const id of ["impl-no-method-name", "impl-unclosed", "trait-impl-missing-default"]) {
    assert.equal(spec.oracle.cases.find(row => row.id === id).symbols.filter(row => row.kind === "Object").length, 1);
  }
});

test("workspace recovery marker goldens retain full CST EOF extents and original source bytes", () => {
  for (const crlf of [false, true]) for (const shifted of [false, true]) {
    const transform = source => {
      if (shifted) source = source.startsWith("[[file:start]]#!")
        ? source.replace("\n", "\n// shifted 中😀\n/* extra 😀 */\n")
        : source.replace("[[file:start]]", "[[file:start]]// shifted 中😀\n/* extra 😀 */\n");
      return crlf ? source.replaceAll("\n", "\r\n") : source;
    };
    const base = parseMarkers(transform(spec.files["scripts/main.vela"]));
    const before = base.markers["before-name"];
    assert.deepEqual([before.start.line, before.start.character, before.end.character], [shifted ? 2 : 0, 13, 19]);
    assert.equal(before.start.byte - Buffer.from(base.text).lastIndexOf(10, before.start.byte - 1) - 1, 17);
    for (const current of spec.oracle.cases) {
      const doc = parseMarkers(transform(current.source));
      assert.deepEqual([doc.markers.file.start.byte, doc.markers.file.end.byte], [0, Buffer.byteLength(doc.text)]);
      const original = parseMarkers(previous.oracle.cases.find(row => row.id === current.id).source);
      assert.equal(parseMarkers(current.source).text, original.text);
      for (const row of current.symbols.filter(row => row.file === "scripts/main.vela")) assert(doc.markers[row.range]);
      if (doc.markers["workspace-eof"]) {
        assert.equal(doc.markers["workspace-eof"].end.byte, Buffer.byteLength(doc.text));
        assert.equal(doc.markers["workspace-eof"].end.character, 0);
      }
    }
  }
});
