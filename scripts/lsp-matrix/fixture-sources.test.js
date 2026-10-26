"use strict";

const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const test = require("node:test");
const { loadFixture } = require("./fixture-sources");
const { parseMarkers } = require("./fixtures");
const { provenance } = require("./provenance");

function scratch(t) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "vela-fixture-sources-"));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const fixtures = path.join(root, "fixtures");
  fs.mkdirSync(path.join(fixtures, "sources"), { recursive: true });
  const file = path.join(fixtures, "case.json");
  const spec = source => ({ version: 1, id: "reader", files: { "scripts/main.vela": source }, actions: [], oracle: {} });
  const read = value => { fs.writeFileSync(file, JSON.stringify(value)); return loadFixture(file); };
  return { root, fixtures, file, spec, read };
}

const reference = { sourceFile: "sources/exact.vela" };

test("sourceFile preserves BOM, NUL, mixed EOL, Unicode and missing EOF before markers", t => {
  const s = scratch(t);
  const text = "\ufeff/* 中😀 */\r\n[[point]]fn main() {\0}\n\r// no final newline";
  fs.writeFileSync(path.join(s.fixtures, reference.sourceFile), Buffer.from(text));
  const loaded = s.read(s.spec(reference));
  assert.deepEqual(Buffer.from(loaded.files["scripts/main.vela"]), Buffer.from(text));
  assert.deepEqual(parseMarkers(loaded.files["scripts/main.vela"]), parseMarkers(text));
  assert.deepEqual(parseMarkers(text).markers.point.start, { byte: 18, line: 1, character: 0 });
  const inline = s.read(s.spec(text));
  assert.deepEqual(loaded, inline);
  loaded.files["scripts/main.vela"] = "changed";
  assert.equal(loadFixture(s.file).files["scripts/main.vela"], text, "no mutable require cache");
});

test("sourceFile rejects wrong, conflicting, unknown, missing and unsafe references", t => {
  const s = scratch(t);
  fs.writeFileSync(path.join(s.fixtures, "sources/exact.vela"), "fn main() {}");
  for (const value of [null, 1, false, [], {}, { sourceFile: 1 }, { sourceFile: null },
    { sourceFile: "sources/exact.vela", source: "fallback" }, { sourceFile: "sources/exact.vela", other: true },
    ...["", "/tmp/exact.vela", "C:/exact.vela", "C:exact.vela", "\\\\server\\exact.vela",
      "sources\\exact.vela", "../exact.vela", "sources/../exact.vela", "sources/./exact.vela",
      "sources//exact.vela", "sources/exact.vela/", "sources/\0exact.vela", "exact.vela",
      "sources/missing.vela", "sources"].map(sourceFile => ({ sourceFile }))]) {
    assert.throws(() => s.read(s.spec(value)), undefined, JSON.stringify(value));
  }
  fs.mkdirSync(path.join(s.fixtures, "sources/directory"));
  assert.throws(() => s.read(s.spec({ sourceFile: "sources/directory" })));
  assert.throws(() => s.read(s.spec("\ud800")), /Unicode/);
});

test("sourceFile rejects unpaired path surrogates even when the replacement filename exists", t => {
  const s = scratch(t), text = "fn replacement() {}";
  fs.writeFileSync(path.join(s.fixtures, "sources/\ufffd.vela"), text);
  assert.equal(s.read(s.spec({ sourceFile: "sources/\ufffd.vela" })).files["scripts/main.vela"], text);
  for (const sourceFile of ["sources/\ud800.vela", "sources/\udc00.vela"]) {
    assert.throws(() => s.read(s.spec({ sourceFile })), /source path must be valid Unicode/);
  }
});

test("sourceFile uses fatal UTF-8 decoding for documents and fixture JSON", t => {
  const s = scratch(t);
  for (const bytes of [[0xff], [0xc0, 0xaf], [0xed, 0xa0, 0x80], [0xf0, 0x9f, 0x98]]) {
    fs.writeFileSync(path.join(s.fixtures, reference.sourceFile), Buffer.from(bytes));
    assert.throws(() => s.read(s.spec(reference)), /encoded data/);
    fs.writeFileSync(s.file, Buffer.from(bytes));
    assert.throws(() => loadFixture(s.file), /encoded data/);
  }
});

test("sourceFile rejects escaping and internal symlinks including directory components", t => {
  const s = scratch(t);
  fs.writeFileSync(path.join(s.root, "outside.vela"), "outside");
  fs.writeFileSync(path.join(s.fixtures, "sources/exact.vela"), "inside");
  // Windows needs an enabled symlink privilege; do not silently skip this control.
  fs.symlinkSync(path.join(s.root, "outside.vela"), path.join(s.fixtures, "sources/escape.vela"), "file");
  fs.symlinkSync(path.join(s.fixtures, "sources/exact.vela"), path.join(s.fixtures, "sources/alias.vela"), "file");
  fs.symlinkSync(s.root, path.join(s.fixtures, "sources/directory"), "dir");
  for (const name of ["escape.vela", "alias.vela", "directory/outside.vela"]) {
    assert.throws(() => s.read(s.spec({ sourceFile: `sources/${name}` })), /symlink/);
  }
});

