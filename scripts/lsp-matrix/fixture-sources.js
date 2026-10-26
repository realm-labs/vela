"use strict";

// Version 1's declared document slots accept text or exactly { sourceFile }.
// This is deliberately not a recursive include walker: metadata and edit fragments
// outside these slots are never interpreted as filesystem instructions.
const fs = require("node:fs");
const path = require("node:path");
const decoder = new TextDecoder("utf-8", { fatal: true, ignoreBOM: true });

function sourceText(value, root, nullable) {
  if (value === null && nullable) return value;
  if (typeof value === "string") {
    if (!value.isWellFormed()) throw new Error("fixture source must be valid Unicode");
    return value;
  }
  if (!value || Array.isArray(value) || typeof value !== "object" ||
      Object.keys(value).length !== 1 || typeof value.sourceFile !== "string") {
    throw new Error("fixture source must be text or exactly { sourceFile: string }");
  }
  const file = value.sourceFile;
  if (!file.isWellFormed()) throw new Error("fixture source path must be valid Unicode");
  const parts = file.split("/");
  if (parts[0] !== "sources" || parts.length < 2 || file.includes("\\") ||
      file.includes(":") || file.includes("\0") ||
      parts.some(part => !part || part === "." || part === "..")) {
    throw new Error(`invalid fixture source path: ${file}`);
  }
  let target = fs.realpathSync(root);
  for (const part of parts) {
    target = path.join(target, part);
    // Reject internal symlinks too: raw provenance hashes regular files, not
    // symlink targets. These trusted test trees are not concurrently mutable.
    if (fs.lstatSync(target).isSymbolicLink()) throw new Error(`fixture source symlink: ${file}`);
  }
  if (!fs.statSync(target).isFile()) throw new Error(`fixture source is not a file: ${file}`);
  return decoder.decode(fs.readFileSync(target));
}

function resolveSources(spec, root) {
  function slot(node, keys, nullable = false) {
    const [key, ...rest] = keys;
    if (key === "*") {
      if (!node || typeof node !== "object") throw new Error("invalid fixture source container");
      for (const name of Object.keys(node)) {
        if (rest.length) slot(node[name], rest, nullable);
        else node[name] = sourceText(node[name], root, nullable);
      }
    } else if (node && Object.hasOwn(node, key)) {
      if (rest.length) slot(node[key], rest, nullable);
      else node[key] = sourceText(node[key], root, nullable);
    }
  }
  const resolve = (keys, nullable = false) => slot(spec, keys.split("/"), nullable);
  resolve("files/*");
  resolve("actions/*/source", true);
  const id = spec.id || "";
  if (["completion-source-callable-lifecycle", "completion-source-overlay-lifecycle", "completion-source-recovery"].includes(id)) {
    resolve("oracle/sourceLifecycle/*/files/*", true);
    for (const key of ["action/source", "diskSource", "openSource"]) resolve(`oracle/sourceLifecycle/*/${key}`, true);
  }
  if (id.startsWith("hover-") || id.startsWith("signature-")) resolve("oracle/phases/*/actions/*/source", true);
  if (id === "signature-editor") resolve("oracle/recovery/source");
  if (id === "document-change") resolve("oracle/cases/*/steps/*/text");
  if (id.startsWith("selection-") || id.startsWith("document-symbol-") || id.startsWith("workspace-symbol-")) {
    resolve("oracle/cases/*/source");
    resolve("oracle/variants/*/source");
  }
  if (id.startsWith("semantic-token-")) {
    for (const key of ["negative/source", "negative/cases/*/source", "states/*/source",
      "states/*/queries/*/source", "initial/*/source", "steps/*/queries/*/source"]) resolve(`oracle/${key}`);
    resolve("oracle/states/*/diskSources/*", true);
    resolve("oracle/states/*/openSources/*", true);
    resolve("oracle/states/*/action/source", true);
    resolve("oracle/steps/*/source", true);
  }
  if (id === "input-semantic-tokens") {
    resolve("oracle/shifted");
    resolve("oracle/unknown");
  }
  return spec;
}

function loadFixture(file) {
  return resolveSources(JSON.parse(decoder.decode(fs.readFileSync(file))), path.dirname(file));
}

module.exports = { loadFixture };
