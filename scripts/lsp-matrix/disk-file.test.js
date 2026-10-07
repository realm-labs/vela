"use strict";
const assert = require("node:assert/strict"), test = require("node:test");
const fs = require("node:fs"), os = require("node:os"), path = require("node:path");
const { readFileIfPresent } = require("../../editors/vscode/test/input/disk-file");

test("native disk reads observe rename disappearance without an existence/read race", () => {
  const missing = Object.assign(new Error("file disappeared before the read"), { code: "ENOENT" });
  const observed = [];
  const read = (file, encoding) => {
    observed.push([file, encoding]);
    if (observed.length === 1) throw missing;
    return "/*中😀*/ fn retained() {}\r\n";
  };
  assert.equal(readFileIfPresent("owned.vela", read), null);
  assert.equal(readFileIfPresent("owned.vela", read), "/*中😀*/ fn retained() {}\r\n");
  assert.deepEqual(observed, [["owned.vela", "utf8"], ["owned.vela", "utf8"]]);
  assert.equal(readFileIfPresent("empty.vela", () => ""), "");
});

test("native disk reads preserve exact rename contents and fail all non-missing errors", () => {
  for (const code of ["EACCES", "EIO", "EISDIR"]) {
    const error = Object.assign(new Error(code), { code });
    assert.throws(() => readFileIfPresent("owned.vela", () => { throw error; }), e => e === error);
  }
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "vela-disk-read-"));
  try {
    const old = path.join(root, "old.vela"), next = path.join(root, "new.vela");
    const text = "/*中😀*/ fn unchanged() {}\r\n";
    fs.writeFileSync(old, text); assert.equal(readFileIfPresent(old), text);
    fs.renameSync(old, next);
    assert.equal(readFileIfPresent(old), null); assert.equal(readFileIfPresent(next), text);
  } finally {
    const resolved = fs.realpathSync(root), temp = fs.realpathSync(os.tmpdir());
    assert(resolved.startsWith(temp + path.sep)); assert(path.basename(resolved).startsWith("vela-disk-read-"));
    fs.rmSync(resolved, { recursive: true, force: true });
  }
});