test("sourceFile resolves declared lifecycle slots before consumers, not arbitrary oracle metadata", t => {
  const s = scratch(t), text = "[[point]]fn value() {}";
  fs.writeFileSync(path.join(s.fixtures, reference.sourceFile), text);
  const spec = s.spec(reference);
  spec.id = "completion-source-overlay-lifecycle";
  spec.actions = [{ op: "write", file: "scripts/main.vela", source: reference }];
  spec.oracle = { sourceLifecycle: [{ files: { "scripts/api.vela": reference, "deleted.vela": null },
    action: { source: reference }, diskSource: reference, openSource: null }], metadata: { sourceFile: "sources/missing.vela" } };
  const loaded = s.read(spec), phase = loaded.oracle.sourceLifecycle[0];
  assert.equal(loaded.actions[0].source, text);
  assert.equal(phase.files["scripts/api.vela"], text);
  assert.equal(phase.files["deleted.vela"], null);
  assert.equal(phase.action.source, text);
  assert.equal(phase.diskSource, text);
  assert.equal(phase.openSource, null);
  assert.deepEqual(loaded.oracle.metadata, spec.oracle.metadata);
  spec.oracle.sourceLifecycle[0].openSource = { sourceFile: reference.sourceFile, text: "shadow" };
  assert.throws(() => s.read(spec));
});

test("every authored source reference resolves and every shared source has a consumer, without shadow strings", () => {
  const root = path.join(__dirname, "../../tests/lsp_matrix/fixtures");
  const referenced = new Map(), inline = new Set();
  function compare(raw, resolved) {
    if (raw && typeof raw === "object" && Object.hasOwn(raw, "sourceFile")) {
      assert.equal(typeof resolved, "string", "undeclared source slot");
      const bytes = fs.readFileSync(path.join(root, raw.sourceFile));
      assert.deepEqual(Buffer.from(resolved), bytes);
      assert.deepEqual(parseMarkers(resolved), parseMarkers(new TextDecoder("utf-8", { fatal: true, ignoreBOM: true }).decode(bytes)));
      referenced.set(raw.sourceFile, resolved);
    } else if (raw && typeof raw === "object") {
      assert.deepEqual(Object.keys(resolved), Object.keys(raw));
      for (const key of Object.keys(raw)) compare(raw[key], resolved[key]);
    } else {
      assert.equal(resolved, raw);
      if (typeof raw === "string") inline.add(raw);
    }
  }
  for (const file of fs.readdirSync(root).filter(file => file.endsWith(".json"))) {
    compare(JSON.parse(fs.readFileSync(path.join(root, file), "utf8")), loadFixture(path.join(root, file)));
  }
  assert(referenced.size > 0);
  for (const [file, text] of referenced) assert(!inline.has(text), `shadow source: ${file}`);
  function sources(directory) {
    for (const entry of fs.readdirSync(path.join(root, directory), { withFileTypes: true })) {
      const file = `${directory}/${entry.name}`;
      if (file === "sources/local") continue; // SF09's compile-time include owner.
      if (entry.isDirectory()) sources(file);
      else if (entry.name.endsWith(".vela")) assert(referenced.has(file), `orphan source: ${file}`);
    }
  }
  sources("sources");
});

test("raw provenance includes new source bytes and paths without relabelling old captures", t => {
  const s = scratch(t);
  for (const dir of ["editors/vscode/test", "tests/lsp_matrix/fixtures/sources/tokens", "scripts/lsp-matrix"]) {
    fs.mkdirSync(path.join(s.root, dir), { recursive: true });
  }
  for (const file of ["editors/vscode/extension.js", "editors/vscode/package.json", "editors/vscode/package-lock.json", "server"]) {
    fs.writeFileSync(path.join(s.root, file), "unchanged");
  }
  const source = path.join(s.root, "tests/lsp_matrix/fixtures/sources/tokens/exact.vela");
  fs.writeFileSync(source, "fn main() {}");
  const before = provenance(s.root, path.join(s.root, "server"));
  fs.writeFileSync(source, "fn main() {}\n");
  assert.notEqual(provenance(s.root, path.join(s.root, "server")).inputsSha256, before.inputsSha256);
  fs.writeFileSync(source, "fn main() {}");
  fs.renameSync(source, source.replace("exact.vela", "moved.vela"));
  const after = provenance(s.root, path.join(s.root, "server"));
  assert.notEqual(after.inputsSha256, before.inputsSha256);
  assert.equal(after.serverSha256, before.serverSha256);
});
