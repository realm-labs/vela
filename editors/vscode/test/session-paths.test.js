"use strict";
const test = require("node:test");
const assert = require("node:assert/strict");
const path = require("node:path");
const fs = require("node:fs");
const { extraSessionDirectory } = require("./session-paths");

const sessions = ["workspace-symbol", "selection-syntax", "selection-lifecycle"];
const socketFor = directory => path.join(directory, "user-data", "1.999-main.sock");

test("extra editor sessions keep macOS sockets short and independent inside the archived run", () => {
  const root = path.resolve(__dirname, "../test-results/run-XXXXXX");
  const oldRoot = "/Users/mikai/CLionProjects/vela/editors/vscode/test-results/run-Gkgtr4";
  for (const base of [root, oldRoot]) {
    const directories = sessions.map(session => extraSessionDirectory(base, session, "darwin"));
    assert.equal(new Set(directories).size, 3, "no shared artifact or user-data directories");
    for (const directory of directories) {
      assert.equal(path.dirname(directory), base, "all evidence stays in its owning run");
      assert(Buffer.byteLength(socketFor(directory), "utf8") <= 103);
      assert.notEqual(path.join(directory, "user-data"), path.join(base, "user-data"), "main profile stays isolated");
    }
  }
  for (const previous of ["workspace-symbol-editor", "selection-syntax-editor", "selection-lifecycle-editor"])
    assert(Buffer.byteLength(socketFor(path.join(oldRoot, previous)), "utf8") > 103, "reproduce the original path-length regression");
});

test("macOS socket budget counts UTF-8 bytes and rejects unknown or overlong session paths", () => {
  const ascii = "/tmp/" + "a".repeat(26), unicode = "/tmp/" + "中".repeat(26);
  assert.doesNotThrow(() => extraSessionDirectory(ascii, "workspace-symbol", "darwin"));
  assert.throws(() => extraSessionDirectory(unicode, "workspace-symbol", "darwin"), /exceeds 103 bytes/);
  assert.throws(() => extraSessionDirectory("/tmp/" + "a".repeat(100), "selection-syntax", "darwin"), /shorter checkout path/);
  assert.throws(() => extraSessionDirectory(ascii, "../shared", "darwin"), /unknown isolated editor session/);
  assert.doesNotThrow(() => extraSessionDirectory(unicode, "workspace-symbol", "linux"));
});

test("both symbol and selection runner paths use the bounded session directory mapping", () => {
  const runner = fs.readFileSync(path.join(__dirname, "run.js"), "utf8");
  assert(runner.includes('extraSessionDirectory(resultRoot, "workspace-symbol")'));
  assert(runner.includes('extraSessionDirectory(resultRoot, `selection-${kind}`)'));
});
