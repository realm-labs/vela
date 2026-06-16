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

function assertThinLauncher(source, label) {
  const forbiddenProtocolHandlers = [
    "textDocument/",
    "workspace/symbol",
    "workspace/executeCommand",
    "publishDiagnostics",
    "semanticTokens",
    "completionItem/resolve",
    "$/progress"
  ];
  for (const marker of forbiddenProtocolHandlers) {
    assert(!source.includes(marker), `${label} must not implement ${marker} behavior`);
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

const extensionSource = fs.readFileSync(path.join(root, "extension.js"), "utf8");
assert(extensionSource.includes("LanguageClient"), "extension must use vscode-languageclient");
assert(extensionSource.includes("serverCommand"), "extension must provide server command discovery");
assert(extensionSource.includes("initializationOptions"), "extension must pass initialization options");
assertThinLauncher(extensionSource, "VS Code extension");

console.log("VS Code extension package metadata and launcher boundary are valid.");
