"use strict";

const fs = require("node:fs");
const path = require("node:path");
const crypto = require("node:crypto");

function provenance(root, binary) {
  const files = ["editors/vscode/extension.js", "editors/vscode/package.json", "editors/vscode/package-lock.json"];
  function collect(relative) {
    for (const entry of fs.readdirSync(path.join(root, relative), { withFileTypes: true })) {
      const file = `${relative}/${entry.name}`;
      if (entry.isDirectory()) collect(file);
      else if (entry.isFile()) files.push(file);
    }
  }
  collect("editors/vscode/test");
  collect("tests/lsp_matrix/fixtures");
  collect("scripts/lsp-matrix");
  const inputs = crypto.createHash("sha256");
  for (const file of files.sort()) inputs.update(file + "\0").update(fs.readFileSync(path.join(root, file)));
  return {
    inputsSha256: inputs.digest("hex"),
    serverSha256: crypto.createHash("sha256").update(fs.readFileSync(binary)).digest("hex"),
    platform: process.platform
  };
}

module.exports = { provenance };
