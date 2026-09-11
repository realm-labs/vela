"use strict";
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { execFileSync } = require("node:child_process");

async function withInputSource(target, run, invoke, log = console.log) {
  const previous = invoke("get");
  const switched = previous !== target;
  try {
    if (switched) {
      invoke("select", target);
      log(`Temporarily selected ${target}; will restore ${previous}`);
    }
    return await run();
  } finally {
    // Restore even when selection reports a failure after changing OS state.
    if (switched) {
      invoke("select", previous);
      log(`Restored input source ${previous}`);
    }
  }
}

async function withKeyboardLayout(target, run) {
  const temporary = fs.mkdtempSync(path.join(os.tmpdir(), "vela-keyboard-"));
  const helper = path.join(temporary, "keyboard-layout");
  try {
    execFileSync("swiftc", [path.join(__dirname, "keyboard-layout.swift"), "-o", helper],
      { stdio: "inherit", timeout: 120000 });
    return await withInputSource(target, run, (...args) =>
      execFileSync(helper, args, { encoding: "utf8", timeout: 5000 }).trim());
  } finally {
    fs.rmSync(temporary, { recursive: true, force: true });
  }
}
module.exports = { withInputSource, withKeyboardLayout };
