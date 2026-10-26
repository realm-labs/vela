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
test("fixture append preserves Unicode CRLF bytes, disk state and existing unrelated files", t => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "vela-fixture-"));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  fs.writeFileSync(path.join(root, "unrelated"), "keep\r\n");
  const fixture = new FixtureWorkspace({ version: 1, id: "append", actions: [], files: {
    "中文 %/source.vela": "中😀 [[value:start]]value[[value:end]]\r\n",
  } });
  fixture.apply({ op: "open", file: "中文 %/source.vela" });
  fixture.apply({ op: "change", file: "中文 %/source.vela", source: "dirty overlay" });
  fixture.appendTo(root);
  assert.deepEqual(fs.readFileSync(path.join(root, "中文 %/source.vela")), Buffer.from("中😀 value\r\n"));
  assert.equal(fs.readFileSync(path.join(root, "unrelated"), "utf8"), "keep\r\n");
  assert.equal(fixture.document("中文 %/source.vela").text, "dirty overlay");
  assert.deepEqual(fs.readdirSync(root).sort(), ["unrelated", "中文 %"]);
  assert.throws(() => fixture.materialize(root), /new and isolated/);
  assert.throws(() => fixture.appendTo(root), /collision/);
});

test("fixture append requires an existing directory without creating a new root", t => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "vela-fixture-"));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const fixture = new FixtureWorkspace(lifecycle), missing = path.join(root, "missing");
  assert.throws(() => fixture.appendTo(missing), { code: "ENOENT" });
  assert.equal(fs.existsSync(missing), false);
  const file = path.join(root, "file"); fs.writeFileSync(file, "keep");
  assert.throws(() => fixture.appendTo(file), /existing directory/);
  assert.equal(fs.readFileSync(file, "utf8"), "keep");
});

for (const collision of ["file", "directory", "parent-file", "fixture-parent"]) {
  test(`fixture append preflights ${collision} collisions before writing any files`, t => {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), "vela-fixture-"));
    t.after(() => fs.rmSync(root, { recursive: true, force: true }));
    if (collision === "directory") fs.mkdirSync(path.join(root, "taken"));
    else if (collision !== "fixture-parent") fs.writeFileSync(path.join(root, "taken"), "keep");
    const files = { "new/first.vela": "first" };
    files[collision.endsWith("parent") || collision === "parent-file" ? "taken/child.vela" : "taken"] = "replacement";
    if (collision === "fixture-parent") files.taken = "conflicting parent";
    const fixture = new FixtureWorkspace({ version: 1, id: "collision", actions: [], files });
    assert.throws(() => fixture.appendTo(root), /collision/);
    assert.equal(fs.existsSync(path.join(root, "new")), false);
    if (collision === "file" || collision === "parent-file") {
      assert.equal(fs.readFileSync(path.join(root, "taken"), "utf8"), "keep");
    }
  });
}

test("fixture append rejects symlink roots and parents without escaping the workspace", t => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "vela-fixture-"));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const workspace = path.join(root, "workspace"), outside = path.join(root, "outside");
  fs.mkdirSync(workspace); fs.mkdirSync(outside);
  fs.symlinkSync(outside, path.join(workspace, "linked"), "junction");
  const fixture = new FixtureWorkspace({ version: 1, id: "symlink", actions: [], files: {
    "first.vela": "first", "linked/escaped.vela": "must not escape",
  } });
  assert.throws(() => fixture.appendTo(path.join(workspace, "linked")), /existing directory/);
  assert.throws(() => fixture.appendTo(workspace), /collision/);
  assert.deepEqual(fs.readdirSync(outside), []);
  assert.deepEqual(fs.readdirSync(workspace), ["linked"]);
});

test("fixture append rejects a trailing-separator symlink root before writing", t => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "vela-fixture-"));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const outside = path.join(root, "outside"), link = path.join(root, "linked");
  fs.mkdirSync(outside);
  fs.writeFileSync(path.join(outside, "keep"), "unchanged");
  fs.symlinkSync(outside, link, "junction");
  const fixture = new FixtureWorkspace({ version: 1, id: "trailing-symlink", actions: [], files: {
    "first.vela": "must not be written",
  } });
  assert.throws(() => fixture.appendTo(link + path.sep), /existing directory/);
  assert.deepEqual(fs.readdirSync(outside), ["keep"]);
  assert.equal(fs.readFileSync(path.join(outside, "keep"), "utf8"), "unchanged");
  assert.deepEqual(fs.readdirSync(root).sort(), ["linked", "outside"]);
});

for (const [name, files, insensitive] of [
  ["new file before its directory use", ["taken", "taken/child.vela"], false],
  ["case-aliased files", ["Taken", "taken"], true],
  ["case-aliased file before directory", ["Taken", "taken/child.vela"], true],
  ["case-aliased directory before file", ["Taken/child.vela", "taken"], true],
  ["case-aliased nested files", ["Taken/child.vela", "taken/CHILD.vela"], true],
]) {
  test(`fixture append rejects ${name} without partial destination writes`, t => {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), "vela-fixture-"));
    t.after(() => fs.rmSync(root, { recursive: true, force: true }));
    if (insensitive) {
      fs.writeFileSync(path.join(root, "CaseProbe"), "probe");
      const actual = fs.existsSync(path.join(root, "caseprobe"));
      fs.unlinkSync(path.join(root, "CaseProbe"));
      if (!actual) { t.skip("requires an actual case-insensitive test volume"); return; }
    }
    fs.writeFileSync(path.join(root, "keep"), "unchanged");
    const fixture = new FixtureWorkspace({ version: 1, id: "aliases", actions: [], files: {
      "first.vela": "must not be written",
      ...Object.fromEntries(files.map(file => [file, "conflicting"])),
    } });
    assert.throws(() => fixture.appendTo(root), error =>
      /collision/.test(error.message) || ["EEXIST", "EISDIR", "ENOTDIR"].includes(error.code));
    assert.deepEqual(fs.readdirSync(root), ["keep"], "no partial files, directories or probes");
    assert.equal(fs.readFileSync(path.join(root, "keep"), "utf8"), "unchanged");
  });
}

test("fixture append allows distinct files beneath case-aliased directory spellings", t => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "vela-fixture-"));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const files = { "Taken/first.vela": "first", "taken/second.vela": "second" };
  new FixtureWorkspace({ version: 1, id: "shared-directories", actions: [], files }).appendTo(root + path.sep);
  for (const [file, text] of Object.entries(files)) assert.equal(fs.readFileSync(path.join(root, file), "utf8"), text);
  assert(fs.readdirSync(root).every(file => file === "Taken" || file === "taken"), "no leftover probes");
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
