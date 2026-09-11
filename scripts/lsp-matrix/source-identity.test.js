"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { execFileSync } = require("node:child_process");
const { sourceIdentity } = require("./source-identity");

test("source identity includes edits, new files and deletions but excludes its own checkpoint", (t) => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "vela-matrix-identity-"));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const git = (args) => execFileSync("git", args, { cwd: root, timeout: 10000, stdio: "pipe" });
  git(["init"]);
  fs.writeFileSync(path.join(root, "input.txt"), "original");
  git(["add", "input.txt"]);
  git(["-c", "user.name=Matrix Test", "-c", "user.email=matrix@example.invalid", "-c", "commit.gpgsign=false", "commit", "-m", "test: source identity fixture"]);
  const initial = sourceIdentity(root);
  assert.match(initial.revision, /^[a-f0-9]{40}$/);
  assert.deepEqual(sourceIdentity(root), initial);
  fs.mkdirSync(path.join(root, "tests/lsp_matrix"), { recursive: true });
  fs.writeFileSync(path.join(root, "tests/lsp_matrix/checkpoint.json"), "changed status");
  assert.deepEqual(sourceIdentity(root), initial);
  fs.writeFileSync(path.join(root, "input.txt"), "modified");
  assert.notEqual(sourceIdentity(root).treeSha256, initial.treeSha256);
  fs.writeFileSync(path.join(root, "input.txt"), "original");
  fs.writeFileSync(path.join(root, "untracked.txt"), "new implementation");
  assert.notEqual(sourceIdentity(root).treeSha256, initial.treeSha256);
  fs.unlinkSync(path.join(root, "untracked.txt"));
  fs.unlinkSync(path.join(root, "input.txt"));
  assert.notEqual(sourceIdentity(root).treeSha256, initial.treeSha256);
});
