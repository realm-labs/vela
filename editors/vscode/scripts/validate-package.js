"use strict";

const fs = require("fs");
const path = require("path");

const root = path.resolve(__dirname, "..");

function readJson(relativePath) {
  return JSON.parse(fs.readFileSync(path.join(root, relativePath), "utf8"));
}

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

const manifest = readJson("package.json");
assert(manifest.main === "./extension.js", "package main must point at extension.js");
assert(manifest.activationEvents.includes("onLanguage:vela"), "Vela language activation is missing");
assert(manifest.dependencies["vscode-languageclient"], "vscode-languageclient dependency is required");

const language = manifest.contributes.languages.find((entry) => entry.id === "vela");
assert(language, "Vela language contribution is missing");
assert(language.extensions.includes(".vela"), ".vela extension contribution is missing");
assert(fs.existsSync(path.join(root, language.configuration)), "language configuration path is missing");

const grammar = manifest.contributes.grammars.find((entry) => entry.language === "vela");
assert(grammar, "Vela grammar contribution is missing");
assert(grammar.scopeName === "source.vela", "Vela grammar must use source.vela");
assert(fs.existsSync(path.join(root, grammar.path)), "grammar path is missing");

const languageConfiguration = readJson(language.configuration);
assert(languageConfiguration.comments.lineComment === "//", "line comment configuration is missing");

const grammarJson = readJson(grammar.path);
assert(grammarJson.scopeName === "source.vela", "grammar scopeName must match manifest");

console.log("VS Code extension package metadata is valid.");
