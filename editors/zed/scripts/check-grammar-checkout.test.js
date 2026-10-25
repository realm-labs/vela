"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { execFileSync } = require("node:child_process");
const { checkGrammarCheckout } = require("./check-grammar-checkout");

function repository(t) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "vela-zed-validator-"));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  execFileSync("git", ["init", "--quiet", root]);
  return root;
}

test("absent generated grammar checkout is valid", (t) => {
  checkGrammarCheckout(repository(t));
});

test("ignored checkout and local modifications are valid and remain untouched", (t) => {
  const root = repository(t);
  const cache = path.join(root, "editors/zed/grammars/vela");
  fs.mkdirSync(cache, { recursive: true });
  fs.writeFileSync(path.join(root, ".gitignore"), "editors/zed/grammars/vela/\n");
  execFileSync("git", ["init", "--quiet", cache]);
  fs.writeFileSync(path.join(cache, "grammar.js"), "user edits must survive\n");
  execFileSync("git", ["add", "grammar.js"], { cwd: cache });
  fs.appendFileSync(path.join(cache, "grammar.js"), "unstaged local work\n");
  checkGrammarCheckout(root);
  assert.equal(fs.readFileSync(path.join(cache, "grammar.js"), "utf8"), "user edits must survive\nunstaged local work\n");
  assert.match(execFileSync("git", ["--no-pager", "diff", "--", "grammar.js"], { cwd: cache, encoding: "utf8" }), /unstaged local work/);
});

test("tracked duplicate grammar sources reject without deleting them", (t) => {
  const root = repository(t);
  const cache = path.join(root, "editors/zed/grammars/vela");
  fs.mkdirSync(cache, { recursive: true });
  const file = path.join(cache, "grammar.js");
  fs.writeFileSync(file, "tracked duplicate\n");
  execFileSync("git", ["add", "editors/zed/grammars/vela/grammar.js"], { cwd: root });
  assert.throws(() => checkGrammarCheckout(root), /must not contain tracked source files/);
  assert.equal(fs.readFileSync(file, "utf8"), "tracked duplicate\n");
});
