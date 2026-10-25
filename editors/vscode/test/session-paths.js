"use strict";
const assert = require("node:assert/strict");
const path = require("node:path");

const directories = { "workspace-symbol": "w", "selection-syntax": "s", "selection-lifecycle": "l" };

function extraSessionDirectory(resultRoot, session, platform = process.platform) {
  assert(Object.hasOwn(directories, session), `unknown isolated editor session: ${session}`);
  const directory = path.resolve(resultRoot, directories[session]);
  // VS Code's macOS main socket lives inside user-data. Reserve a longer
  // version prefix than the current 1.13-main.sock, without relocating logs.
  if (platform === "darwin") {
    const socket = path.join(directory, "user-data", "1.999-main.sock");
    assert(Buffer.byteLength(socket, "utf8") <= 103,
      `macOS editor IPC socket path exceeds 103 bytes; use a shorter checkout path: ${socket}`);
  }
  return directory;
}

module.exports = { extraSessionDirectory };
