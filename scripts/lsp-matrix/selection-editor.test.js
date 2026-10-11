"use strict";
const assert = require("node:assert/strict"), test = require("node:test");
const spec = require("../../tests/lsp_matrix/fixtures/selection-editor.json");
const lifecycle = require("../../tests/lsp_matrix/fixtures/selection-lifecycle.json");
const oracle = require("./selection-editor-oracle");
const { selectionTrace } = require("./selection-editor-trace");
const flatten = row => oracle.flatten(row).map(r => [r.start.line, r.start.character, r.end.line, r.end.character]);

test("installed selection corpus pins every authored family and complete vectors in all four Unicode forms", () => {
  assert.equal(spec.oracle.cases.length, 105); assert.equal(new Set(spec.oracle.cases.map(c => c.id)).size, 105);
  assert.equal(new Set(spec.oracle.cases.map(c => c.fixture)).size, 10);
  assert.equal(lifecycle.oracle.phases.length, 43); assert.equal(Object.keys(lifecycle.oracle.variants).length, 11);
  assert.equal(lifecycle.oracle.phases.reduce((n, p) => n + p.actions.length, 0), 46);
  const required = ["source-field", "host-field", "trait-field", "dynamic-field", "unknown-field", "source-method",
    "host-method", "trait-method", "dynamic-method", "unknown-method", "damage-member-local", "repair-member-local",
    "redamage-member-local", "shebang-only-eof", "empty", "empty-vector", "boundary-line", "service-pinned-call"];
  for (const id of required) assert(spec.oracle.cases.some(c => c.case === id), id);
  for (const ref of spec.oracle.cases) {
    const { item } = oracle.resolve(ref);
    for (const crlf of [false, true]) for (const shifted of [false, true]) {
      const doc = oracle.document(item.source, crlf, shifted), wire = oracle.expected(doc, item.queries);
      const publicRows = oracle.publicExpected(doc, item.queries);
      assert.equal(wire.length, item.queries.length); assert.equal(publicRows.length, item.queries.length);
      for (const line of doc.text.split(/\r?\n/)) assert(line.length < 1000, "SDK word window never truncates the corpus");
      for (let i = 0; i < item.queries.length; i++) {
        const before = item.queries.findIndex((q, j) => j < i && q.position === item.queries[i].position);
        if (before >= 0) { assert.deepEqual(wire[i], wire[before]); assert.deepEqual(publicRows[i], publicRows[before]); }
      }
      assert.deepEqual(oracle.publicExpected(doc, []), []); assert.deepEqual(oracle.expected(doc, []), []);
    }
  }
  assert.equal(Object.hasOwn(require("../../editors/vscode/language-configuration.json"), "wordPattern"), false);
});

test("installed selection literal public geometry pins SDK augmentation separately from exact wire ancestry", () => {
  const { item } = oracle.resolve({ fixture: "selection-trivia", case: "baseline-field-trivia" });
  const q = item.queries.find(q => q.chain.length);
  for (const crlf of [false, true]) for (const shifted of [false, true]) {
    const doc = oracle.document(item.source, crlf, shifted), line = shifted ? 2 : 0;
    assert.deepEqual(flatten(oracle.publicExpected(doc, [q])[0]), [
      [line+1,13,line+1,18],[line+1,9,line+1,18],[line+1,2,line+1,21],[line+1,0,line+1,21],
      [line,20,line+2,3],[line,8,line+2,3],[line,0,line+2,3],[0,0,line+3,0]
    ]);
    assert.equal(flatten(oracle.expected(doc, [q])[0]).length, 6);
    const empty = oracle.document("[[file:start]][[cursor]][[file:end]]", crlf, shifted);
    assert.equal(oracle.publicExpected(empty, [{ position: "cursor", chain: [] }]).length, 1);
  }
  assert.deepEqual(oracle.wordAt("  hostThing_name  ", 7), { text: "hostThing_name", start: 2, end: 16 });
  assert.deepEqual(oracle.subword({ text: "hostThing_name", start: 2, end: 16 }, 7), { start: 6, end: 11 });
  assert.deepEqual(oracle.wordAt("return 1.25f32;", 10), { text: "1.25f32", start: 7, end: 14 });
  assert.equal(oracle.wordAt(" ; ", 1), undefined);
});

