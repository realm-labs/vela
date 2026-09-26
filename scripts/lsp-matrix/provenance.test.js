"use strict";
const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { provenance } = require("./provenance");

test("installed editor evidence becomes stale when its shared oracle decoder or trace parser changes", (t) => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "vela-token-provenance-"));
  t.after(() => {
    assert.equal(path.dirname(path.resolve(root)), path.resolve(os.tmpdir()));
    assert(path.basename(root).startsWith("vela-token-provenance-"));
    fs.rmSync(root, { recursive: true, force: true });
  });
  const write = (file, text) => {
    const target = path.join(root, file);
    fs.mkdirSync(path.dirname(target), { recursive: true });
    fs.writeFileSync(target, text);
  };
  for (const file of ["editors/vscode/extension.js", "editors/vscode/package.json", "editors/vscode/package-lock.json",
    "editors/vscode/test/provider.js", "tests/lsp_matrix/fixtures/tokens.json", "scripts/lsp-matrix/fixtures.js",
    "scripts/lsp-matrix/semantic-token-oracle.js", "scripts/lsp-matrix/semantic-token-trace.js", "server"]) write(file, "original");
  const binary = path.join(root, "server"), baseline = provenance(root, binary);
  assert.deepEqual(provenance(root, binary), baseline);
  for (const file of ["scripts/lsp-matrix/semantic-token-oracle.js", "scripts/lsp-matrix/semantic-token-trace.js",
    "tests/lsp_matrix/fixtures/tokens.json"]) {
    write(file, "changed");
    const changed = provenance(root, binary);
    assert.notEqual(changed.inputsSha256, baseline.inputsSha256, file);
    assert.equal(changed.serverSha256, baseline.serverSha256);
    write(file, "original");
    assert.deepEqual(provenance(root, binary), baseline);
  }
});
