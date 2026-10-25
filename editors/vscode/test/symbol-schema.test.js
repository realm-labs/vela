"use strict";
const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const { materializeSymbols } = require("./document-symbol-provider");
const { materializeWorkspaceSymbols } = require("./workspace-symbol-provider");

for (const kind of ["document", "workspace"]) {
  test(`${kind} symbol materialization loads schema4 owned record facts without invented source locations`, t => {
    const spec = require(`../../../tests/lsp_matrix/fixtures/${kind}-symbol-ownership.json`);
    const original = structuredClone(spec);
    fs.mkdirSync(path.join(__dirname, "../test-results"), { recursive: true });
    const root = fs.mkdtempSync(path.join(__dirname, "../test-results/symbol-schema-"));
    t.diagnostic(`Materialized fixtures: ${root}`);
    let workspace;
    if (kind === "document") materializeSymbols(root);
    else workspace = JSON.parse(fs.readFileSync(materializeWorkspaceSymbols(root), "utf8"));
    for (const ending of ["lf", "crlf"]) {
      const family = path.join(root, kind === "document" ? "中文 % symbol roots" : "中文 % workspace symbols", `ownership-${ending}`);
      const artifact = JSON.parse(fs.readFileSync(path.join(family, "schema.json"), "utf8"));
      assert.equal(artifact.formatVersion, 4, "installed schema must match the current backend format");
      const facts = structuredClone(spec.oracle.schema);
      for (const entries of Object.values(facts)) for (const entry of entries) delete entry.sourceSpan;
      assert.deepEqual(artifact, { formatVersion: 4, facts }, "all authored facts retained, only source spans omitted");
      assert.deepEqual(artifact.facts.types.find(row => row.name === "host::Box").fact,
        { kind: "record", name: "host::Box" });
      assert.deepEqual(artifact.facts.fields.find(row => row.owner === "host::Box" && row.name === "value").fact,
        { kind: "primitive", name: "string" });
      assert(Object.values(artifact.facts).flat().every(row => !Object.hasOwn(row, "sourceSpan")));
      const source = fs.readFileSync(path.join(family, "scripts/source.vela"), "utf8");
      assert(source.includes("pub struct Widget"), "script Struct ownership stays independent of schema record facts");
      assert.equal(source.includes("\r\n"), ending === "crlf");
      if (kind === "document") {
        const manifest = fs.readFileSync(path.join(family, "vela.toml"), "utf8");
        assert.equal(manifest, ending === "crlf" ? spec.files["vela.toml"].replaceAll("\n", "\r\n") : spec.files["vela.toml"]);
        assert(manifest.replaceAll("\r\n", "\n").includes("[host]\nschema = 'schema.json'"), "manifest selects the actual materialized schema");
        assert.equal(fs.readFileSync(path.join(family, "scripts/schema_probe.vela"), "utf8"),
          "fn schema_probe(host: host::Box) { host.value; }\n");
      } else {
        assert.equal(workspace.folders[0].path, path.dirname(family));
        assert.equal(workspace.settings["vela.host.schema"], "", "schema selection stays session-owned");
      }
    }
    assert.deepEqual(spec, original, "materialization never mutates the backend's source-backed oracle");
  });
}