test("installed selection brackets retain authored ASCII geometry and complete literal parents in every Unicode form", () => {
  const { item } = oracle.resolve({ fixture: "selection-literals", case: "unicode-interpolation-index-operator-and-named-call-owners" });
  const q = item.queries[11]; assert.equal(q.token, "[");
  for (const crlf of [false, true]) for (const shifted of [false, true]) {
    const doc = oracle.document(item.source, crlf, shifted), line = shifted ? 2 : 0;
    const spans = rows => rows.map(([a,b]) => [line,a,line,b]).concat([[0,0,line+1,0]]);
    const utf16 = spans([[52,53],[42,59],[42,66],[41,67],[35,99],[22,100],[20,117],[8,117]]);
    assert.deepEqual(flatten(oracle.expected(doc, [q])[0]), utf16);
    assert.deepEqual(flatten(oracle.expected(doc, [q], false)[0]),
      spans([[60,61],[50,67],[50,74],[49,75],[39,107],[26,108],[24,125],[12,125]]));
    assert.deepEqual(flatten(oracle.publicExpected(doc, [q])[0]),
      utf16.slice(0,-1).concat([[line,0,line,117],[0,0,line+1,0]]));
    assert.throws(() => oracle.expected(doc, [{ ...q, chain: [] }]));
    assert.throws(() => oracle.expected(doc, [{ ...q, position: "member-cursor" }]));
  }
});

test("installed selection literal lifecycle states preserve disk overlay ownership and separate closed/absent models", () => {
  const phases = lifecycle.oracle.phases;
  for (const id of ["close-main-restores-disk", "close-saved-damaged-helper", "repair-saved-closed-helper",
    "rename-helper-back", "reopen-helper-version-reset", "repair-only-main", "repair-only-helper", "close-all-restores-baseline"]) {
    assert(phases.some(p => p.id === id), id);
  }
  const closed = phases.find(p => p.id === "repair-saved-closed-helper");
  assert(!Object.hasOwn(closed.open, "scripts/helper.vela"));
  assert.equal(closed.views["scripts/helper.vela"], closed.disk["scripts/helper.vela"]);
  assert(phases.some(p => p.disk["scripts/helper.vela"] === null && p.open["scripts/helper.vela"]));
  for (const p of phases) for (const [file, name] of Object.entries(p.views)) {
    assert.equal(name, p.open[file] || p.disk[file]);
    if (name) { const v = lifecycle.oracle.variants[name]; assert(v); assert(Array.isArray(v.queries)); }
  }
});

test("installed selection trace pairs complete fresh envelopes and distinguishes [] null errors duplicate IDs and partial records", () => {
  const params = { textDocument: { uri: "file:///Chinese%20%25/a.vela" }, positions: [{ line: 0, character: 4 }] };
  const sent = id => `Sending request 'textDocument/selectionRange - (${id})'.\nParams: ${JSON.stringify(params)}\n\n\n`;
  const received = (id, body) => `Received response 'textDocument/selectionRange - (${id})' in 2ms.\n${body}\n\n\n`;
  const text = sent(1) + received(1, "Result: []");
  assert.deepEqual(selectionTrace(text), { requests: [{ id: "1", params }], responses: [{ id: "1", params, result: [] }] });
  assert.equal(selectionTrace(sent(2) + received(2, "No result returned.")).responses[0].result, null);
  assert.deepEqual(selectionTrace(text.slice(0, -1)).responses, []);
  assert.throws(() => selectionTrace(sent(1) + sent(1)), /duplicate selection request/);
  assert.throws(() => selectionTrace(text + received(1, "Result: []")), /duplicate selection response/);
  assert.throws(() => selectionTrace(sent(1) + received(1, "Request failed: cancelled")), /unrecognized selection response/);
});
