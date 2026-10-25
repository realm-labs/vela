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
      assert.ok(![1, 2].includes(value.formatVersion), `${file}: obsolete schema format`);
      assert.equal(Object.hasOwn(value, "serviceSet"), false, `${file}: removed schema metadata`);
      if (value.formatVersion === 4) inspectValueSchema(value.facts, file);
    }
    for (const field of ["asyncness", "spawnsTasks", "taskTargets", "taskMetadata", "executionEffectCeiling"]) {
          assert.equal(Object.hasOwn(value, field), false, `${file}: removed ${field} metadata`);
        }
        for (const item of Object.values(value)) inspectArtifacts(item, file);
  }
}

test("shared editor artifacts reject obsolete schema formats and removed metadata", () => {
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

test("async and task-only matrices are rejection contracts, never synchronous success rewrites", () => {
  const names = fs.readdirSync(fixtures).filter((file) => /^completion-(task-|async-callables|await-contexts)/.test(file));
  assert.ok(names.length >= 10);
  for (const file of names) {
    const spec = load(file.slice(0, -5));
    assert.equal(spec.oracle.removedSurface, true, file);
    assert.ok(spec.oracle.cases.some((entry) => entry.syntaxError));
    assert.ok(spec.oracle.cases.some((entry) => entry.signatureNull));
    assert.match(Object.values(spec.files).join("\n"), /async fn/);
    assert.match(Object.values(spec.files).join("\n"), /\.await/);
    assert.equal(spec.oracle.queries, undefined);
  }
});

test("removed spellings in selection sources are diagnosed nonexecuting recovery cases", () => {
  for (const name of ["selection-declarations", "selection-calls", "selection-types", "selection-literals"]) {
    for (const entry of load(name).oracle.cases) {
      const source = entry.source.replace(/\[\[[^\]]+\]\]/g, "");
      if (/\basync\b|\.await|task::spawn_scoped/.test(source)) {
        assert.ok(entry.syntaxCodes.length > 0, `${name}/${entry.id}`);
        assert.ok(entry.syntaxCodes.every((code) => /^syntax::removed_(async|task)$/.test(code)));
      }
    }
  }
});

test("editor grammars keep ordinary state without removed extern, async or await keywords", () => {
  const grammar = fs.readFileSync(path.join(root, "editors/tree-sitter-vela/grammar.js"), "utf8");
  assert.doesNotMatch(grammar, /["'](?:async|await|extern)["']/);
  const generated = JSON.parse(fs.readFileSync(path.join(root, "editors/tree-sitter-vela/src/node-types.json"), "utf8"));
  assert.ok(!generated.some((node) => ["extern", "async", "await"].includes(node.type)));
  const state = generated.find((node) => node.type === "state_declaration");
  assert.equal(state.fields.value.required, true);
  for (const file of ["editors/vscode/syntaxes/vela.tmLanguage.json", "site/src/syntax/vela.tmLanguage.json"]) {
    const textmate = fs.readFileSync(path.join(root, file), "utf8");
    assert.doesNotMatch(textmate, /\basync\b|\bawait\b|\bextern\b/, file);
    const declarations = JSON.parse(textmate).repository.declarations.patterns;
    const stateRule = declarations.find((rule) => rule.match.includes("(state)"));
    const match = new RegExp(stateRule.match).exec("state count: i64 = 0");
    assert.deepEqual(match.slice(1), ["state", "count"], file);
    assert.equal(stateRule.captures[1].name, "keyword.declaration.vela", file);
    assert.match(stateRule.captures[2].name, /^variable\./, file);
  }
  const highlights = fs.readFileSync(path.join(root, "editors/zed/languages/vela/highlights.scm"), "utf8");
  assert.doesNotMatch(highlights, /"(?:extern|async|await)"/);
  assert.match(highlights, /\(state_declaration "state" @keyword\)/);
});

test("removed example bins stay retired while ordinary consumer entrypoints remain", () => {
  for (const name of ["service_hard_switch_fixture", "service_hotfix_coverage", "scoped_service_task", "plugin_provider_demo", "async_basic", "async_stateful_reentry"]) {
    assert.equal(fs.existsSync(path.join(root, "examples/src/bin", name)), false);
  }
  for (const name of ["modules", "package_dependency_demo", "native_function", "serde_value", "hot_reload_function_swap"]) {
    assert.ok(fs.existsSync(path.join(root, "examples/src/bin", name, "main.rs")));
  }
});

function inspectValueSchema(value, file) {
  if (Array.isArray(value)) {
    for (const item of value) inspectValueSchema(item, file);
  } else if (value && typeof value === "object") {
    assert.ok(!["host", "arrayView", "arrayMut", "mapView", "mapMut", "setView", "setMut", "scopedIterator"].includes(value.kind), `${file}: removed type fact`);
    for (const field of ["scoped_resource", "scopedResource", "indexCapabilities", "receiver", "storage", "capabilities", "collectionView", "constructorIds", "hostConstructors"]) {
      assert.equal(Object.hasOwn(value, field), false, `${file}: removed ${field} fact`);
    }
    for (const item of Object.values(value)) inspectValueSchema(item, file);
  }
}

test("checkpoint D retires borrowed success expectations and dependent example registrations", () => {
  for (const name of ["completion-callable-hints", "completion-stdlib-arguments"]) {
    const retired = load(name).oracle.queries.filter((query) => query.removedBoundary);
    assert.ok(retired.length > 0, name);
    for (const query of retired) {
      assert.deepEqual(query.parameters, [], query.id);
      assert.equal(query.signature, null, query.id);
      assert.ok(["hir::removed_host_type_hint", "syntax::removed_host_borrow"].includes(query.expectedDiagnostic), query.id);
    }
  }
  for (const name of ["host_type_methods", "host_iterable", "interop_round_trip", "level_up", "stale_host_ref"]) {
    assert.equal(fs.existsSync(path.join(root, "examples/src/bin", name, "main.rs")), false, name);
  }
  assert.equal(fs.existsSync(path.join(root, "examples/build.rs")), false);
  for (const name of ["native_function", "serde_value", "modules", "package_dependency_demo", "script_state", "hot_reload_function_swap"]) {
    assert.ok(fs.existsSync(path.join(root, "examples/src/bin", name, "main.rs")), name);
  }
});
