"use strict";
const test = require("node:test"), assert = require("node:assert/strict"), fs = require("node:fs"), path = require("node:path"), os = require("node:os");
const { assertObserverHealthy } = require("../../editors/vscode/test/input/observer-health");
test("zero workbench exit and passing feature proofs cannot conceal an installed observer failure", () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "vela-observer-health-"));
  try {
    assertObserverHealthy(root);
    fs.writeFileSync(path.join(root, "bridge-failure.log"), "ReferenceError: identity is not defined\n");
    assert.throws(() => assertObserverHealthy(root), /installed observer failed/);
    assert.match(fs.readFileSync(path.join(root, "bridge-failure.log"), "utf8"), /ReferenceError/);
  } finally {
    assert(path.resolve(root).startsWith(path.resolve(os.tmpdir()) + path.sep));
    assert(path.basename(root).startsWith("vela-observer-health-")); fs.rmSync(root, { recursive: true });
  }
});
