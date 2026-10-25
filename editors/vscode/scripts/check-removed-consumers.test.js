"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");

const root = path.resolve(__dirname, "../../..");
const fixtures = path.join(root, "tests/lsp_matrix/fixtures");
const load = (name) => JSON.parse(fs.readFileSync(path.join(fixtures, `${name}.json`), "utf8"));

function inspectArtifacts(value, file) {
  if (typeof value === "string" && value.trimStart().startsWith("{")) {
    let embedded;
    try { embedded = JSON.parse(value); } catch { return; } // Deliberately malformed schema cases stay malformed.
    inspectArtifacts(embedded, file);
  } else if (Array.isArray(value)) {
    for (const item of value) inspectArtifacts(item, file);
  } else if (value && typeof value === "object") {
    if (Object.hasOwn(value, "formatVersion")) {
      assert.notEqual(value.formatVersion, 1, `${file}: obsolete schema format`);
      assert.equal(Object.hasOwn(value, "serviceSet"), false, `${file}: removed schema metadata`);
    }
    for (const item of Object.values(value)) inspectArtifacts(item, file);
  }
}

test("shared editor artifacts do not consume format 1 or Service metadata", () => {
  for (const file of fs.readdirSync(fixtures).filter((file) => file.endsWith(".json"))) {
    inspectArtifacts(JSON.parse(fs.readFileSync(path.join(fixtures, file), "utf8")), file);
  }
});

test("ordinary trait hint and return scenarios survive without Provider attributes", () => {
  for (const name of ["completion-callable-hints", "completion-callable-returns"]) {
    const spec = load(name);
    const sources = Object.values(spec.files).join("\n");
    assert.doesNotMatch(sources, /#\[provider/);
    assert.match(sources, /trait Provider/); // An ordinary trait name, not a discovery capability.
    assert.match(sources, /impl Provider for Worker/);
    assert.ok(spec.oracle.queries.length > 0);
  }
});

test("mixed call matrices keep ordinary coverage and no removed callable success contracts", () => {
  const mapping = load("call-parameter-mapping");
  assert.equal(mapping.oracle.queries.length, 41);
  assert.ok(mapping.oracle.queries.every((query) => !query.id.startsWith("service_")));
  const signatures = load("signature-s5");
  const removed = signatures.oracle.groups.find((group) => group.fixture === "call-parameter-mapping")
    .queries.filter((query) => query.id.startsWith("service_"));
  assert.equal(removed.length, 3);
  for (const query of removed) assert.equal(query.signature, null);
  for (const group of signatures.oracle.groups) {
    assert.ok(fs.existsSync(path.join(fixtures, `${group.fixture}.json`)));
    assert.doesNotMatch(group.fixture, /^completion-service-/);
  }
  for (const name of ["call-parameter-mapping", "callable-imports", "completion-call-expressions", "signature-s5"]) {
    assert.doesNotMatch(Object.values(load(name).files).join("\n"), /serviceSet|#\[service_impl/);
  }
  for (const labels of Object.values(load("callable-imports").oracle.expressionSets)) {
    assert.ok(labels.every((label) => !label.startsWith("service::")));
  }
  assert.ok(load("selection-calls").oracle.cases.length > 0);
});

test("removed example bins stay retired while ordinary consumer entrypoints remain", () => {
  for (const name of ["service_hard_switch_fixture", "service_hotfix_coverage", "scoped_service_task", "plugin_provider_demo"]) {
    assert.equal(fs.existsSync(path.join(root, "examples/src/bin", name)), false);
  }
  for (const name of ["modules", "package_dependency_demo", "native_function", "serde_value", "hot_reload_function_swap"]) {
    assert.ok(fs.existsSync(path.join(root, "examples/src/bin", name, "main.rs")));
  }
});
