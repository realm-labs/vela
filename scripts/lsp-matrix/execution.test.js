"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const model = require("./model");
const execution = require("./execution");

const root = path.resolve(__dirname, "../..");
const read = (file) => fs.readFileSync(path.join(root, file), "utf8");
const json = (file) => JSON.parse(read(`tests/lsp_matrix/${file}`));

function fixture() {
  const catalog = json("catalog.json");
  catalog.features = catalog.featureFiles.flatMap(json);
  catalog.protocolBaseline = json("protocol-baseline.json");
  return { catalog, requirements: model.obligations(catalog),
    manifest: json("execution-manifest.json"), baseline: json("execution-baseline.json"),
    scenarios: json("interactions.json").scenarios,
    sources: { plan: read("docs/lsp-test-execution-plan.md"), interactions: read("docs/lsp-vscode-interaction-matrix.md") } };
}

function validate(f) {
  return execution.validateManifest(f.manifest, f.baseline, f.catalog, f.requirements, f.scenarios, f.sources);
}

test("every semantic, interaction and later deliverable has exactly one stable owner", () => {
  const f = fixture();
  const all = validate(f);
  assert.equal(all.filter((item) => item.feature).length, f.requirements.length);
  assert.ok(all.some((item) => item.id === "vscode/UX04/accept-tab/input/local"));
  assert.ok(all.some((item) => item.id === "vscode/UX04/accept-enter/render/local"));
  assert.ok(all.some((item) => item.id === "batch/B18/scale-budgets"));
  assert.equal(new Set(all.map((item) => item.id)).size, all.length);
  assert.equal(all.filter((item) => item.owner === "B16").length, 0);
});

test("missing and duplicate owners fail even in an unaccepted batch", () => {
  const f = fixture();
  const batch = f.manifest.batches.find((batch) => batch.id === "B02");
  const id = batch.requirements.pop();
  assert.throws(() => validate(f), /owned requirements/);
  batch.requirements.push(id, id);
  assert.throws(() => validate(f), /duplicate requirement owner/);
});

test("moving an obligation to the wrong batch cannot satisfy ownership", () => {
  const f = fixture();
  const source = f.manifest.batches.find((batch) => batch.id === "B02");
  const target = f.manifest.batches.find((batch) => batch.id === "B03");
  target.requirements.push(source.requirements.pop());
  assert.throws(() => validate(f), /wrong requirement owner/);
});

test("empty semantic batches and missing late gates cannot disappear", () => {
  const f = fixture();
  f.manifest.batches.find((batch) => batch.id === "B13").requirements = [];
  assert.throws(() => validate(f), /empty batch/);
  const g = fixture();
  const late = g.manifest.batches.find((batch) => batch.id === "B17");
  const removed = late.deliverables.pop();
  late.requirements = late.requirements.filter((id) => id !== `batch/B17/${removed.id}`);
  assert.throws(() => validate(g), /explicit migration/);
});

test("only the named B16 families can be deferred", () => {
  const f = fixture();
  f.scenarios.find((item) => item.id === "UX04").scope = "deferred";
  assert.throws(() => validate(f), /invalid scope deferral/);
  const g = fixture();
  g.manifest.batches.find((batch) => batch.id === "B03").scope = "deferred";
  assert.throws(() => validate(g), /invalid batch deferral/);
  const h = fixture();
  h.manifest.batches.find((batch) => batch.id === "B16").requirements.push("completion/editor/smoke");
  assert.throws(() => validate(h), /must not own active local/);
});

test("missing interaction families and explicit alternative routes fail", () => {
  const f = fixture();
  f.scenarios = f.scenarios.filter((item) => item.id !== "UX21");
  assert.throws(() => validate(f), /interaction scenarios/);
  const g = fixture();
  const scenario = g.scenarios.find((item) => item.id === "UX04");
  scenario.routes = scenario.routes.filter((route) => route.id !== "accept-tab");
  const batch = g.manifest.batches.find((batch) => batch.id === "B03");
  batch.requirements = batch.requirements.filter((id) => !id.includes("UX04/accept-tab/"));
  assert.throws(() => validate(g), /explicit migration/);
});

test("interaction evidence cannot be downgraded to provider or command", () => {
  for (const level of ["Provider", "Command"]) {
    const f = fixture();
    f.scenarios.find((item) => item.id === "UX04").routes[0].levels = [level];
    assert.throws(() => validate(f), /evidence levels/);
  }
});

test("negative interaction routes and independent expectations are mandatory", () => {
  const f = fixture();
  f.scenarios[0].routes = f.scenarios[0].routes.filter((route) => route.polarity !== "negative");
  assert.throws(() => validate(f), /missing negative route/);
  const g = fixture();
  g.scenarios[0].routes[0].assertion = "";
  assert.throws(() => validate(g), /invalid route definition/);
});

test("changed interaction text requires reviewed contract and route migration", () => {
  const f = fixture();
  f.sources.interactions = f.sources.interactions.replace("accept using Enter and Tab separately", "accept using Enter, Tab and another route");
  assert.throws(() => validate(f), /interaction contract changed/);
});

test("changed applicability and N/A reasons require explicit migration", () => {
  const f = fixture();
  f.catalog.exemptions.push({ requirement: "definition/environments/encoded_uri/protocol", reason: "too difficult" });
  assert.throws(() => validate(f), /explicit migration/);
  const g = fixture();
  g.requirements = g.requirements.filter((item) => item.id !== "definition/environments/encoded_uri/protocol");
  const batch = g.manifest.batches.find((batch) => batch.id === "B02");
  batch.requirements = batch.requirements.filter((id) => id !== "definition/environments/encoded_uri/protocol");
  assert.throws(() => validate(g), /explicit migration/);
});

test("new requirements must be explicitly assigned before they enter scope", () => {
  const f = fixture();
  f.requirements.push({ id: "definition/states/new-case/protocol", feature: "definition", axis: "states", value: "new-case", layer: "protocol" });
  assert.throws(() => validate(f), /owned requirements/);
  f.manifest.batches.find((batch) => batch.id === "B02").requirements.push("definition/states/new-case/protocol");
  assert.ok(validate(f).some((item) => item.id === "definition/states/new-case/protocol"));
});

test("splits name exact old/new contracts and preserved behavior", () => {
  const original = { id: "old", owner: "B02", contractHash: "old-hash" };
  const current = [{ id: "new-a", owner: "B02", contractHash: "a-hash" }, { id: "new-b", owner: "B14", contractHash: "b-hash" }];
  const baseline = { version: 1, requirements: [original] };
  const migration = { id: "split-old", reason: "split semantic partitions", preservedBehavior: "both original target kinds are covered", from: [original], to: structuredClone(current) };
  assert.throws(() => execution.validateBaseline(current, baseline, []), /explicit migration/);
  execution.validateBaseline(current, baseline, [migration]);
  migration.to[0] = { ...migration.to[0], contractHash: "wrong" };
  assert.throws(() => execution.validateBaseline(current, baseline, [migration]), /invalid migration target/);
  migration.preservedBehavior = "";
  assert.throws(() => execution.validateBaseline(current, baseline, [migration]), /preserve the original behavior/);
});
