"use strict";
const path = require("node:path");
const { fileURLToPath, pathToFileURL } = require("node:url");
const { safeFile } = require("../../../../scripts/lsp-matrix/fixtures");

function relativeFile(root, file) {
  return safeFile(path.relative(root, file).split(path.sep).join("/"));
}
function fileUri(file) {
  // VS Code lowercases drive letters and percent-encodes the drive colon;
  // node:url preserves the drive case and colon. Compare one canonical URI
  // without altering Unicode, escapes, filename case or source ranges.
  return pathToFileURL(path.resolve(file).replace(/^[A-Z]:/, (drive) => drive.toLowerCase())).href;
}
function canonicalUri(uri) {
  const parsed = new URL(uri);
  if (parsed.protocol !== "file:" || parsed.search || parsed.hash) throw Error("expected a plain file URI");
  return fileUri(fileURLToPath(parsed));
}
module.exports = { relativeFile, fileUri, canonicalUri };
