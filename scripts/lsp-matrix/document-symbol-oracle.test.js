"use strict";
const test = require("node:test");
const assert = require("node:assert/strict");
const oracle = require("./document-symbol-oracle");
const { FixtureWorkspace } = require("./fixtures");
const specs = Object.fromEntries(["declarations", "ownership", "recovery", "lifecycle"].map(name =>
  [name, require(`../../tests/lsp_matrix/fixtures/document-symbol-${name}.json`)]));
const flatten = rows => rows.flatMap(row => [row, ...flatten(row.children)]);

test("installed symbol oracle pins the authored 19-root 33-node declaration tree and literal UTF-16 selection", () => {
  const spec = specs.declarations, doc = oracle.document(spec.files[spec.oracle.file]);
  const tree = oracle.expected(doc, spec.oracle.symbols);
  assert.equal(tree.length, 19); assert.equal(oracle.count(tree), 33); oracle.assertAncestry(tree);
  const required = flatten(tree).find(row => row.selectionRange[0] === 27);
  assert.deepEqual({ name: required.name, kind: required.kind, selection: required.selectionRange },
    { name: "read", kind: 5, selection: [27, 39, 27, 43] });
  assert.equal(doc.markers["required-name"].start.byte - Buffer.from(doc.text).lastIndexOf(10, doc.markers["required-name"].start.byte - 1) - 1, 43);
  assert(tree.every(row => Array.isArray(row.children) && typeof row.detail === "string"));
});

test("installed symbol oracle pins all eight ownership trees with exact empties and the independent Widget golden", () => {
  const spec = specs.ownership;
  const trees = Object.entries(spec.oracle.trees).map(([file, rows]) => {
    const tree = oracle.expected(oracle.document(spec.files[file]), rows); oracle.assertAncestry(tree); return tree;
  });
  assert.equal(trees.length, 8); assert.equal(trees.reduce((n, tree) => n + tree.length, 0), 25);
  assert.equal(trees.reduce((n, tree) => n + oracle.count(tree), 0), 46);
  assert.deepEqual(spec.oracle.trees["scripts/imports.vela"], []);
  const doc = oracle.document(spec.files["scripts/source.vela"]), tree = oracle.expected(doc, spec.oracle.trees["scripts/source.vela"]);
  assert.deepEqual({ name: tree[0].name, kind: tree[0].kind, selection: tree[0].selectionRange },
    { name: "Widget", kind: 22, selection: [0, 21, 0, 27] });
  assert.deepEqual([doc.markers["widget-name"].start.byte, doc.markers["widget-name"].end.byte], [25, 31]);
});

test("installed symbol whole-tree LF and CRLF oracles preserve logical ranges after Unicode line shifts", () => {
  for (const spec of [specs.declarations, specs.ownership]) {
    const trees = spec.oracle.trees ?? { [spec.oracle.file]: spec.oracle.symbols };
    for (const [file, rows] of Object.entries(trees)) {
      const lf = oracle.expected(oracle.document(spec.files[file]), rows);
      assert.deepEqual(oracle.expected(oracle.document(spec.files[file], true), rows), lf);
      const shifted = oracle.expected(oracle.document("// 中😀\n/* 😀 */\n" + spec.files[file], true), rows);
      const baseNodes = flatten(lf), shiftedNodes = flatten(shifted);
      assert.equal(shiftedNodes.length, baseNodes.length);
      for (const [index, row] of shiftedNodes.entries()) for (const key of ["range", "selectionRange"]) {
        const [a, b, c, d] = baseNodes[index][key]; assert.deepEqual(row[key], [a + 2, b, c + 2, d]);
      }
    }
  }
});

test("installed recovery oracle covers all 56 authored damage partitions and literal Unicode selections", () => {
  const spec = specs.recovery; assert.equal(spec.oracle.cases.length, 56);
  for (const crlf of [false, true]) for (const item of spec.oracle.cases) {
    const doc = oracle.document("// 中😀\n" + item.source, crlf), tree = oracle.expected(doc, item.symbols);
    oracle.assertAncestry(tree);
    if (["empty-file", "trivia-only"].includes(item.id)) assert.deepEqual(tree, []);
    if (item.id === "const-no-name") assert.deepEqual(tree[0].selectionRange, [1, 13, 1, 19]);
  }
});

test("installed source lifecycle filters only schema actions and retains independent disk overlay phase maps", () => {
  const spec = specs.lifecycle;
  const phases = spec.oracle.phases.filter(phase => !phase.schemaAction); assert.equal(phases.length, 13);
  for (const crlf of [false, true]) {
    const copy = structuredClone(spec);
    for (const [file, text] of Object.entries(copy.files)) if (crlf) copy.files[file] = text.replaceAll("\n", "\r\n");
    const fixture = new FixtureWorkspace(copy);
    for (const phase of phases) {
      for (const a of phase.actions) fixture.apply({ op: a.op, file: a.file,
        source: a.variant && (crlf ? spec.oracle.variants[a.variant].source.replaceAll("\n", "\r\n") : spec.oracle.variants[a.variant].source) });
      for (const [file, id] of Object.entries(phase.views)) {
        const wanted = id && oracle.document(spec.oracle.variants[id].source, crlf);
        assert.deepEqual(fixture.document(file), wanted ?? undefined, `${phase.id}/${file}`);
        const disk = phase.disk[file]; assert.deepEqual(fixture.disk.get(file), disk ? oracle.document(spec.oracle.variants[disk].source, crlf) : undefined);
        const open = phase.open[file]; assert.deepEqual(fixture.open.get(file), open ? oracle.document(spec.oracle.variants[open].source, crlf) : undefined);
        if (id) oracle.assertAncestry(oracle.expected(wanted, spec.oracle.variants[id].symbols));
      }
    }
    const doc = oracle.document(spec.oracle.variants["main-base"].source, crlf);
    assert.deepEqual([doc.markers["main-fn-name"].start.line, doc.markers["main-fn-name"].start.character, doc.markers["main-fn-name"].end.character], [1, 13, 16]);
  }
});

test("installed metadata setup keeps known string facts and leaves the source-backed schema oracle intact", () => {
  const original = structuredClone(specs.ownership.oracle.schema), artifact = oracle.metadataArtifact(original);
  assert.equal(artifact.formatVersion, 1);
  assert.deepEqual(Object.fromEntries(Object.entries(artifact.facts).map(([key, values]) => [key, values.length])),
    { types: 4, traits: 1, functions: 2, fields: 5, methods: 3, traitMethods: 1, variants: 3 });
  assert(artifact.facts.fields.every(field => field.fact.kind === "primitive" && field.fact.name === "string"));
  assert(Object.values(artifact.facts).flat().every(entry => !Object.hasOwn(entry, "sourceSpan")));
  assert(Object.values(original).flat().some(entry => Object.hasOwn(entry, "sourceSpan")));
  assert.deepEqual(original, specs.ownership.oracle.schema, "authored source-backed metadata never mutated");
});

test("installed symbol ancestry refuses empty name selections and escaped child extents", () => {
  const spec = specs.declarations;
  const tree = oracle.expected(oracle.document(spec.files[spec.oracle.file]), spec.oracle.symbols);
  const empty = structuredClone(tree); empty[0].selectionRange[3] = empty[0].selectionRange[1];
  assert.throws(() => oracle.assertAncestry(empty));
  const escaped = structuredClone(tree), parent = escaped.find(row => row.children.length);
  parent.children[0].range[0] = parent.range[0] - 1;
  assert.throws(() => oracle.assertAncestry(escaped));
});
