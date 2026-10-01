"use strict";
const fs = require("node:fs");
const path = require("node:path");
function logFiles(directory, name) {
  const matches = [];
  function visit(directory) {
    for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
      const file = path.join(directory, entry.name);
      if (entry.isDirectory()) visit(file);
      else if (entry.isFile() && name(entry.name)) matches.push(file);
    }
  }
  visit(directory);
  return matches.sort();
}
function findLog(root, name) {
  const matches = logFiles(path.join(root, "user-data/logs"), name);
  if (matches.length !== 1) throw Error(`expected one test-owned log, found ${matches.length}`);
  return matches[0];
}
function retainLogs(root, workspace) {
  for (const [destination, predicate] of [
    ["lsp-trace.log", name => name.endsWith("-Vela LSP Trace.log")],
    ["extension-host.log", name => name === "exthost.log"],
    ["vela-output.log", name => /-Vela\.log$/.test(name)],
  ]) {
    const sources = logFiles(path.join(root, "user-data/logs"), predicate);
    if (!sources.length) throw Error(`missing retained log ${destination}`);
    fs.writeFileSync(path.join(root, destination), sources.map(file =>
      `\n=== ${path.relative(root,file).split(path.sep).join('/')} ===\n` + fs.readFileSync(file,'utf8')).join(''));
  }
  fs.copyFileSync(path.join(workspace, ".vela-lsp-trace.jsonl"), path.join(root,"server-trace.jsonl"));
}
module.exports = { findLog, logFiles, retainLogs };
