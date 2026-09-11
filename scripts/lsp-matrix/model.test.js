"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const model = require("./model");

function fixture() {
  const sources = {
    protocol: "| S1 | items | functions |\n## Protocol Matrix\n| `textDocument/definition` | definitionProvider | S1, S3-S5 | exact target | no guessed target |\n## Fixture Design",
    grammar: "(* grammar *)\nsource_file = item ;\nitem = 'fn' ;\n",
    syntax: { "syntax.rs": "enum Kind { Function }\n" }
  };
  const catalog = { version: 1,
    syntaxDimensions: model.syntaxDimensions(sources.protocol),
    grammarGroups: [{ id: "items", syntax: ["S1"], productions: ["item", "source_file"] }],
    grammarRules: model.grammarRules(sources.grammar), syntaxSources: { "syntax.rs": model.digest(sources.syntax["syntax.rs"]) },
    protocolBaseline: model.protocolRows(sources.protocol), states: { dirty: "overlay wins" }, environments: { utf16: "exact ranges" },
    features: [{ id: "definition", row: 0, support: "supported", syntax: ["S1", "S3", "S4", "S5"],
      layers: ["service", "protocol"], layerRationale: "query and projection", environmentRationale: "source ranges",
      states: ["dirty"], environments: ["utf16"], editor: true }], evidence: [] };
  return { sources, catalog };
}

test("grammar inventory excludes comment examples and accepts multiline rules", () => {
  assert.deepEqual(model.grammarRules("(* fake = x ; *)\nreal\n = 'x' ;\nTOKEN = 'y' ;"), ["TOKEN", "real"]);
});

test("protocol dimension ranges expand without dropping intermediate dimensions", () => {
  const { catalog } = fixture();
  assert.deepEqual(catalog.protocolBaseline[0].syntax, ["S1", "S3", "S4", "S5"]);
});

test("every grammar production is classified once and surface changes require review", () => {
  const { catalog, sources } = fixture();
  catalog.grammarGroups[0].productions.pop();
  assert.throws(() => model.validateCatalog(catalog, sources), /exactly one semantic group/);
  catalog.grammarGroups[0].productions.push("source_file");
  sources.protocol = sources.protocol.replace("| functions |", "| functions and aliases |");
  assert.throws(() => model.validateCatalog(catalog, sources), /syntax dimension contract changed/);
});

test("an omitted protocol feature fails the inventory gate", () => {
  const { catalog, sources } = fixture();
  catalog.features = [];
  assert.throws(() => model.validateCatalog(catalog, sources), /exactly one/);
});

test("new grammar productions and modified syntax contracts force review", () => {
  const { catalog, sources } = fixture();
  assert.throws(() => model.validateCatalog(catalog, { ...sources, grammar: sources.grammar + "new_item = 'type' ;\n" }), /grammar productions/);
  sources.syntax["syntax.rs"] += "enum NewKind { Alias }\n";
  assert.throws(() => model.validateCatalog(catalog, sources), /syntax contract/);
});

test("changing a protocol assertion forces review even if row names are unchanged", () => {
  const { catalog, sources } = fixture();
  sources.protocol = sources.protocol.replace("exact target", "exact target and version");
  assert.throws(() => model.validateCatalog(catalog, sources), /protocol requirements changed/);
});

test("missing evidence stays unreviewed regardless of candidate test count", () => {
  const { catalog, sources } = fixture();
  const requirements = model.validateCatalog(catalog, sources);
  const result = model.assess(requirements, [], { service: ["test_a", "test_b"], protocol: ["test_c"] });
  assert.ok(result.length > 0);
  assert.ok(result.every((item) => item.status === "unreviewed"));
});

test("evidence must exist in the compiled test registry and match the required layer", () => {
  const { catalog, sources } = fixture();
  catalog.evidence = [{ requirement: "definition/environments/utf16/protocol", assertion: "exact UTF-16 start/end", tests: [{ layer: "protocol", name: "removed" }] }];
  const requirements = model.validateCatalog(catalog, sources);
  assert.throws(() => model.assess(requirements, catalog.evidence, { protocol: [] }), /stale test reference/);
  catalog.evidence[0].tests[0].layer = "service";
  assert.throws(() => model.validateCatalog(catalog, sources), /wrong evidence layer/);
});

test("all linked tests must execute successfully; skipped or failed evidence cannot pass", () => {
  const requirements = [{ id: "a", layer: "protocol" }];
  const evidence = [{ requirement: "a", tests: [{ layer: "protocol", name: "one" }, { layer: "protocol", name: "two" }] }];
  const available = { protocol: ["one", "two"] };
  const assess = (states) => model.assess(requirements, evidence, available, { protocol: new Map(states) })[0].status;
  assert.equal(assess([]), "mapped");
  assert.equal(assess([["one", "ok"]]), "mapped");
  assert.equal(assess([["one", "ok"], ["two", "ignored"]]), "failed");
  assert.equal(assess([["one", "ok"], ["two", "FAILED"]]), "failed");
  assert.equal(assess([["one", "ok"], ["two", "ok"]]), "verified");
});

test("N/A requires a reason and cannot conceal mapped evidence", () => {
  const { catalog, sources } = fixture();
  catalog.exemptions = [{ requirement: "definition/editor/smoke", reason: "" }];
  assert.throws(() => model.validateCatalog(catalog, sources), /invalid or conflicting exemption/);
  catalog.exemptions[0].reason = "explicit policy";
  catalog.evidence = [{ requirement: "definition/editor/smoke", assertion: "target", tests: [{ layer: "editor", name: "test" }] }];
  assert.throws(() => model.validateCatalog(catalog, sources), /invalid or conflicting exemption/);
});

test("cargo parser distinguishes discovery, execution and ignored tests", () => {
  assert.deepEqual(model.testNames("a::b: test\r\na::bench: benchmark\r\n1 test, 1 benchmark\r\n"), ["a::b"]);
  assert.deepEqual([...model.testResults("test a::b ... ok\r\ntest a::c ... ignored, expensive\r\ntest a::d ... FAILED\n")],
    [["a::b", "ok"], ["a::c", "ignored"], ["a::d", "FAILED"]]);
});
