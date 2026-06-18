"use strict";

const fs = require("fs");
const path = require("path");

const root = path.resolve(__dirname, "..");
const repositoryRoot = path.resolve(root, "..", "..").replace(/\\/g, "/");
const sharedHighlightingFixture = path.join(
  root,
  "..",
  "..",
  "tests",
  "fixtures",
  "lsp_highlighting",
  "showcase.vela"
);
const sharedHighlightingConsistency = path.join(
  root,
  "..",
  "..",
  "tests",
  "fixtures",
  "lsp_highlighting",
  "consistency.json"
);

function read(relativePath) {
  return fs.readFileSync(path.join(root, relativePath), "utf8");
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

function hasTomlValue(text, key, value) {
  return text.split(/\r?\n/).some((line) => line.trim() === `${key} = ${value}`);
}

const manifest = read("extension.toml");
assert(hasTomlValue(manifest, "id", '"vela"'), "extension id must be vela");
assert(hasTomlValue(manifest, "schema_version", "1"), "schema_version must be 1");
assert(
  manifest.includes('languages = ["languages/vela"]'),
  "extension.toml must register the Vela language config path"
);
assert(
  manifest.includes("[language_servers.vela-language-server]"),
  "language server section is missing"
);
assert(
  !manifest.includes("[languages."),
  "extension.toml must not use [languages.*] tables; Zed expects language server languages lists"
);
assert(
  manifest.includes('languages = ["Vela"]'),
  "Vela language server must list the Vela language from languages/vela/config.toml"
);
assert(manifest.includes("[grammars.vela]"), "Vela grammar section is missing");
assert(
  manifest.includes(`repository = "${repositoryRoot}"`),
  "Vela grammar must use the repository root as the local grammar checkout source"
);
assert(
  manifest.includes('rev = "master"'),
  "Vela grammar must include a rev field for Zed extension manifest parsing"
);
assert(
  manifest.includes('path = "editors/tree-sitter-vela"'),
  "Vela grammar must use the grammar source path inside the repository checkout"
);

const languageConfigPath = "languages/vela/config.toml";
assert(fs.existsSync(path.join(root, languageConfigPath)), "language config is missing");
const languageConfig = read(languageConfigPath);
assert(hasTomlValue(languageConfig, "name", '"Vela"'), "language name is missing");
assert(hasTomlValue(languageConfig, "grammar", '"vela"'), "grammar name is missing");
assert(hasTomlValue(languageConfig, "path_suffixes", '["vela"]'), "path suffix is missing");

const extensionRs = read("src/lib.rs");
assert(
  extensionRs.includes("vela_lsp_server") && extensionRs.includes("--stdio"),
  "extension launcher must start vela_lsp_server over stdio"
);
assertThinLauncher(extensionRs, "Zed extension");

const cargoManifest = read("Cargo.toml");
assert(cargoManifest.includes("[workspace]"), "Zed extension crate must be isolated from the root workspace");

const grammarRoot = path.resolve(root, "..", "tree-sitter-vela");
assert(fs.existsSync(path.join(grammarRoot, "grammar.js")), "tree-sitter grammar.js is missing");
assert(fs.existsSync(path.join(grammarRoot, "tree-sitter.json")), "tree-sitter.json is missing");
assert(fs.existsSync(path.join(grammarRoot, "src", "parser.c")), "generated parser.c is missing");
assert(!fs.existsSync(path.join(root, "grammars", "vela")), "Zed grammar checkout directory must not contain source files");
assert(fs.existsSync(path.join(root, "languages", "vela", "highlights.scm")), "highlights query is missing");
assert(fs.existsSync(path.join(root, "languages", "vela", "brackets.scm")), "brackets query is missing");
assert(fs.existsSync(path.join(root, "languages", "vela", "indents.scm")), "indents query is missing");
assert(fs.existsSync(path.join(root, "languages", "vela", "outline.scm")), "outline query is missing");
assert(fs.existsSync(path.join(root, "languages", "vela", "overrides.scm")), "overrides query is missing");
assert(fs.existsSync(path.join(root, "languages", "vela", "textobjects.scm")), "textobjects query is missing");

assert(fs.existsSync(sharedHighlightingFixture), "shared highlighting showcase fixture is missing");
assert(fs.existsSync(sharedHighlightingConsistency), "shared highlighting consistency table is missing");
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

const highlights = read("languages/vela/highlights.scm");
const consistency = JSON.parse(fs.readFileSync(sharedHighlightingConsistency, "utf8"));
for (const capture of [
  "@namespace",
  "@attribute",
  "@function",
  "@function.method",
  "@type",
  "@property",
  "@constant",
  "@constant.builtin",
  "@variable.parameter",
  "@variable.special",
  "@string.special",
  "@boolean",
  "@number",
  "@operator",
  "@punctuation.delimiter",
  "@punctuation.bracket"
]) {
  assert(highlights.includes(capture), `Zed highlights query must include ${capture}`);
}
assert(!highlights.includes("@variant"), "Zed highlights query should use conventional captures for variants");
assert(!highlights.includes("@enum"), "Zed highlights query should use @type for enum declarations");
for (const entry of consistency) {
  assert(
    highlights.includes(entry.zedCapture),
    `Zed highlights query must include ${entry.zedCapture} for ${entry.concept}`
  );
}

console.log("Zed extension package metadata and launcher boundary are valid.");
