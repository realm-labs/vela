"use strict";
const fs = require("node:fs");
const path = require("node:path");
function findLog(root, name) {
  const matches = [];
  function visit(directory) {
    for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
      const file = path.join(directory, entry.name);
      if (entry.isDirectory()) visit(file);
      else if (entry.isFile() && name(entry.name)) matches.push(file);
    }
  }
  visit(path.join(root, "user-data/logs"));
  if (matches.length !== 1) throw Error(`expected one test-owned log, found ${matches.length}`);
  return matches[0];
}
function retainLogs(root, workspace) {
  for (const [destination, source] of [
    ["lsp-trace.log", findLog(root, (name) => name.endsWith("-Vela LSP Trace.log"))],
    ["extension-host.log", findLog(root, (name) => name === "exthost.log")],
    ["server-trace.jsonl", path.join(workspace, ".vela-lsp-trace.jsonl")],
  ]) fs.copyFileSync(source, path.join(root, destination));
}
module.exports = { findLog, retainLogs };
