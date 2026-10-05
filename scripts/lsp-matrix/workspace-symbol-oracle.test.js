"use strict";
const assert = require("node:assert/strict");
const test = require("node:test");
const oracle = require("./workspace-symbol-oracle");
const specs = ["declarations", "ownership"].map(name => require(`../../tests/lsp_matrix/fixtures/workspace-symbol-${name}.json`));
const uri = file => `file:///Chinese%20%25/${file}`;
const docs = (spec, crlf, shift = false) => Object.fromEntries(Object.entries(spec.files)
  .filter(([file]) => file.endsWith(".vela")).map(([file, source]) =>
    [file, oracle.document(shift ? source.replace("[[file:start]]", "[[file:start]]// 中😀\n/* 😀 */\n") : source, crlf)]));

test("installed workspace oracle pins complete declaration and schema/source rows including real empty query sets", () => {
  assert.deepEqual(specs.map(spec => spec.oracle.symbols.length), [31, 60]);
  assert.deepEqual(specs.map(spec => spec.oracle.queries.length), [32, 49]);
  for (const spec of specs) for (const query of spec.oracle.queries) {
    const wire = oracle.expected(spec, docs(spec, false), uri, query);
    assert.equal(wire.length, query.symbols.length);
    assert.deepEqual(wire.map(row => row.name), query.symbols.map(id => spec.oracle.symbols.find(row => row.id === id).name));
    assert.deepEqual(oracle.editorExpected(wire).map(row => row.kind), wire.map(row => row.kind - 1));
    if (query.symbols.length === 0) assert.deepEqual(wire, []);
  }
});

test("installed workspace oracle pins independent Unicode UTF16 declaration extents and all dirty file boundaries", () => {
  const spec = specs[0], all = spec.oracle.queries[0];
  const lf = oracle.expected(spec, docs(spec, false), uri, all);
  assert.deepEqual(oracle.expected(spec, docs(spec, true), uri, all), lf);
  const row = lf.find(row => row.name === "api::LIMIT");
  assert.deepEqual(row, { name: "api::LIMIT", kind: 14, containerName: "api",
    location: { uri: uri("scripts/api.vela"), range: { start: { line: 1, character: 8 }, end: { line: 1, character: 33 } } },
    data: { detail: "i64" } });
  assert.equal(docs(spec, false)["scripts/api.vela"].markers["limit-range"].start.byte -
    Buffer.from(docs(spec, false)["scripts/api.vela"].text).lastIndexOf(10, docs(spec, false)["scripts/api.vela"].markers["limit-range"].start.byte - 1) - 1, 12);
  for (const spec of specs) for (const crlf of [false, true]) {
    const all = spec.oracle.queries[0], base = oracle.expected(spec, docs(spec, crlf), uri, all);
    const shifted = oracle.expected(spec, docs(spec, crlf, true), uri, all);
    for (const [i, row] of base.entries()) {
      if (!row.location.range) { assert.deepEqual(shifted[i], row); continue; }
      const wanted = structuredClone(row);
      // Whole-file ranges begin before the inserted prefix; declarations shift.
      if (spec.oracle.symbols[i].range !== "file") wanted.location.range.start.line += 2;
      wanted.location.range.end.line += 2; assert.deepEqual(shifted[i], wanted);
    }
  }
});

test("installed metadata-only workspace rows keep duplicate names distinct without fabricating source locations", () => {
  const spec = specs[1], source = structuredClone(spec.oracle.schema);
  const artifact = oracle.metadataArtifact(source);
  assert(Object.values(artifact.facts).flat().every(row => !Object.hasOwn(row, "sourceSpan")));
  assert.deepEqual(source, spec.oracle.schema);
  const query = spec.oracle.queries.find(row => row.id === "same-function");
  const wire = oracle.expected(spec, docs(spec, false), uri, query);
  assert.deepEqual(wire.map(row => row.name), ["source::make", "source::make"]);
  assert.deepEqual(wire[1], { name: "source::make", kind: 12, location: { uri: "vela-schema:" }, data: { detail: "Function() -> String" } });
  const editor = oracle.editorExpected(wire);
  assert.equal(editor[0].uri, uri("scripts/source.vela"));
  assert.deepEqual(editor[1], { name: "source::make", kind: 11, containerName: "", uri: "vela-schema:", range: [0, 0, 0, 0] });
  assert.equal(oracle.ordered([...editor, editor[1]]).length, 3, "complete multiset never deduplicates");
  assert.deepEqual(oracle.ordered([...editor].reverse()), oracle.ordered(editor));
});

test("workspace wire URI normalization preserves raw data every field order and empty versus absent provider", () => {
  const rows = [{ name: "owned", kind: 12, location: { uri: "file:///F:/Chinese%20%25/main.vela", range: { start: { line: 1, character: 15 }, end: { line: 1, character: 19 } } },
    data: { detail: "()" }, extra: "must still fail whole-row assertion" }, { name: "metadata", kind: 5, location: { uri: "vela-schema:" } }];
  const original = structuredClone(rows);
  const normalized = oracle.normalizeWire(rows, text => text === rows[0].location.uri ? "file:///f%3A/Chinese%20%25/main.vela" : text);
  assert.deepEqual(rows, original);
  const wanted = structuredClone(rows); wanted[0].location.uri = "file:///f%3A/Chinese%20%25/main.vela";
  assert.deepEqual(normalized, wanted); assert.deepEqual(oracle.normalizeWire([], String), []);
  for (const value of [null, undefined, { error: "failed" }]) assert.throws(() => oracle.normalizeWire(value, String), /explicit complete array/);
});
