"use strict";
const fs = require("node:fs");
const path = require("node:path");
const model = require("./model");
const { validateManifest } = require("./execution");

function loadInventory(root) {
  const read = (file) => fs.readFileSync(path.join(root, file), "utf8");
  const catalog = JSON.parse(read("tests/lsp_matrix/catalog.json"));
  catalog.protocolBaseline = JSON.parse(
    read("tests/lsp_matrix/protocol-baseline.json"),
  );
  Object.assign(
    catalog,
    JSON.parse(read("tests/lsp_matrix/syntax-baseline.json")),
  );
  catalog.features = catalog.featureFiles.flatMap((file) => {
    if (!/^features\/[a-z-]+\.json$/.test(file))
      throw new Error(`invalid matrix feature file ${file}`);
    return JSON.parse(read(`tests/lsp_matrix/${file}`));
  });
  const requirements = model.validateCatalog(catalog, {
    protocol: read("docs/lsp-protocol-test-matrix.md"),
    grammar: read("docs/grammar.ebnf"),
    syntax: Object.fromEntries(
      Object.keys(catalog.syntaxSources).map((file) => [file, read(file)]),
    ),
  });
  const manifest = JSON.parse(read("tests/lsp_matrix/execution-manifest.json"));
  const interactions = JSON.parse(read("tests/lsp_matrix/interactions.json"));
  if (interactions.version !== 1)
    throw new Error("unsupported interaction inventory version");
  const executionRequirements = validateManifest(
    manifest,
    JSON.parse(read("tests/lsp_matrix/execution-baseline.json")),
    catalog,
    requirements,
    interactions.scenarios,
    {
      plan: read("docs/lsp-test-execution-plan.md"),
      interactions: read("docs/lsp-vscode-interaction-matrix.md"),
    },
  );
  return { catalog, requirements, manifest, executionRequirements };
}
module.exports = { loadInventory };
