"use strict";
const fs = require("node:fs");
const path = require("node:path");
const assert = require("node:assert/strict");
const { FixtureWorkspace } = require("../../../scripts/lsp-matrix/fixtures");
const spec = require("../../../tests/lsp_matrix/fixtures/input-completion.json");

function materializeCompletion(workspace) {
  for (const [file, document] of new FixtureWorkspace(spec).disk) {
    const target = path.join(workspace, file);
    assert.ok(!fs.existsSync(target), "completion fixture must not overwrite existing files");
    fs.mkdirSync(path.dirname(target), { recursive: true });
    fs.writeFileSync(target, document.text);
  }
  const config = path.join(workspace, "vela.toml");
  assert.ok(!fs.readFileSync(config, "utf8").includes("[host]"));
  fs.appendFileSync(config, "\n[host]\nschema = 'ux04-schema.json'\n");
}
module.exports = { materializeCompletion };
