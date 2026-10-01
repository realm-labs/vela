"use strict";
const test = require("node:test"), assert = require("node:assert/strict");
const { suites, appendSuite } = require("./rust-suites"), model = require("./model");
const proof = [{ requirement: "stdio", tests: [{ layer: "protocol", name: "real_stdio" }] }];
const requirements = [{ id: "stdio", layer: "protocol" }];
test("Rust audit declares a real stdio process target alongside both libraries", () => {
  assert.deepEqual(suites.map((s) => [s.layer, s.crate, s.target]), [
    ["service", "vela_language_service", ["--lib"]], ["protocol", "vela_lsp_server", ["--lib"]],
    ["protocol", "vela_lsp_server", ["--test", "stdio_transport", "--target-dir", "target/lsp-matrix-stdio"]]
  ]);
});
test("stdio discovery cannot substitute library or listed-only results for execution", () => {
  const available = {}, results = {};
  appendSuite(suites[1], "lib: test\n", "test lib ... ok\n", available, results);
  assert.throws(() => model.assess(requirements, proof, available, results), /stale test reference/);
  appendSuite(suites[2], "real_stdio: test\n", undefined, available, results);
  assert.equal(model.assess(requirements, proof, available, results)[0].status, "mapped");
});
test("a failed or skipped stdio result remains failed even with a passing library", () => {
  for (const state of ["FAILED", "ignored", "ok"]) {
    const available = {}, results = {};
    appendSuite(suites[1], "lib: test\n", "test lib ... ok\n", available, results);
    appendSuite(suites[2], "real_stdio: test\n", `test real_stdio ... ${state}\n`, available, results);
    assert.equal(results.protocol.get("lib"), "ok");
    assert.equal(model.assess(requirements, proof, available, results)[0].status, state === "ok" ? "verified" : "failed");
  }
});
test("Rust suite merging rejects duplicate identities and undiscovered results", () => {
  const available = {}, results = {};
  appendSuite(suites[1], "shared: test\n", "test shared ... ok\n", available, results);
  assert.throws(() => appendSuite(suites[2], "shared: test\n", "test shared ... ok\n", available, results), /duplicate/);
  assert.throws(() => appendSuite(suites[2], "real_stdio: test\n", "test phantom ... ok\n", available, results), /undiscovered/);
});
