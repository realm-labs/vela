"use strict";

const fs = require("fs");
const path = require("path");

const root = path.resolve(__dirname, "..");
const sharedHighlightingFixture = path.join(
  root,
  "..",
  "..",
  "tests",
  "fixtures",
  "lsp_highlighting",
  "showcase.vela"
);

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

const expectedSemanticTokenTypes = [
  "bytes",
  "typeAlias",
  "const",
  "global",
  "boolean",
  "null",
  "builtinType",
  "label",
  "unresolvedReference",
  "arithmeticOperator",
  "assignmentOperator",
  "bitwiseOperator",
  "comparisonOperator",
  "logicalOperator",
  "negationOperator",
  "punctuation",
  "brace",
  "bracket",
  "parenthesis",
  "comma",
  "dot",
  "colon",
  "semicolon",
  "pathSeparator"
];
const semanticTokenTypes = manifest.contributes.semanticTokenTypes ?? [];
for (const id of expectedSemanticTokenTypes) {
  const entry = semanticTokenTypes.find((tokenType) => tokenType.id === id);
  assert(entry, `semanticTokenTypes must include ${id}`);
  assert(entry.superType, `semantic token type ${id} must declare a standard superType`);
}

const expectedSemanticTokenModifiers = [
  "host",
  "unresolved",
  "source",
  "public",
  "mutable",
  "callable",
  "controlFlow",
  "associated",
  "trait",
  "schema"
];
const semanticTokenModifiers = manifest.contributes.semanticTokenModifiers ?? [];
for (const id of expectedSemanticTokenModifiers) {
  assert(
    semanticTokenModifiers.some((modifier) => modifier.id === id),
    `semanticTokenModifiers must include ${id}`
  );
}

const semanticTokenScopes = manifest.contributes.semanticTokenScopes ?? [];
const velaScopes = semanticTokenScopes.find((entry) => entry.language === "vela");
assert(velaScopes, "semanticTokenScopes must include Vela language mappings");
for (const id of expectedSemanticTokenTypes) {
  assert(velaScopes.scopes[id], `semanticTokenScopes must map ${id}`);
}
assert(velaScopes.scopes["function.defaultLibrary"], "semanticTokenScopes must map stdlib functions");
assert(velaScopes.scopes["type.defaultLibrary"], "semanticTokenScopes must map builtin types");

const languageConfiguration = readJson(language.configuration);
assert(languageConfiguration.comments.lineComment === "//", "line comment configuration is missing");

const grammarJson = readJson(grammar.path);
assert(grammarJson.scopeName === "source.vela", "grammar scopeName must match manifest");
assert(fs.existsSync(sharedHighlightingFixture), "shared highlighting showcase fixture is missing");
const showcase = fs.readFileSync(sharedHighlightingFixture, "utf8");
for (const marker of [
  "pub struct Reward",
  "pub enum Progress",
  "pub trait Scored",
  "player.grant(level)",
  "math::max(score, rewards.len())",
  "missing_symbol"
]) {
  assert(showcase.includes(marker), `shared highlighting fixture must contain ${marker}`);
}
for (const scope of [
  "entity.name.function.vela",
  "entity.name.function.member.vela",
  "entity.name.namespace.vela",
  "entity.name.type.struct.vela",
  "entity.name.type.enum.vela",
  "entity.name.type.interface.vela",
  "support.type.builtin.vela",
  "constant.language.boolean.vela",
  "constant.language.null.vela",
  "constant.other.enum-member.vela",
  "constant.numeric.vela",
  "string.quoted.bytes.vela",
  "string.interpolated.vela",
  "variable.other.property.vela",
  "keyword.operator.vela",
  "punctuation.accessor.dot.vela",
  "punctuation.separator.vela",
  "punctuation.bracket.vela",
  "comment.line.double-slash.vela"
]) {
  assert(JSON.stringify(grammarJson).includes(scope), `TextMate grammar must include ${scope}`);
}

const extensionSource = fs.readFileSync(path.join(root, "extension.js"), "utf8");
assert(extensionSource.includes("LanguageClient"), "extension must use vscode-languageclient");
assert(extensionSource.includes("serverCommand"), "extension must provide server command discovery");
assert(extensionSource.includes("initializationOptions"), "extension must pass initialization options");
assertThinLauncher(extensionSource, "VS Code extension");

console.log("VS Code extension package metadata and launcher boundary are valid.");
