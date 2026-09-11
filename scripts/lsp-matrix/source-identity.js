"use strict";

const fs = require("node:fs");
const path = require("node:path");
const crypto = require("node:crypto");
const { execFileSync } = require("node:child_process");

// The checkpoint records the tree it validates; including its own bytes would
// make accepting a batch invalidate that same evidence. All other source files,
// including untracked implementation work, participate in the identity.
function sourceIdentity(root) {
  const git = (args) => execFileSync("git", args, { cwd: root, encoding: "utf8", timeout: 10000, maxBuffer: 16 * 1024 * 1024 });
  const files = [...new Set(git(["ls-files", "-z", "--cached", "--others", "--exclude-standard"]).split("\0").filter(Boolean))].sort();
  const hash = crypto.createHash("sha256");
  for (const file of files) {
    if (file === "tests/lsp_matrix/checkpoint.json") continue;
    const absolute = path.join(root, file);
    hash.update(file + "\0");
    let stat;
    try { stat = fs.lstatSync(absolute); }
    catch (error) {
      if (error.code !== "ENOENT") throw error;
      hash.update("deleted\0");
      continue;
    }
    hash.update(stat.isSymbolicLink() ? "symlink\0" : "file\0");
    hash.update(String(stat.mode & 0o111) + "\0");
    hash.update(stat.isSymbolicLink() ? fs.readlinkSync(absolute) : fs.readFileSync(absolute));
    hash.update("\0");
  }
  return { revision: git(["rev-parse", "HEAD"]).trim(), treeSha256: hash.digest("hex") };
}

module.exports = { sourceIdentity };
