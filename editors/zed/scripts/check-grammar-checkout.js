"use strict";

const assert = require("node:assert/strict");
const { execFileSync } = require("node:child_process");

function checkGrammarCheckout(repositoryRoot) {
  // Zed creates an ignored checkout here; it may contain local work. Only reject
  // duplicate sources tracked in this repository, without touching cache contents.
  const tracked = execFileSync(
    "git",
    ["ls-files", "--", "editors/zed/grammars/vela"],
    { cwd: repositoryRoot, encoding: "utf8" }
  );
  assert.equal(tracked.trim(), "", "Zed grammar checkout directory must not contain tracked source files");
}

module.exports = { checkGrammarCheckout };
