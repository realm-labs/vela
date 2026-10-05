"use strict";
const assert = require("node:assert/strict");
const crypto = require("node:crypto");
const fs = require("node:fs");
const path = require("node:path");
const { logFiles } = require("./logs");

// A PID may be reused while an earlier reload's output files still exist.
// Bind the channel name and its complete marker to this specific invocation.
function hostIdentity(pid, nonce) {
  assert(Number.isSafeInteger(pid) && pid > 0, "valid observer PID");
  assert(typeof nonce === "string" && /^[0-9a-f]{32}$/.test(nonce), "valid observer invocation nonce");
  return { name: `Vela Test Host ${pid} ${nonce}`, marker: `VELA_INPUT_HOST ${pid} ${nonce}` };
}
function hostLogDirectory(root, identity) {
  const files = logFiles(root, name => name.endsWith(`-${identity.name}.log`));
  assert(files.length <= 1, "unique current invocation identity log");
  if (files.length === 0) return null;
  const lines = fs.readFileSync(files[0], "utf8").split(/\r?\n/); lines.pop();
  return lines.includes(identity.marker) ? path.dirname(files[0]) : null;
}
async function observeHostLogs(vscode, host) {
  const identity = hostIdentity(host.pid, crypto.randomBytes(16).toString("hex"));
  const channel = vscode.window.createOutputChannel(identity.name);
  channel.appendLine(identity.marker);
  const deadline = Date.now() + 10000;
  while (Date.now() < deadline) {
    const directory = hostLogDirectory(host.logDirectory, identity);
    if (directory) return directory;
    await new Promise(resolve => setTimeout(resolve, 25));
  }
  throw Error("current installed observer identity log did not become ready");
}
module.exports = { hostIdentity, hostLogDirectory, observeHostLogs };
