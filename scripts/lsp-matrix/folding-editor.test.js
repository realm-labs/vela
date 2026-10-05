"use strict";
const assert = require("node:assert/strict"), test = require("node:test");
const fs = require("node:fs"), os = require("node:os"), path = require("node:path");
const spec = require("../../tests/lsp_matrix/fixtures/folding-editor.json");
const oracle = require("./folding-editor-oracle");
const goldens = [
  [[0, 1, "imports"]],
  [[0, 6, "region"], [1, 3, "region"], [3, 5, "region"]],
  [[0, 9, "region"], [1, 7, "region"], [2, 5, "region"]],
  [[2, 11, "region"], [3, 9, "region"], [5, 8, "region"]],
  [[1, 6, "imports"]],
  [[0, 2, "region"], [3, 5, "region"], [6, 8, "region"]], [], [],
];

test("folding editor pins independently marked partitions and literal complete public line models", () => {
  assert.equal(spec.oracle.cases.length, 8);
  assert.deepEqual(spec.oracle.cases.map(c => c.ranges.length), [1, 4, 4, 4, 1, 5, 0, 0]);
  assert.equal(spec.oracle.cases.reduce((n, c) => n + c.ranges.length, 0), 19);
  assert.equal(goldens.reduce((n, rows) => n + rows.length, 0), 14);
  const kinds = { Imports: 2, Region: 3 };
  assert.deepEqual(oracle.publicRanges([{ start: 0, end: 1, kind: 2 }, { start: 2, end: 4, kind: 3 }], kinds),
    [{ start: 0, end: 1, kind: "imports" }, { start: 2, end: 4, kind: "region" }]);
  for (const kind of [undefined, 1, "imports", { value: "imports" }])
    assert.throws(() => oracle.publicRanges([{ start: 0, end: 1, kind }], kinds), /unexpected public folding kind/);
  for (const [index, c] of spec.oracle.cases.entries()) {
    const reviewed = require(`../../tests/lsp_matrix/fixtures/${c.partition}.json`).oracle.cases.find(row => row.id === c.id);
    assert.equal(c.source, reviewed.source); assert.deepEqual(c.ranges, reviewed.ranges);
    const doc = oracle.document(c.source);
    assert.deepEqual(oracle.editor(doc, c.ranges).map(r => [r.start, r.end, r.kind]), goldens[index]);
    assert.equal(oracle.wire(doc, c.ranges).length, c.ranges.length, "wire retains coincident declaration/body starts");
  }
});

test("folding editor keeps UTF16 byte differences and LF CRLF shifts including first-line shebang", () => {
  for (const crlf of [false, true]) for (const shifted of [false, true]) {
    for (const [index, c] of spec.oracle.cases.entries()) {
      const doc = oracle.document(c.source, crlf, shifted), bytes = Buffer.from(doc.text);
      assert.deepEqual(oracle.editor(doc, c.ranges).map(r => [r.start, r.end, r.kind]),
        goldens[index].map(([start, end, kind]) => [start + (shifted ? 2 : 0), end + (shifted ? 2 : 0), kind]));
      if (c.source.startsWith("#!")) assert(doc.text.startsWith("#!/usr/bin/env vela\n") || doc.text.startsWith("#!/usr/bin/env vela\r\n"));
      if (c.id === "unresolved-pair") {
        assert.deepEqual(oracle.wire(doc, c.ranges), [{ startLine: shifted ? 2 : 0, startCharacter: 8,
          endLine: shifted ? 3 : 1, endCharacter: 37, kind: "imports" }]);
        const range = doc.markers.imports;
        for (const [point, column] of [[range.start, 12], [range.end, 41]])
          assert.equal(point.byte - (bytes.lastIndexOf(10, point.byte - 1) + 1), column);
      }
      assert.equal(doc.text.includes("\r\n"), crlf && (shifted || c.source.includes("\n")));
    }
  }
});

test("folding editor materializes four fresh physical encoded roots with distinct dirty baselines", () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "vela-folding-editor-"));
  try {
    const { materializeFolding } = require("../../editors/vscode/test/folding-provider");
    materializeFolding(root);
    const dir = path.join(root, "中文 % folding roots"), names = fs.readdirSync(dir);
    assert.deepEqual(names.sort(), ["crlf-original", "crlf-shifted", "lf-original", "lf-shifted"]);
    for (const name of names) {
      const crlf = name.startsWith("crlf"), shifted = name.endsWith("shifted");
      assert.equal(fs.readFileSync(path.join(dir, name, spec.oracle.file), "utf8"),
        oracle.document("// disk baseline 中😀\n" + spec.files[spec.oracle.file], crlf, shifted).text);
      const manifest = fs.readFileSync(path.join(dir, name, "vela.toml"), "utf8");
      assert(manifest.includes(`id = 'dev.vela.folding.${crlf}.${shifted}'`));
      for (const c of spec.oracle.cases) assert.notEqual(oracle.document(c.source, crlf, shifted).text,
        fs.readFileSync(path.join(dir, name, spec.oracle.file), "utf8"));
    }
    assert.throws(() => materializeFolding(root), /new private/);
  } finally { fs.rmSync(root, { recursive: true }); }
});
