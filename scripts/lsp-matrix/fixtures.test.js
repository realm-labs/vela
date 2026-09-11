"use strict";
const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { parseMarkers, safeFile, FixtureWorkspace, offsetAt, applyEdits } = require("./fixtures");
const golden = require("../../tests/lsp_matrix/fixtures/marker-golden.json");
const lifecycle = require("../../tests/lsp_matrix/fixtures/shared-unicode-lifecycle.json");

test("shared fixture markers match independent byte and UTF-16 LF/CRLF golden positions", () => {
  for (const item of golden.valid) assert.deepEqual(parseMarkers(item.source), { text: item.text, markers: item.markers }, item.id);
});
test("shared fixture parser rejects malformed duplicate crossing and split-CRLF markers", () => {
  for (const source of [...golden.invalid, "\ud800"]) assert.throws(() => parseMarkers(source), undefined, source);
});
test("shared multi-file actions retain overlays and restore independent disk oracles", () => {
  const fixture = new FixtureWorkspace(lifecycle);
  for (const [i, action] of lifecycle.actions.entries()) {
    fixture.apply(action);
    const actual = fixture.document(lifecycle.oracle.file), expected = lifecycle.oracle.afterEachAction[i];
    if (expected === null) { assert.equal(actual, undefined); continue; }
    const range = actual.markers.definition;
    assert.deepEqual([range.start.line, range.start.character, range.end.line, range.end.character],
      [expected.line, expected.character, expected.line, expected.endCharacter]);
    assert.equal(Buffer.from(actual.text).subarray(range.start.byte, range.end.byte).toString(), lifecycle.oracle.selectedText);
    assert.ok(actual.text.endsWith(expected.tail));
  }
});
test("fixture paths actions and materialization cannot overwrite unrelated files", () => {
  for (const file of ["../x", "/tmp/x", "a/../b", "a\\b", "C:x", "a//b", "a/./b", "a\0b"]) assert.throws(() => safeFile(file));
  for (const action of [{op:"change",file:"scripts/helper.vela",source:"x"}, {op:"close",file:"scripts/helper.vela"},
    {op:"open",file:"missing.vela"}, {op:"save",file:"scripts/helper.vela"}, {op:"oops",file:"x"}]) {
    assert.throws(() => new FixtureWorkspace(lifecycle).apply(action));
  }
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "vela-fixture-"));
  try {
    const fixture = new FixtureWorkspace(lifecycle), target = path.join(root, "中 % workspace");
    fixture.materialize(target);
    assert.equal(fs.readFileSync(path.join(target,"scripts/main.vela"),"utf8"),fixture.disk.get("scripts/main.vela").text);
    assert.throws(() => fixture.materialize(target));
  } finally { fs.rmSync(root, {recursive:true,force:true}); }
});
test("independent edit oracle handles Unicode ranges and rejects unsafe or overlapping edits", () => {
  const text = "中😀 first\r\nsecond";
  const range = (sl, sc, el, ec) => ({start:{line:sl,character:sc},end:{line:el,character:ec}});
  assert.equal(applyEdits(text,[{range:range(1,0,1,6),newText:"尾"},{range:range(0,4,0,9),newText:"next"}]), "中😀 next\r\n尾");
  for (const p of [{line:0,character:2},{line:0,character:10},{line:2,character:0},{line:-1,character:0}]) assert.throws(() => offsetAt(text,p));
  for (const edits of [[{range:range(0,5,0,4),newText:"x"}],
    [{range:range(0,4,0,7),newText:"a"},{range:range(0,6,0,9),newText:"b"}],
    [{range:range(0,4,0,4),newText:"a"},{range:range(0,4,0,4),newText:"b"}]]) assert.throws(() => applyEdits(text,edits));
});
