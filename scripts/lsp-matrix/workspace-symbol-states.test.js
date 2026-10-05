"use strict";
const test = require("node:test"), assert = require("node:assert/strict");
const changes = require("../../tests/lsp_matrix/fixtures/workspace-symbol-states.json");
const base = require("../../tests/lsp_matrix/fixtures/workspace-symbol-ownership.json");
const { parseMarkers } = require("./fixtures");

test("workspace state sequences retain the complete authored inventory and distinct empty probes", () => {
  assert.equal(changes.base, base.id);
  assert.equal(changes.rows, 60);
  assert.equal(changes.baseQueries, 49);
  assert.equal(base.oracle.symbols.length, changes.rows);
  assert.equal(base.oracle.queries.length, changes.baseQueries);
  assert.deepEqual(Object.keys(changes.sequences), ["unresolved", "dynamic"]);
  assert.deepEqual(changes.emptyQueries, ["absent_state", "StateGhost", "state_call", "state_variant", "unknown_state", "state_field", "state_method", "state_local", "state_callback"]);
  for (const [group, steps] of Object.entries(changes.sequences)) {
    assert.equal(steps.length, 6);
    assert.equal(steps[0].id, "baseline");
    assert.equal(steps.at(-1).id, "restore");
    assert.equal(steps[3].id, "repair");
    assert.equal(steps[4].id, "redamage");
    assert.deepEqual([steps[0].body, steps[0].imports, steps.at(-1).body, steps.at(-1).imports], ["", "", "", ""]);
    assert.deepEqual([steps[1].imports, steps[1].body], [steps[4].imports, steps[4].body]);
    assert.match(steps[3].body, /row\.read\(\); host\.read\(\); build\(value = 2\);/);
    if (group === "unresolved") {
      assert.match(steps[1].imports, /use absent_state::Phantom as StateGhost;/);
      assert.match(steps[2].body, /unknown_state\.state_method\(\); state_call\(\); StateGhost::state_variant;/);
    } else {
      assert(steps.every(step => step.imports === ""));
      assert.match(steps[1].body, /dynamic\.read\(\);/);
      assert.match(steps[2].body, /state_local: Any/);
      assert.match(steps[2].body, /state_local\[0\]\.state_method\(\)/);
      assert.match(steps[2].body, /\|\| dynamic\.state_method\(\)/);
    }
  }
  for (const probe of changes.emptyQueries) assert(base.oracle.symbols.every(row => !row.name.toLowerCase().includes(probe.toLowerCase())));
});

test("workspace state markers pin shifted UTF16 and full body extents while other source bytes stay fixed", () => {
  const raw = base.files[changes.file];
  for (const crlf of [false, true]) for (const shifted of [false, true]) {
    const transform = source => {
      if (shifted) source = source.replace("[[file:start]]", "[[file:start]]// shifted 中😀\n/* extra 😀 */\n");
      return crlf ? source.replaceAll("\n", "\r\n") : source;
    };
    const original = parseMarkers(transform(raw));
    for (const steps of Object.values(changes.sequences)) for (const step of steps) {
      const modified = raw.replace("/* 中😀 */ [[local-range:start]]", step.imports + "/* 中😀 */ [[local-range:start]]")
        .replace("\n}[[uses-range:end]]", "\n" + step.body + "}[[uses-range:end]]");
      const doc = parseMarkers(transform(modified)), imports = step.imports ? 1 : 0;
      const marker = doc.markers["local-name"];
      assert.deepEqual([marker.start.line, marker.start.character, marker.end.character], [6 + imports + (shifted ? 2 : 0), 17, 23]);
      const bytes = Buffer.from(doc.text), lineStart = bytes.lastIndexOf(10, marker.start.byte - 1) + 1;
      assert.equal(marker.start.byte - lineStart, 21);
      assert.equal(bytes.subarray(marker.start.byte, marker.end.byte).toString(), "Widget");
      assert.deepEqual([doc.markers.file.start.byte, doc.markers.file.end.byte], [0, bytes.length]);
      assert.equal(doc.markers["uses-range"].end.line - original.markers["uses-range"].end.line, imports + (step.body.match(/\n/g) || []).length);
      if (step.body === "" && step.imports === "") assert.deepEqual(doc, original);
      for (const row of base.oracle.symbols.filter(row => row.file === changes.file)) assert(doc.markers[row.range]);
      assert.equal(base.files["scripts/origins.vela"].includes("absent_state"), false);
    }
  }
});
